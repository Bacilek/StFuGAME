//! Jediné místo, přes které se posílají příkazy na server.
//! Hlídá, aby bot nikdy neutratil houby, a mezi akcemi dělá náhodné pauzy.

use std::time::Duration;

use chrono::Local;
use sf_api::{SimpleSession, command::Command, error::SFError, gamestate::GameState};

/// Rezerva po konci cooldownu arény, než ji smíme použít (hodiny serveru a naše se můžou lišit).
const ARENA_SAFETY_SEC: i64 = 30;

/// Příkazy, které bot smí poslat. Vše ostatní je zakázané (whitelist),
/// takže nový příkaz se k serveru nedostane, dokud ho sem vědomě nepřidáme.
/// Žádný z nich neutrácí houby (`Fight` jen mimo cooldown, viz `arena_is_free`).
fn is_allowed(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Update
            | Command::ExpeditionStart { .. }
            | Command::ExpeditionPickEncounter { .. }
            | Command::ExpeditionContinue
            | Command::ExpeditionPickReward { .. }
            | Command::CheckArena
            | Command::ViewPlayer { .. }
            | Command::Fight { use_mushroom: false, .. }
    )
}

/// Je aréna podle stavu hry volná i s rezervou? Na cooldownu by boj stál houbu
/// (server příznak `use_mushroom` ignoruje a bojuje vždy).
pub fn arena_is_free(gs: &GameState) -> bool {
    match gs.arena.next_free_fight {
        None => true,
        Some(t) => Local::now() >= t + chrono::Duration::seconds(ARENA_SAFETY_SEC),
    }
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
    if matches!(cmd, Command::Fight { .. }) && !session.game_state().is_some_and(arena_is_free) {
        return Err(SFError::InvalidRequest("aréna je na cooldownu, boj by stál houbu"));
    }

    let mushrooms_before = session.game_state().map(|gs| gs.character.mushrooms);
    let res = session.send_command(cmd).await.map(|_| ());

    // Poslední pojistka: kdyby houby přesto ubyly, bot okamžitě končí
    if let (Some(before), Some(after)) = (mushrooms_before, session.game_state().map(|gs| gs.character.mushrooms))
        && after < before
    {
        report!("!!! UBYLY HOUBY ({before} → {after}). Bot se okamžitě zastavuje, prověřit!");
        std::process::exit(2);
    }

    human_pause().await;
    res?;
    session.game_state_mut().ok_or(SFError::EmptyResponse)
}
