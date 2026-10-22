//! One place that answers "when does this character next have something to do?" (user 2026-10-14, backlog package C).
//! Used by the main loop's idle sleep and by the Tavern's expedition wait, so a character waiting for an expedition
//! also wakes up for the Arena, Dungeons, fortress, City Guard, daily rewards, reports and midnight – and the main
//! loop knows when the expedition ends.

use chrono::Local;
use sf_api::gamestate::{GameState, tavern::{CurrentAction, ExpeditionStage}};

use crate::{arena, daily, dungeons, fortress, guard, hunt, roster, tournament};

/// Nothing known: look again after this many seconds.
const FALLBACK_SEC: u64 = 30 * 60;

/// The nearest thing to wake up for.
#[derive(Debug, Clone, Copy)]
pub struct Wake {
    pub secs: u64,
    pub what: &'static str,
}

/// All due times of the character. `expedition` = also count the end of the current expedition wait (the Tavern's own
/// wait handles that one itself).
pub fn next(gs: &GameState, expedition: bool) -> Wake {
    let now = Local::now();
    let mut best = Wake { secs: FALLBACK_SEC, what: "fallback" };
    let mut put = |secs: Option<u64>, what: &'static str| {
        if let Some(secs) = secs
            && secs < best.secs
        {
            best = Wake { secs, what };
        }
    };
    put(
        arena::secs_until_ready(gs).or_else(|| hunt::wants_arena(gs).then(|| arena::secs_until_free(gs).max(60))),
        "Arena",
    );
    // Dungeons "free" but no fight happened (full inventory etc.): the next attempt only after RETRY_SEC
    put(Some(dungeons::secs_until_ready(gs)).map(|s| if s == 0 { dungeons::RETRY_SEC } else { s }), "Dungeons");
    put(crate::pets::secs_until_ready(gs).map(|s| if s == 0 { dungeons::RETRY_SEC } else { s }), "Pets");
    put(guard::secs_until_done(gs.tavern.current_action), "City Guard");
    put(Some(guard::secs_until_midnight(now)), "midnight");
    put(daily::secs_until_ready(gs), "daily reward");
    put(roster::secs_until_due(), "daily report");
    put(tournament::secs_until_due(), "duel round");
    put(fortress::secs_until_due(gs), "fortress");
    if expedition
        && gs.tavern.current_action == CurrentAction::Expedition
        && let Some(exp) = gs.tavern.expeditions.active()
        && let ExpeditionStage::Waiting { busy_until, .. } = exp.current_stage()
    {
        put(Some(u64::try_from((busy_until - now).num_seconds()).unwrap_or(0)), "expedition");
    }
    best
}
