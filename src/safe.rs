//! Jediné místo, přes které se posílají příkazy na server.
//! Hlídá, aby bot nikdy neutratil houby, a mezi akcemi dělá náhodné pauzy.

use std::{sync::Mutex, time::Duration};

use chrono::{DateTime, Local};
use sf_api::{SimpleSession, command::Command, error::SFError, gamestate::GameState};

/// Rezerva po konci cooldownu (aréna, podzemí), než smíme bojovat (hodiny serveru a naše se můžou lišit).
pub const ARENA_SAFETY_SEC: i64 = 30;

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
            | Command::UpdateDungeons
            | Command::FightDungeon { use_mushroom: false, .. }
    )
}

/// Druh boje s cooldownem. Na cooldownu by boj stál houbu (server příznak `use_mushroom` ignoruje).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cooldown {
    Arena,
    Dungeon,
}

/// Kdy jsme naposledy bojovali (za tohoto běhu bota).
static LAST_FIGHT: Mutex<Vec<(Cooldown, DateTime<Local>)>> = Mutex::new(Vec::new());

fn last_fight(kind: Cooldown) -> Option<DateTime<Local>> {
    LAST_FIGHT.lock().ok()?.iter().find(|(k, _)| *k == kind).map(|(_, t)| *t)
}

fn remember_fight(kind: Cooldown) {
    if let Ok(mut v) = LAST_FIGHT.lock() {
        v.retain(|(k, _)| *k != kind);
        v.push((kind, Local::now()));
    }
}

/// Je boj volný? Konec cooldownu (+ rezerva) musí být za námi, a pokud jsme už bojovali,
/// musí server mezitím poslat NOVÝ konec cooldownu (pozdější než náš boj). Jinak by mohl být
/// stav zastaralý (např. u podzemí se čas obnoví jen přes UpdateDungeons) a boj by stál houbu.
pub fn cooldown_free(kind: Cooldown, next_free: Option<DateTime<Local>>) -> bool {
    let last = last_fight(kind);
    match next_free {
        None => last.is_none(),
        Some(t) => Local::now() >= t + chrono::Duration::seconds(ARENA_SAFETY_SEC) && last.is_none_or(|l| t > l),
    }
}

fn fight_kind(cmd: &Command) -> Option<Cooldown> {
    match cmd {
        Command::Fight { .. } => Some(Cooldown::Arena),
        Command::FightDungeon { .. } => Some(Cooldown::Dungeon),
        _ => None,
    }
}

/// Je aréna podle stavu hry volná i s rezervou? Na cooldownu by boj stál houbu
/// (server příznak `use_mushroom` ignoruje a bojuje vždy).
pub fn arena_is_free(gs: &GameState) -> bool {
    cooldown_free(Cooldown::Arena, gs.arena.next_free_fight)
}

/// Je podzemí volné? Platí jen pro stav čerstvě po `UpdateDungeons`.
pub fn dungeon_is_free(gs: &GameState) -> bool {
    cooldown_free(Cooldown::Dungeon, gs.dungeons.next_free_fight)
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
    let kind = fight_kind(&cmd);
    match kind {
        Some(Cooldown::Arena) if !session.game_state().is_some_and(arena_is_free) => {
            return Err(SFError::InvalidRequest("aréna je na cooldownu, boj by stál houbu"));
        }
        Some(Cooldown::Dungeon) if !session.game_state().is_some_and(dungeon_is_free) => {
            return Err(SFError::InvalidRequest("podzemí je na cooldownu, boj by stál houbu"));
        }
        _ => {}
    }

    let mushrooms_before = session.game_state().map(|gs| gs.character.mushrooms);
    let res = session.send_command(cmd).await.map(|_| ());
    if let Some(kind) = kind {
        // I při chybě: server mohl boj provést, další boj až po novém čase ze serveru
        remember_fight(kind);
    }

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
