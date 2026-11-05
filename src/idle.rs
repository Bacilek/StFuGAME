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

/// The cheapest affordable batch: (building, amount, price).
fn next_purchase(gs: &GameState) -> Option<(IdleBuildingType, IdleUpgradeAmount, f64)> {
    let idle = gs.idle_game.as_ref()?;
    let money = num(&idle.current_money);
    let (typ, b) = idle.buildings.iter().min_by(|a, b| num(&a.1.upgrade_cost).total_cmp(&num(&b.1.upgrade_cost)))?;
    if num(&b.upgrade_cost) > money {
        return None;
    }
    let (amount, price) = [
        (IdleUpgradeAmount::Hundred, num(&b.upgrade_cost_100x)),
        (IdleUpgradeAmount::TwentyFive, num(&b.upgrade_cost_25x)),
        (IdleUpgradeAmount::Ten, num(&b.upgrade_cost_10x)),
        (IdleUpgradeAmount::One, num(&b.upgrade_cost)),
    ]
    .into_iter()
    .find(|(_, p)| *p <= money)?;
    Some((typ, amount, price))
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
    // The prestige first (user 2026-11-05): it resets the buildings, the upgrades below start over with the new runes
    if let Some(gs) = session.game_state().filter(|gs| sacrifice_due(gs)) {
        let idle = gs.idle_game.as_ref();
        report!("[idle] Arena Manager: SACRIFICE, runes {} + {} (reset no. {})", idle.map_or(String::new(), |i| i.current_runes.to_string()), idle.map_or(String::new(), |i| i.sacrifice_runes.to_string()), idle.map_or(0, |i| i.resets + 1));
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
        }
    }
    for _ in 0..MAX_PER_PASS {
        let Some(gs) = session.game_state() else { break };
        let Some((typ, amount, price)) = next_purchase(gs) else { break };
        report!("[idle] Arena Manager: {typ:?} x{} for {price:.0} dollars", amount as i32);
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
    use super::rune_rule;

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
