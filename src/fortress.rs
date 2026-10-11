//! Fortress (unlocked at level 25, user 2026-10-11, rules in `docs/fortress.md`): collect the resources, finish and start
//! builds. Only Fortress, Laborers' Quarters, Woodcutter's Hut and Quarry for now (the user answers about the other buildings
//! later); NEVER the Wall (fortifications: weak defence = more attackers = more counter-attacks) and no Gem Mine yet
//! (only from character level 125). Never mushrooms or hourglasses to finish a build (`FortressBuildFinish` only with
//! `mushrooms: 0`, which is also what the game wants after a build timer ran out).
//!
//! Gold priority (user 2026-10-11): better items > fortress > spinning the shops > attributes, so a build is only paid
//! from gold ABOVE the shop reserve and only after the day's shop pass (`shops::pass_done`).

use chrono::Local;
use sf_api::{
    command::Command,
    gamestate::{
        GameState,
        fortress::{FortressBuildingType as B, FortressResourceType as R},
    },
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// Buildings the bot may touch (the same list is enforced in `safe.rs`).
pub fn building_allowed(b: B) -> bool {
    matches!(b, B::Fortress | B::LaborersQuarters | B::WoodcuttersHut | B::Quarry)
}

/// Which building to build next: the Woodcutter's Hut right after the Fortress exists (user: its level 1 is the second
/// upgrade, the wood has to come from somewhere), then the Quarry (the stone, user 2026-10-11), then the Fortress and the Laborers' Quarters together – whichever
/// has the lower level, the Fortress on a tie (a building cannot be higher than the Fortress).
fn pick(level: impl Fn(B) -> u16, max_level: u16) -> Option<B> {
    let (fortress, quarters) = (level(B::Fortress), level(B::LaborersQuarters));
    if fortress == 0 {
        return Some(B::Fortress);
    }
    if level(B::WoodcuttersHut) == 0 {
        return Some(B::WoodcuttersHut);
    }
    if level(B::Quarry) == 0 {
        return Some(B::Quarry);
    }
    if quarters < fortress {
        return Some(B::LaborersQuarters);
    }
    (fortress < max_level).then_some(B::Fortress)
}

/// Resources of one kind that can be collected right now (the server only recalculates them from time to time).
fn collectable(gs: &GameState, r: R) -> u64 {
    let Some(f) = gs.fortress.as_ref() else { return 0 };
    let p = &f.resources[r].production;
    let hours = f.last_collectable_updated.map_or(0.0, |t| (Local::now() - t).num_seconds().max(0) as f64 / 3600.0);
    (p.last_collectable + (p.per_hour as f64 * hours) as u64).min(p.limit)
}

/// One pass: finish a build whose timer ran out, collect resources worth collecting, start the next build when
/// everything is affordable (gold above the shop reserve).
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
    if !f.can_build(next, silver.saturating_sub(crate::shops::reserve())) {
        return Outcome::Done; // not enough wood/stone/gold above the reserve (or level cap)
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

    fn levels(f: u16, q: u16, w: u16) -> impl Fn(B) -> u16 {
        levels4(f, q, w, 1)
    }

    fn levels4(f: u16, q: u16, w: u16, quarry: u16) -> impl Fn(B) -> u16 {
        move |b| match b {
            B::Fortress => f,
            B::LaborersQuarters => q,
            B::WoodcuttersHut => w,
            B::Quarry => quarry,
            _ => 0,
        }
    }

    #[test]
    fn build_order() {
        assert_eq!(pick(levels(0, 0, 0), 15), Some(B::Fortress));
        // Woodcutter's Hut 1 right after Fortress 1 (user 2026-10-11)
        assert_eq!(pick(levels(1, 0, 0), 15), Some(B::WoodcuttersHut));
        // then the Quarry (stone), user 2026-10-11
        assert_eq!(pick(levels4(1, 0, 1, 0), 15), Some(B::Quarry));
        assert_eq!(pick(levels(1, 0, 1), 15), Some(B::LaborersQuarters));
        assert_eq!(pick(levels(1, 1, 1), 15), Some(B::Fortress));
        assert_eq!(pick(levels(2, 1, 1), 15), Some(B::LaborersQuarters));
        assert_eq!(pick(levels(15, 15, 1), 15), None);
        assert_eq!(pick(levels(15, 14, 1), 15), Some(B::LaborersQuarters));
    }

    #[test]
    fn only_the_agreed_buildings() {
        assert!(building_allowed(B::Fortress) && building_allowed(B::LaborersQuarters) && building_allowed(B::WoodcuttersHut));
        assert!(building_allowed(B::Quarry));
        assert!(!building_allowed(B::Wall) && !building_allowed(B::GemMine) && !building_allowed(B::Barracks));
    }
}
