//! Arena Manager (idle game, level 105+, user 2026-10-25/31, `docs/idle.md`): spends the dollars on building upgrades. Dollars are
//! credited by time. `IdleUpgrade` and, by the user's rule (2026-11-05, `sacrifice_due`), `IdleSacrifice` (prestige); never the merchant (mushrooms).
//! Strategy: always upgrade the building whose next level is cheapest, in the largest batch (100/25/10/1) that is still affordable.

use sf_api::{
    command::{Command, IdleUpgradeAmount},
    gamestate::{GameState, idle::IdleBuildingType},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// Unlock level of the Arena Manager.
pub const MIN_LEVEL: u16 = 105;
/// Upgrade commands per pass and the pause between passes.
const MAX_PER_PASS: usize = 15;
const PASS_EVERY_SEC: u64 = 20 * 60;

static LAST_PASS: crate::ctx::PerChar<Option<std::time::Instant>> = crate::ctx::PerChar::new();
static WARNED: crate::ctx::PerChar<bool> = crate::ctx::PerChar::new();

fn num(b: &impl ToString) -> f64 {
    b.to_string().parse::<f64>().unwrap_or(f64::MAX)
}

/// The prestige rule (user 2026-11-05): sacrifice as soon as the runes it would give equal the runes we already have in total, the first one at 20
/// (20 -> 40 -> 80 ...; when the exact number is missed, the first moment beyond it: 22 -> 44+ ...). The runes are never spent.
pub fn sacrifice_due(gs: &GameState) -> bool {
    let Some(idle) = gs.idle_game.as_ref() else { return false };
    rune_rule(num(&idle.sacrifice_runes), num(&idle.current_runes))
}

fn rune_rule(gain: f64, owned: f64) -> bool {
    gain > 0.0 && gain >= owned.max(FIRST_SACRIFICE_RUNES)
}

const FIRST_SACRIFICE_RUNES: f64 = 20.0;
static STATE_SHOWN: crate::ctx::PerChar<bool> = crate::ctx::PerChar::new();

/// Persistent journal (`idle.log`, not rotated like the progress log): the state at every pass and every sacrifice.
fn journal(msg: &str) {
    use std::io::Write;
    let path = crate::ctx::log_path("idle.log");
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

use IdleBuildingType::*;

/// The user's upgrade order (2026-11-07): building levels are breakpoints (25, 50, 100, 250, 500, ...) where the earning doubles and the cycle
/// shortens, a jump bigger than the cost efficiency of any single upgrade. A staggered wave: each building starts two steps behind the previous one.
/// The bot works down the list, buys exactly up to the next unreached target (never past it) and invests nowhere else while it saves for it.
/// TO EXTEND: the user adds the rest (VIPSeat, Snacks, StrayingMonsters, Toilet, higher breakpoints). Past the end the old rule applies
/// (the building with the cheapest next level).
const STEPS: &[(IdleBuildingType, u32)] = &[
    (Seat, 25), (Seat, 50), (PopcornStand, 25), (Seat, 100), (PopcornStand, 50), (PopcornStand, 100),
    (ParkingLot, 25), (ParkingLot, 50), (Seat, 250), (ParkingLot, 100), (Trap, 25), (PopcornStand, 250),
    (Trap, 50), (Trap, 100), (Drinks, 25), (ParkingLot, 250), (Drinks, 50), (Drinks, 100),
    (DeadlyTrap, 25), (Seat, 500), (Trap, 250), (DeadlyTrap, 50), (PopcornStand, 500), (DeadlyTrap, 100),
];

/// The first step whose building has not reached its target yet: (building, levels still missing).
fn next_step(steps: &[(IdleBuildingType, u32)], level: impl Fn(IdleBuildingType) -> u32) -> Option<(IdleBuildingType, u32)> {
    steps.iter().find_map(|&(typ, target)| target.checked_sub(level(typ)).filter(|m| *m > 0).map(|m| (typ, m)))
}

type Batch = (IdleUpgradeAmount, u32, f64);

/// The biggest batch that does not exceed the `missing` levels and is affordable: (amount, levels, price).
fn pick_batch(missing: u32, money: f64, batches: [Batch; 4]) -> Option<Batch> {
    batches.into_iter().find(|&(_, n, p)| n <= missing && p <= money)
}

fn batches(b: &sf_api::gamestate::idle::IdleBuilding) -> [Batch; 4] {
    [
        (IdleUpgradeAmount::Hundred, 100, num(&b.upgrade_cost_100x)),
        (IdleUpgradeAmount::TwentyFive, 25, num(&b.upgrade_cost_25x)),
        (IdleUpgradeAmount::Ten, 10, num(&b.upgrade_cost_10x)),
        (IdleUpgradeAmount::One, 1, num(&b.upgrade_cost)),
    ]
}

/// What to buy now: (building, batch). While the list has an unreached step ONLY that step is bought (None = save up); past the end of the
/// list the building with the cheapest next level, in the biggest affordable batch.
fn next_purchase(gs: &GameState) -> Option<(IdleBuildingType, Batch)> {
    let idle = gs.idle_game.as_ref()?;
    let money = num(&idle.current_money);
    if let Some((typ, missing)) = next_step(STEPS, |t| idle.buildings[t].level) {
        return Some((typ, pick_batch(missing, money, batches(&idle.buildings[typ]))?));
    }
    let (typ, b) = idle.buildings.iter().min_by(|a, b| num(&a.1.upgrade_cost).total_cmp(&num(&b.1.upgrade_cost)))?;
    Some((typ, pick_batch(u32::MAX, money, batches(b))?))
}

/// The guard for `safe.rs`: the Arena Manager exists and the chosen batch is affordable.
pub fn upgrade_ok(gs: &GameState, typ: IdleBuildingType, amount: IdleUpgradeAmount) -> bool {
    let Some(idle) = gs.idle_game.as_ref() else { return false };
    let b = &idle.buildings[typ];
    let price = match amount {
        IdleUpgradeAmount::One => &b.upgrade_cost,
        IdleUpgradeAmount::Ten => &b.upgrade_cost_10x,
        IdleUpgradeAmount::TwentyFive => &b.upgrade_cost_25x,
        IdleUpgradeAmount::Hundred => &b.upgrade_cost_100x,
        IdleUpgradeAmount::Max => return false,
    };
    num(price) <= num(&idle.current_money)
}

pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    if gs.character.level < MIN_LEVEL {
        return Outcome::Done;
    }
    if gs.idle_game.is_none() {
        if let Ok(mut w) = WARNED.lock()
            && !*w
        {
            *w = true;
            report!("[idle] Arena Manager: level {} but the server sent no idle game (unlock pending?)", gs.character.level);
        }
        return Outcome::Done;
    }
    if let Some(idle) = gs.idle_game.as_ref()
        && STATE_SHOWN.lock().is_ok_and(|mut s| !std::mem::replace(&mut *s, true))
    {
        report!("[idle] Arena Manager: runes {}, a sacrifice would give {}, resets {}", idle.current_runes, idle.sacrifice_runes, idle.resets);
    }
    if LAST_PASS.lock().is_ok_and(|l| l.is_some_and(|t| t.elapsed().as_secs() < PASS_EVERY_SEC)) {
        return Outcome::Done;
    }
    if let Ok(mut l) = LAST_PASS.lock() {
        *l = Some(std::time::Instant::now());
    }
    if let Some(i) = session.game_state().and_then(|gs| gs.idle_game.as_ref()) {
        journal(&format!("state: runes {}, a sacrifice would give {}, resets {}, total sacrificed {}, money {}", i.current_runes, i.sacrifice_runes, i.resets, i.total_sacrificed, i.current_money));
    }
    // The prestige first (user 2026-11-05): it resets the buildings, the upgrades below start over with the new runes
    if let Some(gs) = session.game_state().filter(|gs| sacrifice_due(gs)) {
        let idle = gs.idle_game.as_ref();
        report!("[idle] Arena Manager: SACRIFICE, runes {} + {} (reset no. {})", idle.map_or(String::new(), |i| i.current_runes.to_string()), idle.map_or(String::new(), |i| i.sacrifice_runes.to_string()), idle.map_or(0, |i| i.resets + 1));
        journal("SACRIFICE sent");
        match safe::send(session, Command::IdleSacrifice).await {
            Ok(_) => {}
            Err(e) if crate::tavern::is_session_error(&e) => return Outcome::SessionLost,
            Err(e) => report!("[idle] Sacrifice failed: {e}"),
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
        if let Some(i) = session.game_state().and_then(|gs| gs.idle_game.as_ref()) {
            report!("[idle] Arena Manager after the sacrifice: runes {}, a next one would give {}, resets {}", i.current_runes, i.sacrifice_runes, i.resets);
            journal(&format!("after the sacrifice: runes {}, a next one would give {}, resets {}, total sacrificed {}", i.current_runes, i.sacrifice_runes, i.resets, i.total_sacrificed));
        }
    }
    for _ in 0..MAX_PER_PASS {
        let Some(gs) = session.game_state() else { break };
        let Some((typ, (amount, n, price))) = next_purchase(gs) else { break };
        report!("[idle] Arena Manager: {typ:?} x{n} for {price:.0} dollars");
        match safe::send(session, Command::IdleUpgrade { typ, amount }).await {
            Ok(_) => {}
            Err(e) if crate::tavern::is_session_error(&e) => return Outcome::SessionLost,
            Err(e) => {
                report!("[idle] Upgrade failed: {e}");
                break;
            }
        }
        safe::human_pause().await;
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::{IdleBuildingType::*, IdleUpgradeAmount as A, next_step, pick_batch, rune_rule};

    fn prices() -> [(A, u32, f64); 4] {
        [(A::Hundred, 100, 1000.0), (A::TwentyFive, 25, 300.0), (A::Ten, 10, 100.0), (A::One, 1, 10.0)]
    }

    #[test]
    fn buys_exactly_up_to_the_breakpoint() {
        let n = |missing, money| pick_batch(missing, money, prices()).map(|b| b.1);
        assert_eq!(n(24, 1e9), Some(10)); // 100 and 25 would overshoot
        assert_eq!(n(25, 1e9), Some(25));
        assert_eq!(n(1, 1e9), Some(1));
        assert_eq!(n(250, 1e9), Some(100));
        assert_eq!(n(250, 150.0), Some(10)); // the biggest affordable
        assert_eq!(n(25, 5.0), None); // even one level is too dear: save up
    }

    #[test]
    fn the_list_is_followed_in_order() {
        let steps = [(Seat, 25), (Seat, 50), (PopcornStand, 25)];
        assert_eq!(next_step(&steps, |_| 0), Some((Seat, 25)));
        assert_eq!(next_step(&steps, |t| if t == Seat { 30 } else { 0 }), Some((Seat, 20)));
        assert_eq!(next_step(&steps, |t| if t == Seat { 50 } else { 0 }), Some((PopcornStand, 25)));
        assert_eq!(next_step(&steps, |_| 99), None);
    }

    #[test]
    fn sacrifice_when_the_gain_equals_the_runes_owned() {
        assert!(!rune_rule(19.0, 0.0));
        assert!(rune_rule(20.0, 0.0)); // the first one at 20
        assert!(!rune_rule(19.0, 20.0));
        assert!(rune_rule(20.0, 20.0)); // 20 owned -> second at +20 (40 total)
        assert!(!rune_rule(39.0, 40.0));
        assert!(rune_rule(40.0, 40.0)); // 40 owned -> third at +40 (80 total)
        assert!(rune_rule(22.0, 22.0)); // missed the exact 20: 22 -> 44
        assert!(!rune_rule(21.0, 22.0));
        assert!(!rune_rule(0.0, 0.0));
    }
}
