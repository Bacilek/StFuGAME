//! Fortress (unlocked at level 25, user 2026-10-11, rules in `docs/fortress.md`): collect the resources, finish and start
//! builds, strictly following `PLAN` below (the user dictates the sequence; nothing outside it is ever built); NEVER the Wall (fortifications: weak defence = more attackers = more counter-attacks) and no Gem Mine yet
//! (only from character level 125). Never mushrooms or hourglasses to finish a build (`FortressBuildFinish` only with
//! `mushrooms: 0`, which is also what the game wants after a build timer ran out).
//!
//! Gold priority (user 2026-10-11): better items > fortress > spinning the shops > guild upgrades > attributes, so a build
//! is only started after the day's shop pass (`shops::pass_done`). The shop reserve does NOT apply to the fortress
//! (user 2026-10-11): the build may use all the gold.

use chrono::Local;
use sf_api::{
    command::Command,
    gamestate::{
        GameState,
        fortress::{FortressBuildingType as B, FortressResourceType as R},
    },
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// The build plan (user 2026-10-11): (building, level to reach), in this order. The bot builds the first entry whose
/// building is below its level and WAITS for it (resources/gold) instead of skipping ahead. When the list is done it
/// stops until the user dictates more. Wall and Gem Mine (only from character level 125) are not in it.
const PLAN: &[(B, u16)] = &[
    (B::Fortress, 1),
    (B::WoodcuttersHut, 1),
    (B::Quarry, 1),
    (B::LaborersQuarters, 1),
    (B::Fortress, 2),
    // User 2026-10-11: the production buildings go before the Laborers' Quarters – they are cheaper and raise the income,
    // while the Laborers' Quarters only shorten build times, which is pointless when we wait for wood/stone anyway.
    // Keep this in mind when the list grows: income first, time savers after.
    (B::Quarry, 2),
    (B::WoodcuttersHut, 2),
    (B::LaborersQuarters, 2),
    (B::Treasury, 1),
];

/// Buildings the bot may touch (the same list is enforced in `safe.rs`).
pub fn building_allowed(b: B) -> bool {
    PLAN.iter().any(|(p, _)| *p == b)
}

/// The next building of the plan that is not at its target level yet.
fn pick(level: impl Fn(B) -> u16, max_level: u16) -> Option<B> {
    PLAN.iter().find(|(b, target)| level(*b) < (*target).min(max_level.max(1))).map(|(b, _)| *b)
}

/// Resources of one kind that can be collected right now (the server only recalculates them from time to time).
fn collectable(gs: &GameState, r: R) -> u64 {
    let Some(f) = gs.fortress.as_ref() else { return 0 };
    let p = &f.resources[r].production;
    let hours = f.last_collectable_updated.map_or(0.0, |t| (Local::now() - t).num_seconds().max(0) as f64 / 3600.0);
    (p.last_collectable + (p.per_hour as f64 * hours) as u64).min(p.limit)
}

/// One pass: finish a build whose timer ran out, collect resources worth collecting, start the next build when
/// everything is affordable (all of the gold, no reserve kept).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    if f.buildings[B::Fortress].upgrade_cost.time.is_zero() && f.buildings[B::Fortress].level == 0 {
        return Outcome::Done; // prices not known yet
    }

    // 1. A build whose timer ran out has to be finished (with 0 mushrooms) before anything new can start
    if let (Some(target), Some(finish)) = (f.building_upgrade.target, f.building_upgrade.finish) {
        if finish > Local::now() || !building_allowed(target) {
            return Outcome::Done;
        }
        report!("[fortress] {target:?} is finished, collecting it");
        if let Err(e) = safe::send(session, Command::FortressBuildFinish { f_type: target, mushrooms: 0 }).await {
            return fail(&e);
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
    }

    // 2. Collect what has piled up (free): when half of the production limit is ready, or when it covers what the next
    // build is still missing (the stock is tiny at the start: 17 wood, 13 stone)
    for r in [R::Wood, R::Stone, R::Experience] {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
        let (limit, ready) = (f.resources[r].production.limit, collectable(gs, r));
        let room = r == R::Experience || f.resources[r].current < f.resources[r].limit;
        let missing = pick(|b| f.buildings[b].level, u16::from(f.building_max_lvl)).map_or(0, |next| {
            let c = f.buildings[next].upgrade_cost;
            match r {
                R::Wood => c.wood.saturating_sub(f.resources[r].current),
                R::Stone => c.stone.saturating_sub(f.resources[r].current),
                R::Experience => 0,
            }
        });
        if limit > 0 && ready > 0 && (ready * 2 >= limit || (missing > 0 && ready >= missing)) && room {
            report!("[fortress] Collecting {r:?} (about {ready} of {limit})");
            if let Err(e) = safe::send(session, Command::FortressGather { resource: r }).await {
                return fail(&e);
            }
        }
    }

    // 3. The next build
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    if f.building_upgrade.target.is_some() || !crate::shops::pass_done() {
        return Outcome::Done;
    }
    let Some(next) = pick(|b| f.buildings[b].level, u16::from(f.building_max_lvl)) else { return Outcome::Done };
    let cost = f.buildings[next].upgrade_cost;
    let silver = gs.character.silver;
    if !f.can_build(next, silver) {
        return Outcome::Done; // not enough wood/stone/gold (or level cap)
    }
    report!(
        "[fortress] Building {next:?} level {} ({} min, wood {}, stone {}, {})",
        f.buildings[next].level + 1,
        cost.time.as_secs() / 60,
        cost.wood,
        cost.stone,
        crate::report::gold(cost.silver)
    );
    if let Err(e) = safe::send(session, Command::FortressBuild { f_type: next }).await {
        return fail(&e);
    }
    Outcome::Done
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[fortress] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(levels: &'static [(B, u16)]) -> impl Fn(B) -> u16 {
        move |b| levels.iter().find(|(x, _)| *x == b).map_or(0, |(_, l)| *l)
    }

    #[test]
    fn build_order() {
        assert_eq!(pick(at(&[]), 15), Some(B::Fortress));
        assert_eq!(pick(at(&[(B::Fortress, 1)]), 15), Some(B::WoodcuttersHut));
        assert_eq!(pick(at(&[(B::Fortress, 1), (B::WoodcuttersHut, 1)]), 15), Some(B::Quarry));
        // the user's order: Fortress 2, then Quarry 2, Woodcutter 2, Laborers' Quarters 2, Treasury 1 (income before time savers)
        let done = &[(B::Fortress, 1), (B::WoodcuttersHut, 1), (B::Quarry, 1), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(done), 15), Some(B::Fortress));
        let f2 = &[(B::Fortress, 2), (B::WoodcuttersHut, 1), (B::Quarry, 1), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(f2), 15), Some(B::Quarry));
        let q2 = &[(B::Fortress, 2), (B::WoodcuttersHut, 1), (B::Quarry, 2), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(q2), 15), Some(B::WoodcuttersHut));
        let w2 = &[(B::Fortress, 2), (B::WoodcuttersHut, 2), (B::Quarry, 2), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(w2), 15), Some(B::LaborersQuarters));
        let all = &[(B::Fortress, 2), (B::WoodcuttersHut, 2), (B::Quarry, 2), (B::LaborersQuarters, 2)];
        assert_eq!(pick(at(all), 15), Some(B::Treasury));
        let end = &[(B::Fortress, 2), (B::WoodcuttersHut, 2), (B::Quarry, 2), (B::LaborersQuarters, 2), (B::Treasury, 1)];
        assert_eq!(pick(at(end), 15), None);
    }

    #[test]
    fn only_the_planned_buildings() {
        for b in [B::Fortress, B::LaborersQuarters, B::WoodcuttersHut, B::Quarry, B::Treasury] {
            assert!(building_allowed(b));
        }
        assert!(!building_allowed(B::Wall) && !building_allowed(B::GemMine) && !building_allowed(B::Barracks));
    }
}
