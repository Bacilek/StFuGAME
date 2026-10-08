// Release build has no console window, it is controlled by the icon next to the clock
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod report;
mod app;
mod arena;
mod control;
mod ctx;
mod daily;
mod dungeons;
mod guard;
mod guild;
mod hunt;
mod inventory;
mod journal;
mod missions;
mod potions;
mod roster;
mod safe;
mod session;
mod shops;
mod stable;
mod tasks;
mod tavern;
mod tournament;
mod tray;

use std::process::ExitCode;

use sf_api::{command::Command, error::SFError, gamestate::tavern::CurrentAction};

use crate::session::SimpleSession;

/// Reads a required environment variable (.env). Never prints the value.
fn env_var(name: &str) -> Result<String, String> {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => Ok(v.trim().to_string()),
        _ => Err(format!("Variable {name} is missing in the .env file")),
    }
}

/// Human-readable description of a login error. Never contains the password.
fn describe_login_error(err: &SFError) -> String {
    match err {
        SFError::ServerError(msg) => format!("The server rejected the login: {msg} (check name/password/server)"),
        SFError::ConnectionError => "Could not connect to the server (wrong SF_SERVER address or no connection)".into(),
        SFError::EmptyResponse => "The server returned an empty response".into(),
        SFError::UnsupportedVersion(v) => format!("Unsupported server version: {v}"),
        SFError::ParsingError(..) | SFError::TooShortResponse { .. } => {
            "Could not parse the server response (maybe a new game version, try updating sf-api)".into()
        }
        other => format!("Unknown error: {other}"),
    }
}

#[derive(Clone)]
pub(crate) struct Credentials {
    user: String,
    pass: String,
    pub(crate) character: String,
    /// Optional: only if there are characters with the same name on several servers
    server: Option<String>,
}

/// Logs in via the S&F account, finds the character and downloads its state.
async fn login(c: &Credentials) -> Result<SimpleSession, String> {
    report!("Logging in to the S&F account...");
    let sessions = SimpleSession::login_sf_account(&c.user, &c.pass)
        .await
        .map_err(|e| format!("Account login failed: {}", describe_login_error(&e)))?;

    let mut matching: Vec<SimpleSession> = sessions
        .into_iter()
        .filter(|s| s.username().eq_ignore_ascii_case(&c.character))
        .filter(|s| match &c.server {
            Some(srv) => s.server_host().is_some_and(|h| h.eq_ignore_ascii_case(srv)),
            None => true,
        })
        .collect();

    let mut session = match matching.len() {
        1 => matching.remove(0),
        0 => {
            return Err(format!(
                "Character {} was not found under this account (check SF_CHARACTER / SF_SERVER)",
                c.character
            ));
        }
        _ => {
            let servers: Vec<&str> = matching.iter().map(|s| s.server_host().unwrap_or("?")).collect();
            return Err(format!(
                "There are several characters named {}, specify the server in SF_SERVER: {}",
                c.character,
                servers.join(", ")
            ));
        }
    };

    roster::set_character(&c.character);
    report!("Loading character {} on {}...", c.character, session.server_host().unwrap_or("?"));
    // After an account login we have no game state yet - Update downloads it
    safe::send(&mut session, Command::Update)
        .await
        .map_err(|e| format!("Loading the character failed: {}", describe_login_error(&e)))?;
    Ok(session)
}

