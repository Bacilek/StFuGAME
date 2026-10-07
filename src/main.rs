#[macro_use]
mod report;
mod arena;
mod dungeons;
mod journal;
mod missions;
mod safe;
mod tavern;

use std::process::ExitCode;

use sf_api::{SimpleSession, command::Command, error::SFError, gamestate::tavern::CurrentAction};

/// Načte povinnou proměnnou z prostředí (.env). Hodnotu nikdy nevypisuje.
fn env_var(name: &str) -> Result<String, String> {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => Ok(v.trim().to_string()),
        _ => Err(format!("Chybí proměnná {name} v souboru .env")),
    }
}

/// Srozumitelný popis chyby přihlášení. Neobsahuje heslo.
fn describe_login_error(err: &SFError) -> String {
    match err {
        SFError::ServerError(msg) => format!("Server odmítl přihlášení: {msg} (zkontroluj jméno/heslo/server)"),
        SFError::ConnectionError => "Nepodařilo se spojit se serverem (špatná adresa SF_SERVER nebo nefunguje připojení)".into(),
        SFError::EmptyResponse => "Server vrátil prázdnou odpověď".into(),
        SFError::UnsupportedVersion(v) => format!("Nepodporovaná verze serveru: {v}"),
        SFError::ParsingError(..) | SFError::TooShortResponse { .. } => {
            "Nepodařilo se zpracovat odpověď serveru (možná nová verze hry, zkus aktualizovat sf-api)".into()
        }
        other => format!("Neznámá chyba: {other}"),
    }
}

struct Credentials {
    user: String,
    pass: String,
    character: String,
    /// Volitelné: jen pokud máš postavu stejného jména na více serverech
    server: Option<String>,
}

/// Přihlásí se přes S&F účet, najde postavu a stáhne její stav.
async fn login(c: &Credentials) -> Result<SimpleSession, String> {
    report!("Přihlašuji se k S&F účtu...");
    let sessions = SimpleSession::login_sf_account(&c.user, &c.pass)
        .await
        .map_err(|e| format!("Přihlášení k účtu selhalo: {}", describe_login_error(&e)))?;

    let mut matching: Vec<SimpleSession> = sessions
        .into_iter()
        .filter(|s| s.username().eq_ignore_ascii_case(&c.character))
        .filter(|s| match &c.server {
            Some(srv) => s.server_url().host_str().is_some_and(|h| h.eq_ignore_ascii_case(srv)),
            None => true,
        })
        .collect();

    let mut session = match matching.len() {
        1 => matching.remove(0),
        0 => {
            return Err(format!(
                "Postava {} nebyla pod tímto účtem nalezena (zkontroluj SF_CHARACTER / SF_SERVER)",
                c.character
            ));
        }
        _ => {
            let servers: Vec<&str> = matching.iter().map(|s| s.server_url().host_str().unwrap_or("?")).collect();
            return Err(format!(
                "Postav se jménem {} je víc, upřesni server v SF_SERVER: {}",
                c.character,
                servers.join(", ")
            ));
        }
    };

    report!("Načítám postavu {} na {}...", c.character, session.server_url().host_str().unwrap_or("?"));
    // Po přihlášení přes účet ještě nemáme stav hry - Update ho stáhne
    safe::send(&mut session, Command::Update)
        .await
        .map_err(|e| format!("Načtení postavy selhalo: {}", describe_login_error(&e)))?;
    Ok(session)
}

fn print_status(session: &SimpleSession) {
    let Some(gs) = session.game_state() else {
        report!("Server nevrátil stav hry");
        return;
    };
    let ch = &gs.character;
    let tavern = &gs.tavern;
    let alu = tavern.thirst_for_adventure_sec;

    report!("Postava:  {}", ch.name);
    report!("Level:    {}", ch.level);
    report!("Zlato:    {} g {} s", ch.silver / 100, ch.silver % 100);
    report!("Houby:    {}", ch.mushrooms);
    report!("Hospoda:");
    report!("  ALU:    {} min {} s", alu / 60, alu % 60);
    report!("  Piva:   {}/{}", tavern.beer_drunk, tavern.beer_max);
    let action = match &tavern.current_action {
        CurrentAction::Idle => "nic nedělá".to_string(),
        CurrentAction::Quest { busy_until, .. } => format!("na výpravě do {}", busy_until.format("%H:%M:%S")),
        CurrentAction::CityGuard { busy_until, .. } => format!("hlídka do {}", busy_until.format("%H:%M:%S")),
        CurrentAction::Expedition => "na expedici".to_string(),
        CurrentAction::Unknown(_) => "neznámá činnost".to_string(),
    };
    report!("  Stav:   {action}");
}

/// Hlavní smyčka: aréna (když je volná, max 10 výher denně), podzemí (když je volné), hospoda (jedna expedice),
/// a když není co dělat, čekání na konec nejbližšího cooldownu. Běží, dokud ji nezastavíme.
async fn play(session: &mut SimpleSession, journal: &mut journal::Journal) -> tavern::Outcome {
    let mut last_dungeon_try: Option<std::time::Instant> = None;
    loop {
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
        if before.0 == CurrentAction::Expedition || before.1 > 0 {
            if let tavern::Outcome::SessionLost = tavern::run(session, journal).await {
                return tavern::Outcome::SessionLost;
            }
            // Hospoda něco odehrála (změnil se stav/ALU) → hned znovu: aréna, pak další expedice.
            // Když se nic nezměnilo (zbytek ALU na žádnou expedici nestačí), jdeme čekat.
            let after = session.game_state().map(tavern_state);
            if after.is_some_and(|a| a != before) {
                continue;
            }
        }

        // Není co dělat: počkat, až se uvolní aréna nebo podzemí (+ náhodná rezerva), nejvýš 30 min
        let gs = session.game_state();
        let arena = gs.and_then(arena::secs_until_ready).unwrap_or(30 * 60);
        let mut dungeon = gs.map_or(30 * 60, dungeons::secs_until_ready);
        if dungeon == 0 {
            // Podzemí „volné“, ale boj neproběhl (plný inventář apod.): další pokus až za RETRY_SEC
            dungeon = dungeons::RETRY_SEC;
        }
        let wait = arena.min(dungeon) + fastrand::u64(30..120);
        let wait = wait.clamp(60, 30 * 60);
        report!("Není co dělat, další kontrola za {} min {} s", wait / 60, wait % 60);
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
    }
}

/// Kolikrát se za jeden běh smíme znovu přihlásit po ztrátě session.
const MAX_RELOGINS: u32 = 3;

#[tokio::main]
async fn main() -> ExitCode {
    // .env je volitelný - proměnné mohou být nastavené i v systému
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
    for attempt in 0..=MAX_RELOGINS {
        if attempt > 0 {
            let wait = fastrand::u64(20..60);
            report!("Session vypršela (třeba kvůli přihlášení na účet jinde), za {wait} s se přihlásím znovu ({attempt}/{MAX_RELOGINS})");
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        }
        let mut session = match login(&creds).await {
            Ok(s) => s,
            Err(msg) => {
                report!("{msg}");
                return ExitCode::FAILURE;
            }
        };
        if attempt == 0 {
            print_status(&session);
        }
        if let tavern::Outcome::SessionLost = play(&mut session, &mut journal).await {
            continue;
        }
        return ExitCode::SUCCESS;
    }
    report!("Session se ztratila příliš často, končím");
    ExitCode::FAILURE
}
