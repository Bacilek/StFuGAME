#[macro_use]
mod report;
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

#[tokio::main]
async fn main() -> ExitCode {
    // .env je volitelný - proměnné mohou být nastavené i v systému
    let _ = dotenvy::dotenv();

    let (user, pass, character) = match (env_var("SF_USER"), env_var("SF_PASS"), env_var("SF_CHARACTER")) {
        (Ok(u), Ok(p), Ok(c)) => (u, p, c),
        (u, p, c) => {
            for e in [u.err(), p.err(), c.err()].into_iter().flatten() {
                report!("{e}");
            }
            return ExitCode::FAILURE;
        }
    };
    // Volitelné: jen pokud máš postavu stejného jména na více serverech
    let server = env_var("SF_SERVER").ok();

    report!("Přihlašuji se k S&F účtu...");
    let sessions = match SimpleSession::login_sf_account(&user, &pass).await {
        Ok(s) => s,
        Err(e) => {
            report!("Přihlášení k účtu selhalo: {}", describe_login_error(&e));
            return ExitCode::FAILURE;
        }
    };

    let mut matching: Vec<SimpleSession> = sessions
        .into_iter()
        .filter(|s| s.username().eq_ignore_ascii_case(&character))
        .filter(|s| match &server {
            Some(srv) => s.server_url().host_str().is_some_and(|h| h.eq_ignore_ascii_case(srv)),
            None => true,
        })
        .collect();

    let mut session = match matching.len() {
        1 => matching.remove(0),
        0 => {
            report!("Postava {character} nebyla pod tímto účtem nalezena (zkontroluj SF_CHARACTER / SF_SERVER)");
            return ExitCode::FAILURE;
        }
        _ => {
            report!("Postav se jménem {character} je víc, upřesni server v SF_SERVER:");
            for s in &matching {
                report!("  {}", s.server_url().host_str().unwrap_or("?"));
            }
            return ExitCode::FAILURE;
        }
    };

    report!("Načítám postavu {character} na {}...", session.server_url().host_str().unwrap_or("?"));
    // Po přihlášení přes účet ještě nemáme stav hry - Update ho stáhne
    if let Err(e) = safe::send(&mut session, Command::Update).await {
        report!("Načtení postavy selhalo: {}", describe_login_error(&e));
        return ExitCode::FAILURE;
    }

    let Some(gs) = session.game_state() else {
        report!("Přihlášení proběhlo, ale server nevrátil stav hry");
        return ExitCode::FAILURE;
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

    tavern::run(&mut session).await;

    ExitCode::SUCCESS
}