fn print_status(session: &SimpleSession) {
    let Some(gs) = session.game_state() else {
        report!("The server returned no game state");
        return;
    };
    let ch = &gs.character;
    let tavern = &gs.tavern;
    let thirst = tavern.thirst_for_adventure_sec;

    report!("Character: {}", ch.name);
    report!("Level:     {}", ch.level);
    report!("Gold:      {}", report::gold(ch.silver));
    report!("Mushrooms: {}", ch.mushrooms);
    report!("Tavern:");
    report!("  Thirst for Adventure: {} min {} s", thirst / 60, thirst % 60);
    report!("  Beers:  {}/{}", tavern.beer_drunk, tavern.beer_max);
    let action = match &tavern.current_action {
        CurrentAction::Idle => "idle".to_string(),
        CurrentAction::Quest { busy_until, .. } => format!("on a quest until {}", busy_until.format("%H:%M:%S")),
        CurrentAction::CityGuard { busy_until, .. } => format!("City Guard until {}", busy_until.format("%H:%M:%S")),
        CurrentAction::Expedition => "on an expedition".to_string(),
        CurrentAction::Unknown(_) => "unknown activity".to_string(),
    };
    report!("  Status: {action}");
    let fmt = |t: Option<chrono::DateTime<chrono::Local>>| t.map_or("unknown".to_string(), |t| t.format("%d.%m. %H:%M").to_string());
    report!("Daily login bonus: next {} (collected {}×)", fmt(gs.specials.calendar.next_possible), gs.specials.calendar.collected);
    report!("Wheel of Fortune: next free spin {}", fmt(gs.specials.wheel.next_free_spin));
    report!("Mount: {:?} until {}", gs.character.mount, fmt(gs.character.mount_end));
}

