//! Stable: make sure we have a mount before an expedition, so we never go on a mission without one.
//! It is bought only when really needed (no mount and heading to the Tavern), not right after
//! it expires – this saves mushrooms on days when the mount would not be used anyway.
//!
//! The ONLY allowed exception to the "never spend mushrooms" rule (user, 2026-10-07):
//! griffin/dragon (tier 4) for 25 mushrooms for 14 days; without enough mushrooms tiger/raptor (tier 3) for 10 g + 1 mushroom.

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

/// Retry a failed purchase at the earliest after this long.
const RETRY: Duration = Duration::from_secs(30 * 60);
static LAST_TRY: Mutex<Option<Instant>> = Mutex::new(None);

/// Is the character without a mount (none, or expired)?
pub fn needs_mount(gs: &GameState) -> bool {
    gs.character.mount.is_none() || gs.character.mount_end.is_none_or(|end| end <= Local::now())
}

/// Which mount to buy: griffin if there are enough mushrooms, otherwise tiger. None = cannot afford even the tiger.
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

/// Before an expedition: rent a mount if the character has none. Returns SessionLost when the session is lost.
pub async fn ensure_mount(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !needs_mount(gs) || !may_try() {
        return Outcome::Done;
    }
    let Some(mount) = choose(gs) else {
        report!(
            "[stable] WARNING: cannot afford a mount (mushrooms {}, gold {} g), going on the expedition without one",
            gs.character.mushrooms,
            gs.character.silver / 100
        );
        return Outcome::Done;
    };
    let cost = mount.cost();
    report!(
        "[stable] Character has no mount, renting {mount:?} for 14 days ({} mushrooms, {} g)",
        cost.mushrooms,
        cost.silver / 100
    );
    match safe::send(session, Command::BuyMount { mount }).await {
        Ok(gs) => {
            match (gs.character.mount, gs.character.mount_end) {
                (Some(m), Some(end)) => report!("[stable] Mount {m:?} until {}", end.format("%d.%m. %H:%M")),
                _ => report!("[stable] Purchase went through, but the server shows no mount"),
            }
            Outcome::Done
        }
        Err(e) => {
            report!("[stable] Error: {e}");
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
