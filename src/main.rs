// Release build has no console window, it is controlled by the icon next to the clock
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod report;
mod arena;
mod daily;
mod dungeons;
mod guard;
mod guild;
mod inventory;
mod journal;
mod missions;
mod safe;
mod session;
mod shops;
mod stable;
mod tavern;
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

struct Credentials {
    user: String,
    pass: String,
    character: String,
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
        if let tavern::Outcome::SessionLost = daily::run(session).await {
            return tavern::Outcome::SessionLost;
        }
        if let tavern::Outcome::SessionLost = inventory::manage(session).await {
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
            // If nothing changed (the leftover Thirst for Adventure is not enough for any expedition), the Tavern is done.
            let after = session.game_state().map(tavern_state);
            if after.is_some_and(|a| a != before) {
                continue;
            }
            tavern_done = before.0 != CurrentAction::Expedition;
        }

        // Shops once a day after the Tavern is done (before City Guard, the best items to sell are in by now)
        if let tavern::Outcome::SessionLost = shops::run(session, tavern_done).await {
            return tavern::Outcome::SessionLost;
        }

        // City Guard: pay for a finished shift, a new one when the Tavern is done (until midnight, max 10 h)
        if let tavern::Outcome::SessionLost = guard::run(session, tavern_done).await {
            return tavern::Outcome::SessionLost;
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
        let wait = arena.min(dungeon).min(guard_done).min(midnight).min(daily) + fastrand::u64(30..120);
        let wait = wait.clamp(60, 30 * 60);
        report!("Nothing to do, next check in {} min {} s", wait / 60, wait % 60);
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
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
    enter_project_dir();
    if tray::already_running() {
        tray::message_box("The StFuGAME bot is already running (icon next to the clock).");
        return ExitCode::FAILURE;
    }
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            report!("Could not start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    tray::run(&rt, run_bot);
    ExitCode::SUCCESS
}

/// The whole bot: login and main loop. The icon starts and stops it.
async fn run_bot() -> ExitCode {
    // .env is optional - the variables can also be set in the system
    let _ = dotenvy::dotenv();

    let creds = match (env_var("SF_USER"), env_var("SF_PASS"), env_var("SF_CHARACTER")) {
        (Ok(user), Ok(pass), Ok(character)) => Credentials { user, pass, character, server: env_var("SF_SERVER").ok() },
        (u, p, c) => {
            for e in [u.err(), p.err(), c.err()].into_iter().flatten() {
                report!("{e}");
            }
            return ExitCode::FAILURE;
        }
    };

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