/// Main loop: Arena (when free, max 10 wins a day), Dungeons (when free), Tavern (one expedition),
/// Shops (once a day after the Tavern), City Guard (when the Tavern is done, until midnight),
/// and when there is nothing to do, wait for the nearest cooldown to end. Runs until stopped.
async fn play(session: &mut SimpleSession, journal: &mut journal::Journal) -> tavern::Outcome {
    let mut last_dungeon_try: Option<std::time::Instant> = None;
    loop {
        if let Some(gs) = session.game_state() {
            roster::track_level(gs);
            roster::write_now(gs);
        }
        if let tavern::Outcome::SessionLost = daily::run(session).await {
            return tavern::Outcome::SessionLost;
        }
        if let tavern::Outcome::SessionLost = inventory::manage(session).await {
            return tavern::Outcome::SessionLost;
        }
        if let tavern::Outcome::SessionLost = potions::run(session).await {
            return tavern::Outcome::SessionLost;
        }
        if let tavern::Outcome::SessionLost = guild::run(session).await {
            return tavern::Outcome::SessionLost;
        }
        if let tavern::Outcome::SessionLost = guild::battles(session).await {
            return tavern::Outcome::SessionLost;
        }
        if let tavern::Outcome::SessionLost = arena::run(session).await {
            return tavern::Outcome::SessionLost;
        }
        // Gleeman fight tasks (class, bare hands) via the Hall of Fame, after the XP wins or late
        if let tavern::Outcome::SessionLost = hunt::run(session).await {
            return tavern::Outcome::SessionLost;
        }
        let dungeon_due = session.game_state().is_some_and(|gs| dungeons::secs_until_ready(gs) == 0);
        let retry_ok = last_dungeon_try.is_none_or(|t: std::time::Instant| t.elapsed().as_secs() >= dungeons::RETRY_SEC);
        if dungeon_due && retry_ok {
            last_dungeon_try = Some(std::time::Instant::now());
            if let tavern::Outcome::SessionLost = dungeons::run(session).await {
                return tavern::Outcome::SessionLost;
            }
        }

        let Some(gs) = session.game_state() else { return tavern::Outcome::SessionLost };
        let tavern_state =
            |gs: &sf_api::gamestate::GameState| (gs.tavern.current_action, gs.tavern.thirst_for_adventure_sec);
        let before = tavern_state(gs);
        // Tavern done = no expedition running and no Thirst for Adventure left for another one
        let mut tavern_done = before.0 != CurrentAction::Expedition && before.1 == 0;
        if before.0 == CurrentAction::Expedition || before.1 > 0 {
            if let tavern::Outcome::SessionLost = tavern::run(session, journal).await {
                return tavern::Outcome::SessionLost;
            }
            // The Tavern did something (state/Thirst for Adventure changed) → again right away: Arena, then the next expedition.
            // If nothing changed, the Tavern is done ONLY when it was actually attempted (character was Idle and no
            // expedition fit the remaining Thirst for Adventure) – not when it was skipped because the character was
            // busy with something else (City Guard): that leftover Thirst must still get a real attempt once free
            // (bug found 2026-10-08: City Guard was restarting shift after shift without ever touching fresh,
            // post-midnight Thirst for Adventure, because being guard-busy was wrongly treated as "Tavern done").
            let after = session.game_state().map(tavern_state);
            if after.is_some_and(|a| a != before) {
                continue;
            }
            tavern_done = before.0 == CurrentAction::Idle;
        }

        // Shops once a day after the Tavern is done (before City Guard, the best items to sell are in by now)
        if let tavern::Outcome::SessionLost = shops::run(session, tavern_done).await {
            return tavern::Outcome::SessionLost;
        }

        // Goblin Gleeman tasks: chests, guild skill, attributes; shell game only after the Tavern and the shops
        if let tavern::Outcome::SessionLost = tasks::run(session, tavern_done).await {
            return tavern::Outcome::SessionLost;
        }

        // City Guard: pay for a finished shift, a new one when the Tavern is done (until midnight, max 10 h)
        if let tavern::Outcome::SessionLost = guard::run(session, tavern_done).await {
            return tavern::Outcome::SessionLost;
        }

        // Daily report for the character challenge at ~23:50 (roster/, local only)
        // Simulated duels of all challenge characters at 23:20 (win rate for the dashboard)
        if let Some(day) = tournament::due_today()
            && let tavern::Outcome::SessionLost = tournament::run(session, day).await
        {
            return tavern::Outcome::SessionLost;
        }
        // "Run end of day now" from the icon menu: duels + report as a preview (the 23:20/23:50 runs replace it)
        let manual = ctx::take_end_of_day_request();
        if manual {
            report!("[roster] Manual end of day (preview)");
            tournament::unlock_today();
            if let tavern::Outcome::SessionLost = tournament::run(session, tournament::today()).await {
                return tavern::Outcome::SessionLost;
            }
        }
        if roster::due() || manual {
            let fin = roster::due();
            match safe::send(session, Command::Update).await {
                Ok(gs) => {
                    let summary = roster::write_day(gs, fin);
                    report!("[roster] {} written: {summary}", if fin { "Daily report" } else { "Preview report" });
                }
                Err(e) => report!("[roster] Update before the daily report failed: {e}"),
            }
        }

        // Nothing to do: wait until the Arena or Dungeons become free (+ random margin), at most 30 min
        let gs = session.game_state();
        let arena = gs.and_then(arena::secs_until_ready).unwrap_or(30 * 60);
        let mut dungeon = gs.map_or(30 * 60, dungeons::secs_until_ready);
        if dungeon == 0 {
            // Dungeons "free" but no fight happened (full inventory etc.): next attempt only after RETRY_SEC
            dungeon = dungeons::RETRY_SEC;
        }
        let now = chrono::Local::now();
        let guard_done = gs.and_then(|gs| guard::secs_until_done(gs.tavern.current_action)).unwrap_or(30 * 60);
        // The Thirst for Adventure resets at midnight: wake up and start a new day
        let midnight = guard::secs_until_midnight(now);
        let daily = gs.and_then(daily::secs_until_ready).unwrap_or(30 * 60);
        let roster_due = roster::secs_until_due().unwrap_or(30 * 60).min(tournament::secs_until_due().unwrap_or(30 * 60));
        let wait = arena.min(dungeon).min(guard_done).min(midnight).min(daily).min(roster_due) + fastrand::u64(30..120);
        let wait = wait.clamp(60, 30 * 60);
        report!("Nothing to do, next check in {} min {} s", wait / 60, wait % 60);
        // A manual "end of day now" from the icon wakes the loop up early
        tokio::select! {
            () = tokio::time::sleep(std::time::Duration::from_secs(wait)) => {}
            () = ctx::EOD_WAKE.notified() => {}
        }
    }
}

/// How many times in a row we may log in again when the session drops right away (within SESSION_OK_SEC).
const MAX_RELOGINS: u32 = 3;
/// A session that lasted at least this long counts as healthy (the counter resets).
const SESSION_OK_SEC: u64 = 5 * 60;

/// Finds the project folder (with `.env`) upwards from the current folder and from the exe location and
/// switches to it, so the bot can be started from anywhere (desktop shortcut, double click on the exe in target\release).
fn enter_project_dir() {
    let mut starts = vec![];
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        starts.push(exe);
    }
    for start in starts {
        if let Some(dir) = start.ancestors().find(|d| d.join(".env").is_file()) {
            let _ = std::env::set_current_dir(dir);
            return;
        }
    }
}

