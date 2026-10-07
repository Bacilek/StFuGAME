//! Jediné místo, přes které se posílají příkazy na server.
//! Hlídá, aby bot nikdy neutratil houby, a mezi akcemi dělá náhodné pauzy.

use std::time::Duration;

use sf_api::{SimpleSession, command::Command, error::SFError, gamestate::GameState};

/// Příkazy, které bot smí poslat. Vše ostatní je zakázané (whitelist),
/// takže nový příkaz se k serveru nedostane, dokud ho sem vědomě nepřidáme.
/// Žádný z nich neutrácí houby.
fn is_allowed(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Update
            | Command::ExpeditionStart { .. }
            | Command::ExpeditionPickEncounter { .. }
            | Command::ExpeditionContinue
            | Command::ExpeditionPickReward { .. }
    )
}

/// Náhodná pauza mezi akcemi (simulace člověka).
pub async fn human_pause() {
    tokio::time::sleep(Duration::from_millis(fastrand::u64(2500..7000))).await;
}

/// Pošle příkaz, pokud je povolený, a pak chvíli počká.
pub async fn send<'a>(session: &'a mut SimpleSession, cmd: Command) -> Result<&'a mut GameState, SFError> {
    if !is_allowed(&cmd) {
        return Err(SFError::InvalidRequest("příkaz není na seznamu povolených (ochrana hub)"));
    }
    let res = session.send_command(cmd).await;
    human_pause().await;
    res
}
