//! Arena Manager (idle game, level 105+, user 2026-10-25/31, `docs/idle.md`): spends the dollars on building upgrades. Dollars are
//! credited by time. Only `IdleUpgrade` is sent: NEVER `IdleSacrifice` (prestige timing is not decided), never the merchant (mushrooms).
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
    if LAST_PASS.lock().is_ok_and(|l| l.is_some_and(|t| t.elapsed().as_secs() < PASS_EVERY_SEC)) {
        return Outcome::Done;
    }
    if let Ok(mut l) = LAST_PASS.lock() {
        *l = Some(std::time::Instant::now());
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
