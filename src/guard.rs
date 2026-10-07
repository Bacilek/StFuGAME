//! Hlídka (městská stráž): když je hospoda dojetá, jít na hlídku. Nejvýš 10 h, ale tak,
//! aby skončila nejpozději o půlnoci (pak se resetuje ALU a začíná nový den úkolů).
//! Po skončení hlídky vyzvednout výplatu.

use chrono::{DateTime, Duration, Local, Timelike};
use sf_api::{SimpleSession, command::Command, gamestate::tavern::CurrentAction};

use crate::{safe, tavern::Outcome};

const MAX_HOURS: i64 = 10;

/// Kolik celých hodin hlídky se vejde do půlnoci (nejvýš 10).
pub fn hours_until_midnight(now: DateTime<Local>) -> u8 {
    let since_midnight = i64::from(now.num_seconds_from_midnight());
    let left = 24 * 3600 - since_midnight;
    u8::try_from((left / 3600).min(MAX_HOURS)).unwrap_or(0)
}

/// Za kolik sekund je půlnoc.
pub fn secs_until_midnight(now: DateTime<Local>) -> u64 {
    u64::from(24 * 3600 - now.num_seconds_from_midnight())
}

/// Za kolik sekund skončí běžící hlídka (None, když postava nehlídá).
pub fn secs_until_done(action: CurrentAction) -> Option<u64> {
    match action {
        CurrentAction::CityGuard { busy_until, .. } => {
            Some(u64::try_from((busy_until - Local::now()).num_seconds()).unwrap_or(0))
        }
        _ => None,
    }
}

fn chyba(e: &sf_api::error::SFError) -> Outcome {
    report!("[hlídka] Chyba: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Vyzvedne výplatu za skončenou hlídku; když je hospoda dojetá a postava nic nedělá, nastoupí na hlídku.
/// `tavern_done`: na expedici už není ALU (rozhoduje hlavní smyčka).
pub async fn run(session: &mut SimpleSession, tavern_done: bool) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };

    if let CurrentAction::CityGuard { busy_until, hours } = gs.tavern.current_action {
        if Local::now() < busy_until + Duration::seconds(10) {
            return Outcome::Done;
        }
        let silver_before = gs.character.silver;
        report!("[hlídka] Hlídka ({hours} h) skončila, vyzvedávám výplatu");
        let gs = match safe::send(session, Command::FinishWork).await {
            Ok(gs) => gs,
            Err(e) => return chyba(&e),
        };
        let earned = gs.character.silver.saturating_sub(silver_before);
        report!("[hlídka] Výplata {} g {} s", earned / 100, earned % 100);
    }

    let Some(gs) = session.game_state() else { return Outcome::Done };
    if !tavern_done || gs.tavern.current_action != CurrentAction::Idle {
        return Outcome::Done;
    }
    let hours = hours_until_midnight(Local::now());
    if hours == 0 {
        return Outcome::Done;
    }
    report!(
        "[hlídka] Hospoda dojetá, jdu na hlídku na {hours} h (mzda {} s/h)",
        gs.tavern.guard_wage
    );
    match safe::send(session, Command::StartWork { hours }).await {
        Ok(gs) => {
            if let CurrentAction::CityGuard { busy_until, .. } = gs.tavern.current_action {
                report!("[hlídka] Hlídka skončí v {}", busy_until.format("%H:%M"));
            }
            Outcome::Done
        }
        Err(e) => chyba(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 7, h, m, 0).unwrap()
    }

    #[test]
    fn guard_ends_before_midnight() {
        assert_eq!(hours_until_midnight(at(8, 0)), 10);
        assert_eq!(hours_until_midnight(at(14, 0)), 10);
        assert_eq!(hours_until_midnight(at(17, 5)), 6); // 6 h 55 min do půlnoci → 6 h
        assert_eq!(hours_until_midnight(at(22, 59)), 1);
        assert_eq!(hours_until_midnight(at(23, 30)), 0);
    }
}