fn main() -> ExitCode {
    // The release build has no console: show crashes in a dialog, otherwise they are invisible.
    std::panic::set_hook(Box::new(|info| {
        report!("PANIC: {info}");
        tray::message_box(&format!("StFuGAME crashed:\n{info}"));
    }));
    enter_project_dir();
    if tray::already_running() {
        tray::message_box("The StFuGAME bot is already running (icon next to the clock).");
        return ExitCode::FAILURE;
    }
    // .env is optional - the variables can also be set in the system
    let _ = dotenvy::dotenv();
    let accounts = match accounts() {
        Ok(a) => a,
        Err(e) => {
            report!("{e}");
            // The release build has no console: without this the app would just silently not start.
            tray::message_box(&format!("StFuGAME could not start:\n{e}\n\nCheck the .env file (see .env.example)."));
            return ExitCode::FAILURE;
        }
    };
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            report!("Could not start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The app window owns the one message loop (icon + window) and never returns.
    app::run(rt, accounts)
}

/// All accounts from the environment (.env): `SF_USER`/`SF_PASS`/`SF_CHARACTER` (optional, one account) and
/// `SF_ACCOUNTS` = `login|password|character;login|password|character;…` (the challenge characters).
/// `SF_SERVER` applies to all. Never logs the values.
///
/// Every character defaults to switched OFF (user 2026-10-08: opening the app must never start anything by
/// itself) – `control::should_autostart` only ever starts one that the user previously, explicitly switched on
/// (remembered in `roster/switches.json`, local only). There is no `.env`-level "start this one automatically".
pub(crate) fn accounts() -> Result<Vec<Credentials>, String> {
    let server = env_var("SF_SERVER").ok();
    let mut out = Vec::new();
    if let (Ok(user), Ok(pass), Ok(character)) = (env_var("SF_USER"), env_var("SF_PASS"), env_var("SF_CHARACTER")) {
        out.push(Credentials { user, pass, character, server: server.clone() });
    }
    if let Ok(list) = env_var("SF_ACCOUNTS") {
        for (i, entry) in list.split(';').map(str::trim).filter(|e| !e.is_empty()).enumerate() {
            let parts: Vec<&str> = entry.split('|').map(str::trim).collect();
            let [user, pass, character] = parts[..] else {
                return Err(format!("SF_ACCOUNTS: entry {} must be login|password|character", i + 1));
            };
            if out.iter().any(|c: &Credentials| c.character.eq_ignore_ascii_case(character)) {
                continue;
            }
            out.push(Credentials {
                user: user.to_string(),
                pass: pass.to_string(),
                character: character.to_string(),
                server: server.clone(),
            });
        }
    }
    if out.is_empty() {
        return Err("No account: set SF_USER, SF_PASS, SF_CHARACTER and/or SF_ACCOUNTS in the .env file".to_string());
    }
    Ok(out)
}

/// One character: login and main loop, logging in again after a lost session. Spawned by `control::start`.
pub(crate) async fn run_character(creds: Credentials) -> ExitCode {
    let mut journal = journal::Journal::default();
    // Only consecutive session losses count; when a session lasts, the counter resets
    let mut attempt = 0;
    let mut first = true;
    while attempt <= MAX_RELOGINS {
        if attempt > 0 {
            let wait = fastrand::u64(20..60);
            report!("Session expired, logging in again in {wait} s ({attempt}/{MAX_RELOGINS} in a row)");
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        }
        let mut session = match login(&creds).await {
            Ok(s) => s,
            Err(msg) => {
                report!("{msg}");
                return ExitCode::FAILURE;
            }
        };
        if first {
            print_status(&session);
            first = false;
        }
        let started = std::time::Instant::now();
        if let tavern::Outcome::SessionLost = play(&mut session, &mut journal).await {
            attempt = if started.elapsed().as_secs() >= SESSION_OK_SEC { 1 } else { attempt + 1 };
            continue;
        }
        return ExitCode::SUCCESS;
    }
    report!("The session was lost too often, stopping");
    ExitCode::FAILURE
}
