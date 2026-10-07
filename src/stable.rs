//! Stáj: před expedicí zajistit zvíře, ať nikdy nejdeme na misi bez něj.
//! Kupuje se až když je opravdu potřeba (postava je bez zvířete a jde do hospody), ne hned
//! po vypršení – ušetří se houby za dny, kdy by zvíře stejně nevyužila.
//!
//! JEDINÁ povolená výjimka z pravidla „nikdy neutrácet houby“ (uživatel 2026-10-07):
//! gryf/drak (tier 4) za 25 hub na 14 dní; když hub není dost, tygr/raptor (tier 3) za 10 g + 1 houbu.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use chrono::Local;
use sf_api::{
    SimpleSession,
    command::Command,
    gamestate::{GameState, character::Mount},
};

use crate::{safe, tavern::Outcome};

/// Nepovedený nákup zkusit znovu nejdřív za tuto dobu.
const RETRY: Duration = Duration::from_secs(30 * 60);
static LAST_TRY: Mutex<Option<Instant>> = Mutex::new(None);

/// Nemá postava zvíře (žádné, nebo vypršelo)?
pub fn needs_mount(gs: &GameState) -> bool {
    gs.character.mount.is_none() || gs.character.mount_end.is_none_or(|end| end <= Local::now())
}

/// Které zvíře koupit: gryf, když je dost hub, jinak tygr. None = nemáme ani na tygra.
pub fn choose(gs: &GameState) -> Option<Mount> {
    let can_afford = |m: Mount| {
        let c = m.cost();
        u64::from(gs.character.mushrooms) >= u64::from(c.mushrooms) && gs.character.silver >= c.silver
    };
    [Mount::Dragon, Mount::Tiger].into_iter().find(|m| can_afford(*m))
}

fn may_try() -> bool {
    let Ok(mut last) = LAST_TRY.lock() else { return false };
    if last.is_some_and(|t| t.elapsed() < RETRY) {
        return false;
    }
    *last = Some(Instant::now());
    true
}

/// Před expedicí: když postava nemá zvíře, pronajmout ho. Vrací SessionLost při ztrátě session.
pub async fn ensure_mount(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !needs_mount(gs) || !may_try() {
        return Outcome::Done;
    }
    let Some(mount) = choose(gs) else {
        report!(
            "[stáj] POZOR: na zvíře nemáme (houby {}, zlato {} g), jdu na expedici bez něj",
            gs.character.mushrooms,
            gs.character.silver / 100
        );
        return Outcome::Done;
    };
    let cost = mount.cost();
    report!(
        "[stáj] Postava je bez zvířete, pronajímám {mount:?} na 14 dní ({} hub, {} g)",
        cost.mushrooms,
        cost.silver / 100
    );
    match safe::send(session, Command::BuyMount { mount }).await {
        Ok(gs) => {
            match (gs.character.mount, gs.character.mount_end) {
                (Some(m), Some(end)) => report!("[stáj] Zvíře {m:?} do {}", end.format("%d.%m. %H:%M")),
                _ => report!("[stáj] Nákup proběhl, ale server zvíře neukazuje"),
            }
            Outcome::Done
        }
        Err(e) => {
            report!("[stáj] Chyba: {e}");
            if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gs(mushrooms: u32, silver: u64) -> GameState {
        let mut gs = GameState::default();
        gs.character.mushrooms = mushrooms;
        gs.character.silver = silver;
        gs
    }

    #[test]
    fn dragon_then_tiger_then_nothing() {
        assert_eq!(choose(&gs(25, 0)), Some(Mount::Dragon));
        assert_eq!(choose(&gs(24, 1000)), Some(Mount::Tiger));
        assert_eq!(choose(&gs(0, 100_000)), None);
        assert_eq!(choose(&gs(1, 999)), None);
    }

    #[test]
    fn needs_mount_without_or_expired() {
        let mut g = gs(0, 0);
        assert!(needs_mount(&g));
        g.character.mount = Some(Mount::Dragon);
        g.character.mount_end = Some(Local::now() + chrono::Duration::days(3));
        assert!(!needs_mount(&g));
        g.character.mount_end = Some(Local::now() - chrono::Duration::minutes(1));
        assert!(needs_mount(&g));
    }
}
