//! City Guard: once the Tavern is done, go on guard duty. At most 10 h, but so that it ends
//! in the first hour after midnight (00:00–00:59): then the Thirst for Adventure resets and a new
//! day of tasks begins, and the bot does not idle waiting for the new Thirst for Adventure.
//! Collect the pay when the shift ends.

use chrono::{DateTime, Duration, Local, Timelike};
use sf_api::{SimpleSession, command::Command, gamestate::tavern::CurrentAction};

use crate::{safe, tavern::Outcome};

const MAX_HOURS: i64 = 10;

/// Shift length: hours until midnight rounded up (ends 00:00–00:59), at most 10 h.
pub fn guard_hours(now: DateTime<Local>) -> u8 {
    let since_midnight = i64::from(now.num_seconds_from_midnight());
    let left = 24 * 3600 - since_midnight;
    let hours = (left + 3599) / 3600;
    u8::try_from(hours.clamp(1, MAX_HOURS)).unwrap_or(1)
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
pub async fn run(session: &mut SimpleSession, tavern_done: bool) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };

    if let CurrentAction::CityGuard { busy_until, hours } = gs.tavern.current_action {
        if Local::now() < busy_until + Duration::seconds(10) {
            return Outcome::Done;
        }
        let silver_before = gs.character.silver;
        report!("[guard] Shift ({hours} h) finished, collecting the pay");
        let gs = match safe::send(session, Command::FinishWork).await {
            Ok(gs) => gs,
            Err(e) => return fail(&e),
        };
        let earned = gs.character.silver.saturating_sub(silver_before);
        report!("[guard] Pay {}", crate::report::gold(earned));
    }

    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !tavern_done || gs.tavern.current_action != CurrentAction::Idle {
        return Outcome::Done;
    }
    let hours = guard_hours(Local::now());
    report!(
        "[guard] Tavern done, starting a {hours} h City Guard shift (wage {} s/h)",
        gs.tavern.guard_wage
    );
    match safe::send(session, Command::StartWork { hours }).await {
        Ok(gs) => {
            if let CurrentAction::CityGuard { busy_until, .. } = gs.tavern.current_action {
                report!("[guard] Shift ends at {}", busy_until.format("%H:%M"));
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

    /// Shift ends 00:00–00:59 (user rule), at most 10 h.
    #[test]
    fn guard_ends_in_first_hour_after_midnight() {
        assert_eq!(guard_hours(at(8, 0)), 10);
        assert_eq!(guard_hours(at(14, 0)), 10);
        assert_eq!(guard_hours(at(17, 5)), 7); // ends 00:05
        assert_eq!(guard_hours(at(17, 0)), 7); // exactly at midnight
        assert_eq!(guard_hours(at(22, 59)), 2); // ends 00:59
        assert_eq!(guard_hours(at(23, 30)), 1); // ends 00:30
    }
}
