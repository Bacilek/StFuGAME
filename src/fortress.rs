//! Fortress (unlocked at level 25, user 2026-10-11, rules in `docs/fortress.md`): collect the resources, finish and start
//! builds. What to build: first the sequence the user dictates (`PLAN`), afterwards `pick_generic` carries on by itself
//! with the rules the user described (Fortress is the goal, supported by income buildings, Treasury and Laborers'
//! Quarters, then the Gem Mine at level 1; costs are weighed). NEVER the Wall (fortifications: weak defence = more
//! attackers = more counter-attacks); the Gem Mine only to level 1 until character level 125. Never cancel and never
//! mushrooms or hourglasses to finish a build (`FortressBuildFinish` only with `mushrooms: 0`, which is also what the
//! game wants after a build timer ran out).
//!
//! Gold priority (user 2026-10-11): better items > fortress > spinning the shops > guild upgrades > attributes, so a build
//! is only started after the day's shop pass (`shops::pass_done`). The shop reserve does NOT apply to the fortress
//! (user 2026-10-11): the build may use all the gold.

use chrono::Local;
use sf_api::{
    command::Command,
    gamestate::{
        GameState,
        fortress::{Fortress, FortressBuildingType as B, FortressResourceType as R},
    },
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// The build plan (user 2026-10-11): (building, level to reach), in this order. The bot builds the first entry whose
/// building is below its level and WAITS for it (resources/gold) instead of skipping ahead. When the list is done
/// `pick_generic` takes over (the user may run out of time to dictate more). Wall is never built.
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
    // User 2026-10-11: the Treasury 1 will probably come before the Laborers' Quarters 2 as well
    (B::Treasury, 1),
    (B::LaborersQuarters, 2),
    // User 2026-10-11: after Fortress 3
    (B::Fortress, 3),
    (B::WoodcuttersHut, 3),
    (B::Quarry, 3),
    (B::GemMine, 1),
    (B::Treasury, 2),
    (B::LaborersQuarters, 3),
];

/// May the bot build/finish this building right now? The same rule is enforced in `safe.rs`. Fortress, Laborers'
/// Quarters, Woodcutter's Hut, Quarry and Treasury always; the Gem Mine only to level 1, further levels only from
/// character level 125 (user 2026-10-11); never the Wall, units or anything else.
pub fn build_ok(gs: &GameState, b: B) -> bool {
    match b {
        B::Fortress | B::LaborersQuarters | B::WoodcuttersHut | B::Quarry | B::Treasury => true,
        B::GemMine => {
            gs.character.level >= 125 || gs.fortress.as_ref().is_some_and(|f| f.buildings[B::GemMine].level == 0)
        }
        _ => false,
    }
}

/// The next building of the dictated plan that is not at its target level yet.
fn pick(level: impl Fn(B) -> u16, max_level: u16) -> Option<B> {
    PLAN.iter().find(|(b, target)| level(*b) < (*target).min(max_level.max(1))).map(|(b, _)| *b)
}

/// What to build after the dictated plan (user 2026-10-11, in case he cannot dictate more): the goal is the next
/// Fortress level, but before each new Fortress level the supporting buildings must reach the current Fortress level `n`:
/// 1. the income buildings (Woodcutter's Hut, Quarry) – the cheaper one first, they raise the income,
/// 2. the Treasury (level 1 from Fortress 2, further levels when the next Fortress level would not fit into the
///    wood/stone storage – it could never be afforded otherwise),
/// 3. the Laborers' Quarters (they only shorten build times, so after the income),
/// 4. the Gem Mine at level 1 once the Fortress is level 3 (more levels only from character level 125),
/// 5. the next Fortress level.
///
/// A building never goes above the Fortress level. `cost` = (wood, stone) of the next level, `limit` = storage
/// (wood, stone).
fn pick_generic(
    level: &dyn Fn(B) -> u16,
    cost: &dyn Fn(B) -> (u64, u64),
    limit: (u64, u64),
    char_level: u16,
    max_level: u16,
) -> Option<B> {
    let n = level(B::Fortress);
    if n == 0 {
        return Some(B::Fortress);
    }
    let cap = n.min(max_level.max(1));
    if let Some(b) = [B::WoodcuttersHut, B::Quarry].into_iter().filter(|b| level(*b) < cap).min_by_key(|b| {
        let (w, s) = cost(*b);
        w + s
    }) {
        return Some(b);
    }
    let (fortress_wood, fortress_stone) = cost(B::Fortress);
    let storage_short = fortress_wood > limit.0 || fortress_stone > limit.1;
    if n >= 2 && level(B::Treasury) < cap && (level(B::Treasury) == 0 || storage_short) {
        return Some(B::Treasury);
    }
    if level(B::LaborersQuarters) < cap {
        return Some(B::LaborersQuarters);
    }
    if n >= 3 && (level(B::GemMine) == 0 || (char_level >= 125 && level(B::GemMine) < cap)) {
        return Some(B::GemMine);
    }
    (n < max_level).then_some(B::Fortress)
}

