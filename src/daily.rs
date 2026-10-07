//! Daily rewards: the daily login bonus (calendar) and one free Wheel of Fortune spin per day.
//! Nothing else (user rule). The wheel never for mushrooms or lucky coins.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use chrono::Local;
use sf_api::{
    command::{Command, FortunePayment},
};

use crate::session::SimpleSession;

use crate::{safe, tavern::Outcome};

/// When an action fails (state unchanged), retry it at the earliest after this long.
const RETRY: Duration = Duration::from_secs(30 * 60);

static TRIED: Mutex<Vec<(&'static str, Instant)>> = Mutex::new(Vec::new());

/// May the action be tried? Records the attempt (for retrying after a failure).
fn may_try(what: &'static str) -> bool {
    let Ok(mut tried) = TRIED.lock() else { return false };
    if tried.iter().any(|(w, t)| *w == what && t.elapsed() < RETRY) {
        return false;
    }
    tried.retain(|(w, _)| *w != what);
    tried.push((what, Instant::now()));
    true
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[rewards] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Collects everything available today. Sends nothing when there is nothing to collect.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let now = Local::now();
    let specials = &gs.specials;

    // Calendar: one reward per day
    let calendar_ready = specials.calendar.next_possible.is_some_and(|t| t <= now);
    if calendar_ready && may_try("calendar") {
        let reward = specials.calendar.rewards.get(specials.calendar.collected).map(|r| crate::report::reward(&format!("{:?}", r.typ), r.amount));
        report!("[rewards] Collecting the daily login bonus: {}", reward.unwrap_or_else(|| "?".into()));
        if let Err(e) = safe::send(session, Command::CollectCalendar).await {
            return fail(&e);
        }
    }

    // Wheel of Fortune: free spin only (never for mushrooms or lucky coins)
    if session.game_state().is_some_and(safe::wheel_is_free) && may_try("wheel") {
        report!("[rewards] Spinning the Wheel of Fortune (free)");
        match safe::send(session, Command::SpinWheelOfFortune { payment: FortunePayment::FreeTurn }).await {
            Ok(gs) => match &gs.specials.wheel.result {
                Some(w) => report!("[rewards] Wheel of Fortune: {}", crate::report::reward(&format!("{:?}", w.typ), w.amount)),
                None => report!("[rewards] Wheel of Fortune: no result from the server"),
            },
            Err(e) => return fail(&e),
        }
    }

    Outcome::Done
}

/// In how many seconds something can be collected (calendar or free spin), if known.
pub fn secs_until_ready(gs: &sf_api::gamestate::GameState) -> Option<u64> {
    let now = Local::now();
    [gs.specials.calendar.next_possible, gs.specials.wheel.next_free_spin]
        .into_iter()
        .flatten()
        .filter(|t| *t > now)
        .map(|t| u64::try_from((t - now).num_seconds()).unwrap_or(0))
        .min()
}
