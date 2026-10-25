//! City Guard: once the Tavern is done, go on guard duty. At most 10 h, but capped at a 23:00
//! checkpoint (user 2026-10-08) so a beer-granted bonus Thirst for Adventure right around that
//! time still has a real chance to be spent in the Tavern before the midnight reset, instead of
//! sitting unused for the rest of a shift running past midnight. A shift starting at/after the
//! checkpoint falls back to the old behavior (ends 00:00–00:59) – this is what bridges the gap
//! to the new day on days when no beer is needed, with no extra code: see `guard_hours`.
//! Collect the pay when the shift ends.

use chrono::{DateTime, Duration, Local, Timelike};
use sf_api::{command::Command, gamestate::tavern::CurrentAction};

use crate::session::SimpleSession;

use crate::{safe, tavern::Outcome};

const MAX_HOURS: i64 = 10;
/// Local hour at which a shift that would otherwise run past midnight is capped instead
/// (user 2026-10-08, see module doc).
const CHECKPOINT_HOUR: i64 = 23;

/// Shift length: at most 10 h, capped at the 23:00 checkpoint when starting before it,
/// otherwise (already past the checkpoint) hours until midnight rounded up (ends 00:00–00:59).
pub fn guard_hours(now: DateTime<Local>) -> u8 {
    let since_midnight = i64::from(now.num_seconds_from_midnight());
    let checkpoint_secs = CHECKPOINT_HOUR * 3600;
    let left = if since_midnight < checkpoint_secs {
        checkpoint_secs - since_midnight
    } else {
        24 * 3600 - since_midnight
    };
    let hours = (left + 3599) / 3600;
    u8::try_from(hours.clamp(1, MAX_HOURS)).unwrap_or(1)
}

/// Permanent journal of the shifts (roster/<character>/logs/guard.log, never trimmed, local only), so a gap in the
/// day can be explained later: the progress log only keeps the last 100 lines.
fn journal(msg: &str) {
    use std::io::Write;
    let path = crate::ctx::log_path("guard.log");
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {msg}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// Seconds until midnight.
pub fn secs_until_midnight(now: DateTime<Local>) -> u64 {
    u64::from(24 * 3600 - now.num_seconds_from_midnight())
}

/// Seconds until the running shift ends (None when not on guard duty).
pub fn secs_until_done(action: CurrentAction) -> Option<u64> {
    match action {
        CurrentAction::CityGuard { busy_until, .. } => {
            Some(u64::try_from((busy_until - Local::now()).num_seconds()).unwrap_or(0))
        }
        _ => None,
    }
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[guard] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Collects the pay for a finished shift; when the Tavern is done and the character is idle, starts a new shift.
/// `tavern_done`: no Thirst for Adventure left for another expedition (decided by the main loop).
pub async fn run(session: &mut SimpleSession, tavern_done: bool, pet_wait: Option<DateTime<Local>>) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };

    if let CurrentAction::CityGuard { busy_until, hours } = gs.tavern.current_action {
        if Local::now() < busy_until + Duration::seconds(10) {
            return Outcome::Done;
        }
        let silver_before = gs.character.silver;
        report!("[guard] Shift ({hours} h) finished, collecting the pay");
        journal(&format!("shift of {hours} h finished (was due {})", busy_until.format("%H:%M:%S")));
        let gs = match safe::send(session, Command::FinishWork).await {
            Ok(gs) => gs,
            Err(e) => return fail(&e),
        };
        let earned = gs.character.silver.saturating_sub(silver_before);
        report!("[guard] Pay {}", crate::report::gold(earned));
        journal(&format!("pay {}", crate::report::gold(earned)));
        // Exceptional XP event (user 2026-10-16): the beer comes before the next shift; the Tavern then uses its Thirst
        if crate::tasks::beer_justified(gs) {
            return crate::tasks::event_beer(session).await;
        }
    }

    let Some(gs) = session.game_state() else { return Outcome::Done };
    // Re-check Thirst for Adventure fresh here, not just the `tavern_done` passed in from the
    // top of this loop pass: if `tasks::run` drank a beer earlier in this same pass (granting
    // bonus ALU), `tavern_done` is stale and would otherwise let a new shift start right on top
    // of it (found 2026-10-08). A fresh zero here means the Tavern genuinely has nothing left to
    // do; non-zero means the next pass's Tavern section should get first crack at it instead.
    // Waiting for a pet window (user 2026-10-26): the Thirst for Adventure is kept on purpose, so a shift that ends by the window fills the wait.
    if let Some(window) = pet_wait {
        if gs.tavern.current_action != CurrentAction::Idle {
            return Outcome::Done;
        }
        let hours = u8::try_from(((window - Local::now()).num_seconds() / 3600).clamp(0, i64::from(MAX_HOURS))).unwrap_or(0);
        if hours == 0 {
            return Outcome::Done;
        }
        report!("[guard] Waiting for a pet window at {}, starting a {hours} h City Guard shift", window.format("%H:%M"));
        return start_shift(session, hours).await;
    }
    if !tavern_done
        || gs.tavern.current_action != CurrentAction::Idle
        || gs.tavern.thirst_for_adventure_sec > 0
    {
        return Outcome::Done;
    }
    let hours = guard_hours(Local::now());
    report!(
        "[guard] Tavern done, starting a {hours} h City Guard shift (wage {} s/h)",
        gs.tavern.guard_wage
    );
    start_shift(session, hours).await
}

async fn start_shift(session: &mut SimpleSession, hours: u8) -> Outcome {
    match safe::send(session, Command::StartWork { hours }).await {
        Ok(gs) => {
            if let CurrentAction::CityGuard { busy_until, .. } = gs.tavern.current_action {
                report!("[guard] Shift ends at {}", busy_until.format("%H:%M"));
                journal(&format!("shift of {hours} h started, ends {}", busy_until.format("%H:%M:%S")));
            }
            Outcome::Done
        }
        Err(e) => fail(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 7, h, m, 0).unwrap()
    }

    /// A shift starting before the 23:00 checkpoint is capped there (at most 10 h either way).
    #[test]
    fn guard_caps_at_the_checkpoint() {
        assert_eq!(guard_hours(at(8, 0)), 10); // would be 15h to the checkpoint, capped at MAX_HOURS
        assert_eq!(guard_hours(at(14, 0)), 9); // ends 23:00
        assert_eq!(guard_hours(at(17, 5)), 6); // ends 23:05
        assert_eq!(guard_hours(at(17, 0)), 6); // ends 23:00 exactly
        assert_eq!(guard_hours(at(22, 59)), 1); // ends 23:59 (can't undershoot a whole hour)
        assert_eq!(guard_hours(at(20, 0)), 3); // ends 23:00 exactly
    }

    /// A shift starting at/after the checkpoint falls back to the old rule (ends 00:00–00:59) –
    /// this is the "bridging" shift to the new day on days when beer was never needed.
    #[test]
    fn guard_bridges_to_midnight_once_past_the_checkpoint() {
        assert_eq!(guard_hours(at(23, 0)), 1); // ends 00:00
        assert_eq!(guard_hours(at(23, 30)), 1); // ends 00:30
    }
}