/// The next building to work on: the dictated plan first, then the generic rules.
fn choose(f: &Fortress, char_level: u16) -> Option<B> {
    let level = |b: B| f.buildings[b].level;
    let max = u16::from(f.building_max_lvl);
    pick(level, max).or_else(|| {
        let cost = |b: B| (f.buildings[b].upgrade_cost.wood, f.buildings[b].upgrade_cost.stone);
        let limit = (f.resources[R::Wood].limit, f.resources[R::Stone].limit);
        pick_generic(&level, &cost, limit, char_level, max)
    })
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
        if finish > Local::now() || !build_ok(gs, target) {
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
        let missing = choose(f, gs.character.level).map_or(0, |next| {
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
    let Some(next) = choose(f, gs.character.level) else { return Outcome::Done };
    if !build_ok(gs, next) {
        return Outcome::Done;
    }
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
    fn dictated_plan_order() {
        assert_eq!(pick(at(&[]), 20), Some(B::Fortress));
        assert_eq!(pick(at(&[(B::Fortress, 1)]), 20), Some(B::WoodcuttersHut));
        assert_eq!(pick(at(&[(B::Fortress, 1), (B::WoodcuttersHut, 1)]), 20), Some(B::Quarry));
        let done = &[(B::Fortress, 1), (B::WoodcuttersHut, 1), (B::Quarry, 1), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(done), 20), Some(B::Fortress));
        // Fortress 2, then Quarry 2, Woodcutter 2, Treasury 1, Laborers' Quarters 2 (income before time savers)
        let f2 = &[(B::Fortress, 2), (B::WoodcuttersHut, 1), (B::Quarry, 1), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(f2), 20), Some(B::Quarry));
        let q2 = &[(B::Fortress, 2), (B::WoodcuttersHut, 1), (B::Quarry, 2), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(q2), 20), Some(B::WoodcuttersHut));
        let w2 = &[(B::Fortress, 2), (B::WoodcuttersHut, 2), (B::Quarry, 2), (B::LaborersQuarters, 1)];
        assert_eq!(pick(at(w2), 20), Some(B::Treasury));
        let t1 = &[(B::Fortress, 2), (B::WoodcuttersHut, 2), (B::Quarry, 2), (B::LaborersQuarters, 1), (B::Treasury, 1)];
        assert_eq!(pick(at(t1), 20), Some(B::LaborersQuarters));
        let end = &[(B::Fortress, 2), (B::WoodcuttersHut, 2), (B::Quarry, 2), (B::LaborersQuarters, 2), (B::Treasury, 1)];
        assert_eq!(pick(at(end), 20), Some(B::Fortress));
        let all = &[(B::Fortress, 3), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::GemMine, 1), (B::Treasury, 2), (B::LaborersQuarters, 3)];
        assert_eq!(pick(at(all), 20), None);
    }

    /// Cheap Woodcutter's Hut / expensive Quarry etc. for the generic rules.
    fn generic(levels: &'static [(B, u16)], limit: (u64, u64), char_level: u16) -> Option<B> {
        let cost = |b: B| match b {
            B::Fortress => (150, 50),
            B::WoodcuttersHut => (30, 20),
            B::Quarry => (90, 16),
            _ => (40, 10),
        };
        pick_generic(&at(levels), &cost, limit, char_level, 20)
    }

    #[test]
    fn generic_rules_after_the_plan() {
        let base = &[(B::Fortress, 3), (B::WoodcuttersHut, 2), (B::Quarry, 2), (B::LaborersQuarters, 2), (B::Treasury, 1)];
        // Fortress 3 first needs the income buildings at level 3, the cheaper one first (Woodcutter 50 < Quarry 106)
        assert_eq!(generic(base, (900, 300), 30), Some(B::WoodcuttersHut));
        let w3 = &[(B::Fortress, 3), (B::WoodcuttersHut, 3), (B::Quarry, 2), (B::LaborersQuarters, 2), (B::Treasury, 1)];
        assert_eq!(generic(w3, (900, 300), 30), Some(B::Quarry));
        // income done: Treasury only when the next Fortress level does not fit into the storage, else Laborers' Quarters
        let inc = &[(B::Fortress, 3), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 2), (B::Treasury, 1)];
        assert_eq!(generic(inc, (900, 300), 30), Some(B::LaborersQuarters));
        assert_eq!(generic(inc, (100, 300), 30), Some(B::Treasury));
        // everything at the Fortress level: the Gem Mine at level 1, then the next Fortress level
        let sup = &[(B::Fortress, 3), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 3), (B::Treasury, 3)];
        assert_eq!(generic(sup, (900, 300), 30), Some(B::GemMine));
        let gem = &[(B::Fortress, 3), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 3), (B::Treasury, 3), (B::GemMine, 1)];
        assert_eq!(generic(gem, (900, 300), 30), Some(B::Fortress));
        // the Gem Mine only goes further from character level 125
        let up = &[(B::Fortress, 4), (B::WoodcuttersHut, 4), (B::Quarry, 4), (B::LaborersQuarters, 4), (B::Treasury, 4), (B::GemMine, 1)];
        assert_eq!(generic(up, (900, 300), 30), Some(B::Fortress));
        assert_eq!(generic(up, (900, 300), 125), Some(B::GemMine));
        // never the Wall, never above the Fortress level
        assert_eq!(generic(&[], (900, 300), 30), Some(B::Fortress));
    }
}
