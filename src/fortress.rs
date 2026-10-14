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
        fortress::{Fortress, FortressBuildingType as B, FortressResourceType as R, FortressUnitType as U},
        social::CombatMessageType,
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
    // User 2026-10-11: then save straight for Fortress 4 – no Treasury, Gem Mine or Laborers' Quarters first, not even to
    // shorten a wait – so the Barracks (very cheap, fast, many more resources) can be bought as soon as possible.
    (B::Fortress, 4),
    (B::Barracks, 1),
    (B::GemMine, 1),
    (B::Treasury, 2),
    (B::LaborersQuarters, 3),
    // User 2026-10-12: rush Fortress 6 (the Academy, which gives XP, needs it). User 2026-10-14: then rush the Academy
    // itself (up to the Fortress level) and only after it "a little" Treasury (`pick_generic`: half of the Fortress level).
    (B::Fortress, 5),
    (B::Fortress, 6),
    (B::Academy, 6),
];

/// May the bot build/finish this building right now? The same rule is enforced in `safe.rs`. Fortress, Laborers'
/// Quarters, Woodcutter's Hut, Quarry and Treasury always; the Gem Mine only to level 1, further levels only from
/// character level 125 (user 2026-10-11); never the Wall, units or anything else.
pub fn build_ok(gs: &GameState, b: B) -> bool {
    match b {
        B::Fortress | B::LaborersQuarters | B::WoodcuttersHut | B::Quarry | B::Treasury | B::Barracks | B::Academy => true,
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

/// Barracks level the generic rules aim for (more soldiers = more attacks; the tips say 3-4).
const BARRACKS_TARGET: u16 = 4;

/// What to build after the dictated plan. Since fortress attacks supply wood and stone far beyond the storage cap
/// (user 2026-10-12) the only bottleneck is BUILD TIME, so the income buildings (Woodcutter's Hut, Quarry) are no longer
/// prerequisites. Before each new Fortress level (current level `n`):
/// 1. the Laborers' Quarters up to `n` (they shorten every later build),
/// 2. the Academy up to `n` from Fortress 6 (XP; user 2026-10-14: rush it, ahead of the Barracks and the Treasury),
/// 3. the Barracks up to `BARRACKS_TARGET` (soldier capacity),
/// 4. the Treasury up to half of `n`, rounded up (inventory slots, user: "at least a little"),
/// 5. the Gem Mine at level 1 from Fortress 3 (more levels only from character level 125),
/// 6. the next Fortress level.
///
/// A building never goes above the Fortress level. The Hall of Knights is not automated yet (waiting for the user).
fn pick_generic(level: &dyn Fn(B) -> u16, char_level: u16, max_level: u16) -> Option<B> {
    let n = level(B::Fortress);
    if n == 0 {
        return Some(B::Fortress);
    }
    let cap = n.min(max_level.max(1));
    if level(B::LaborersQuarters) < cap {
        return Some(B::LaborersQuarters);
    }
    if n >= 6 && level(B::Academy) < cap {
        return Some(B::Academy);
    }
    if level(B::Barracks) < cap.min(BARRACKS_TARGET) {
        return Some(B::Barracks);
    }
    if n >= 2 && level(B::Treasury) < n.div_ceil(2).min(cap) {
        return Some(B::Treasury);
    }
    if n >= 3 && (level(B::GemMine) == 0 || (char_level >= 125 && level(B::GemMine) < cap)) {
        return Some(B::GemMine);
    }
    (n < max_level).then_some(B::Fortress)
}

/// Every building type in discriminant order (`b as usize` indexes it).
const ALL: [B; 12] = [
    B::Fortress,
    B::LaborersQuarters,
    B::WoodcuttersHut,
    B::Quarry,
    B::GemMine,
    B::Academy,
    B::ArcheryGuild,
    B::Barracks,
    B::MagesTower,
    B::Treasury,
    B::Smithy,
    B::Wall,
];

/// The next building to work on: the dictated plan first, then the generic rules.
fn choose(f: &Fortress, char_level: u16) -> Option<B> {
    let level = |b: B| f.buildings[b].level;
    next_build(&level, char_level, u16::from(f.building_max_lvl))
}

fn next_build(level: &dyn Fn(B) -> u16, char_level: u16, max: u16) -> Option<B> {
    pick(level, max).or_else(|| pick_generic(level, char_level, max))
}

/// The next `count` builds after the one that is running now (simulated: each pick is counted as built), as
/// (building, level it will reach). For the Fortress tab.
fn upcoming(f: &Fortress, char_level: u16, count: usize) -> Vec<(B, u16)> {
    let mut lv: Vec<u16> = ALL.iter().map(|&b| f.buildings[b].level).collect();
    if let Some(t) = f.building_upgrade.target {
        lv[t as usize] += 1;
    }
    let max = u16::from(f.building_max_lvl);
    let mut out = Vec::new();
    while out.len() < count {
        let Some(b) = next_build(&|b: B| lv[b as usize], char_level, max) else { break };
        lv[b as usize] += 1;
        out.push((b, lv[b as usize]));
    }
    out
}

/// When the next thing to build is the Barracks and it could start right then (the build before it finishes, or none is
/// running; shop pass done; wood/stone/gold ready), the moment it starts. Soldiers in training block the Barracks upgrade
/// (`Fortress::in_use`), so training is limited to what finishes by then (user 2026-10-14). Not restricted while the
/// Barracks upgrade is not affordable: the soldiers' raids bring the resources, so stopping them would deadlock.
fn barracks_due(gs: &GameState) -> Option<chrono::DateTime<Local>> {
    let f = gs.fortress.as_ref()?;
    if !crate::shops::pass_done() {
        return None;
    }
    let mut lv: Vec<u16> = ALL.iter().map(|&b| f.buildings[b].level).collect();
    let at = match f.building_upgrade.target {
        Some(t) => {
            lv[t as usize] += 1;
            f.building_upgrade.finish.unwrap_or_else(Local::now)
        }
        None => Local::now(),
    };
    let next = next_build(&|b: B| lv[b as usize], gs.character.level, u16::from(f.building_max_lvl))?;
    let c = f.buildings[next].upgrade_cost;
    let affordable = c.wood <= f.resources[R::Wood].current && c.stone <= f.resources[R::Stone].current && c.silver <= gs.character.silver;
    (next == B::Barracks && affordable).then_some(at)
}

/// Can the Hall of Knights be upgraded right now (the same rule `safe.rs` enforces)? It can go while another building is
/// being built (user 2026-10-14, corrects the earlier belief), only not – probably – while the Fortress itself is under
/// construction (we do not try then). The price is known and paid in wood, stone and gold we have, and the level stays
/// below the Fortress level.
pub fn hok_affordable(gs: &GameState) -> bool {
    let Some(f) = gs.fortress.as_ref() else { return false };
    let p = f.hall_of_knights_upgrade_price;
    f.building_upgrade.target != Some(B::Fortress)
        && (p.wood > 0 || p.stone > 0)
        && p.wood <= f.resources[R::Wood].current
        && p.stone <= f.resources[R::Stone].current
        && p.silver <= gs.character.silver
        && f.hall_of_knights_level < f.buildings[B::Fortress].level
}

/// Wood and stone the next planned building still needs (None when nothing is planned): other spending (gems) leaves it alone.
pub fn next_build_cost(gs: &GameState) -> Option<(u64, u64)> {
    let f = gs.fortress.as_ref()?;
    let next = choose(f, gs.character.level)?;
    let c = f.buildings[next].upgrade_cost;
    Some((c.wood, c.stone))
}

fn collectable(gs: &GameState, r: R) -> u64 {
    let Some(f) = gs.fortress.as_ref() else { return 0 };
    let p = &f.resources[r].production;
    let hours = f.last_collectable_updated.map_or(0.0, |t| (Local::now() - t).num_seconds().max(0) as f64 / 3600.0);
    (p.last_collectable + (p.per_hour as f64 * hours) as u64).min(p.limit)
}

/// Experience (Academy) production is collected when at least this share of its limit is ready ...
const COLLECT_SHARE_PCT: u64 = 50;
/// ... or when it would be full within this many minutes (the loop may sleep up to 30 min; user 2026-10-14: resources,
/// ESPECIALLY the Academy's XP, must never overflow – a full stock stops producing).
const FULL_SOON_MIN: u64 = 45;
/// How long before it is full the loop is woken up to collect (must stay below `FULL_SOON_MIN`).
const WAKE_BEFORE_FULL_MIN: u64 = 40;

/// Minutes until the production of `r` fills up (`None` when nothing is produced; 0 when it is full).
fn minutes_to_full(gs: &GameState, r: R) -> Option<u64> {
    let f = gs.fortress.as_ref()?;
    let p = &f.resources[r].production;
    if p.per_hour == 0 || p.limit == 0 {
        return None;
    }
    let left = p.limit.saturating_sub(collectable(gs, r));
    Some(left * 60 / p.per_hour)
}

/// Should the production of `r` be collected now? Half of the limit is ready, or it is about to be full, or it covers what
/// the next build is still missing (`missing`, the stock is tiny at the start: 17 wood, 13 stone).
fn should_collect(gs: &GameState, r: R, missing: u64) -> bool {
    let Some(f) = gs.fortress.as_ref() else { return false };
    let (limit, ready) = (f.resources[r].production.limit, collectable(gs, r));
    let room = r == R::Experience || f.resources[r].current < f.resources[r].limit;
    limit > 0
        && ready > 0
        && room
        && (ready * 100 >= limit * COLLECT_SHARE_PCT
            || minutes_to_full(gs, r).is_some_and(|m| m <= FULL_SOON_MIN)
            || (missing > 0 && ready >= missing))
}

/// Collects the production of the given resources when worth it (free). `only_xp`: the attack-only character takes just the
/// Academy's experience.
async fn collect(session: &mut SimpleSession, only_xp: bool) -> Result<(), Outcome> {
    for r in [R::Wood, R::Stone, R::Experience] {
        if only_xp && r != R::Experience {
            continue;
        }
        let Some(gs) = session.game_state() else { return Err(Outcome::Done) };
        let Some(f) = gs.fortress.as_ref() else { return Err(Outcome::Done) };
        let missing = if only_xp {
            0
        } else {
            choose(f, gs.character.level).map_or(0, |next| {
                let c = f.buildings[next].upgrade_cost;
                match r {
                    R::Wood => c.wood.saturating_sub(f.resources[r].current),
                    R::Stone => c.stone.saturating_sub(f.resources[r].current),
                    R::Experience => 0,
                }
            })
        };
        if should_collect(gs, r, missing) {
            report!("[fortress] Collecting {r:?} (about {} of {})", collectable(gs, r), f.resources[r].production.limit);
            if let Err(e) = safe::send(session, Command::FortressGather { resource: r }).await {
                return Err(fail(&e));
            }
        }
    }
    Ok(())
}

/// Attack-only mode (user 2026-10-14, his main character): nothing but the Academy's XP and the soldiers' raids.
pub async fn run_attack_only(session: &mut SimpleSession) -> Outcome {
    if session.game_state().and_then(|gs| gs.fortress.as_ref()).is_none() {
        return Outcome::Done;
    }
    if let Err(o) = collect(session, true).await {
        return o;
    }
    military(session).await
}

/// One pass: finish a build whose timer ran out, collect resources worth collecting, upgrade the Hall of Knights, start the
/// next build when everything is affordable (all of the gold, no reserve kept).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    if f.buildings[B::Fortress].upgrade_cost.time.is_zero() && f.buildings[B::Fortress].level == 0 {
        return Outcome::Done; // prices not known yet
    }

    // 1. A build whose timer ran out has to be finished (with 0 mushrooms) before anything new can start
    if let (Some(target), Some(finish)) = (f.building_upgrade.target, f.building_upgrade.finish)
        && finish <= Local::now()
    {
        if !build_ok(gs, target) {
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

    // 2. Collect what has piled up (free), also while a build is running
    if let Err(o) = collect(session, false).await {
        return o;
    }

    // 3. The Hall of Knights (user 2026-10-12/14): instant, allowed while another building is being built (probably not while
    // the Fortress itself is), so whenever it is affordable it goes before the next build. Never above the Fortress level.
    if !crate::shops::pass_done() {
        return Outcome::Done;
    }
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    if hok_affordable(gs) {
        report!(
            "[fortress] Upgrading the Hall of Knights {} -> {} (wood {}, stone {}, {})",
            f.hall_of_knights_level,
            f.hall_of_knights_level + 1,
            f.hall_of_knights_upgrade_price.wood,
            f.hall_of_knights_upgrade_price.stone,
            crate::report::gold(f.hall_of_knights_upgrade_price.silver)
        );
        if let Err(e) = safe::send(session, Command::FortressUpgradeHallOfKnights).await {
            return fail(&e);
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
    }

    // 4. The next build (nothing is started while one is running)
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    if f.building_upgrade.target.is_some() {
        return Outcome::Done;
    }
    let Some(next) = choose(f, gs.character.level) else { return Outcome::Done };
    if !build_ok(gs, next) {
        return Outcome::Done;
    }
    let cost = f.buildings[next].upgrade_cost;
    let silver = gs.character.silver;
    if !f.can_build(next, silver) {
        return Outcome::Done; // not enough wood/stone/gold (or level cap, or soldiers in training for the Barracks)
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

/// How often the soldiers are looked after (training + attacks), per character.
const MILITARY_EVERY: chrono::Duration = chrono::Duration::minutes(5);
/// An opponent is worth a soldier (an attack always uses one up, even a won one – user 2026-10-12) only if the expected loot
/// (wood + stone, from `PlayerLookAt`) is at least this many times the price of a soldier (`min_loot`). A soldier costs
/// about 20 resources, a good raid brought 500–800 (captures 2026-10-12).
const LOOT_PER_COST: u64 = 2;
/// Used when the server shows no soldier price.
const FALLBACK_COST: u64 = 20;
/// Most attacks in one pass (soldiers come back one by one, a training takes ~10 min each).
const MAX_ATTACKS: usize = 10;
/// Counter-attack candidates examined per pass.
const MAX_COUNTERS: usize = 3;

static NEXT_MILITARY: crate::ctx::PerChar<Option<chrono::DateTime<Local>>> = crate::ctx::PerChar::new();

/// Expected loot (wood + stone) of fortress player `id`, from a fresh `PlayerLookAt`; `None` when it cannot be read.
async fn loot_of(session: &mut SimpleSession, id: u32) -> Result<Option<(u64, u64, u16)>, Outcome> {
    let gs = safe::send(session, Command::ViewPlayer { ident: id.to_string() }).await.map_err(|e| fail(&e))?;
    Ok(gs.lookup.lookup_pid(id).and_then(|p| p.fortress.as_ref()).map(|f| (f.lootable_wood, f.lootable_stone, f.soldier_advice)))
}

/// Soldiers (user 2026-10-12): attack with ONE soldier at a time, on the opponent the fortress offers (`attack_target`);
/// when that one has too little to loot, or a one-soldier attack was lost, on a counter-attack target (`FortressSetCAEnemy`,
/// a player who attacked us). Train soldiers up to the limit (cheap: ~15 wood + 5 stone each). Never rerolls the opponent
/// (only the FREE change, once per ~5 min, via `safe::reroll_is_free`; the paid one costs gold or a mushroom and is forbidden).
pub async fn military(session: &mut SimpleSession) -> Outcome {
    {
        let Ok(mut next) = NEXT_MILITARY.lock() else { return Outcome::Done };
        let now = Local::now();
        if next.is_some_and(|t| now < t) {
            return Outcome::Done;
        }
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
        if f.units[U::Soldier].limit == 0 {
            return Outcome::Done; // no Barracks yet
        }
        *next = Some(now + MILITARY_EVERY);
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        return fail(&e);
    }

    let min_loot = session
        .game_state()
        .and_then(|gs| gs.fortress.as_ref())
        .map(|f| f.units[U::Soldier].training.cost)
        .map_or(0, |c| c.wood + c.stone)
        .max(FALLBACK_COST)
        * LOOT_PER_COST;
    let mut attacked: Vec<u32> = Vec::new();
    let mut tried_counters: Vec<i64> = Vec::new();
    let mut use_offered = true;
    let mut rerolled = false;
    let mut sacrificed = false;
    for _ in 0..MAX_ATTACKS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
        if f.units[U::Soldier].count == 0 {
            break;
        }

        // 1. The target: the offered opponent first
        let mut target = None;
        if use_offered
            && let Some(id) = f.attack_target
            && !attacked.contains(&id)
        {
            match loot_of(session, id).await {
                Ok(Some((w, s, advice))) if advice <= 1 && w + s >= min_loot => target = Some((id, w, s)),
                Ok(Some((w, s, advice))) => report!(
                    "[fortress] The offered opponent {id} is not worth one soldier (wood {w}, stone {s}, need {min_loot}; the game advises {advice} soldiers)"
                ),
                Ok(None) => report!("[fortress] Could not read the offered opponent {id}"),
                Err(o) => return o,
            }
        }
        // 2. Otherwise a counter-attack target
        if target.is_none() {
            let Some(gs) = session.game_state() else { return Outcome::Done };
            let mut log: Vec<(i64, String, chrono::DateTime<Local>)> = gs
                .mail
                .combat_log
                .iter()
                .filter(|e| matches!(e.battle_type, CombatMessageType::FortressDefense) && !tried_counters.contains(&e.msg_id))
                .map(|e| (e.msg_id, e.player_name.clone(), e.time))
                .collect();
            log.sort_by_key(|e| e.2); // oldest first (user 2026-10-12)
            for (msg_id, name, _) in log.into_iter().take(MAX_COUNTERS) {
                tried_counters.push(msg_id);
                if let Err(err) = safe::send(session, Command::FortressSetCAEnemy { msg_id: msg_id as u32 }).await {
                    return fail(&err);
                }
                let Some(id) = session.game_state().and_then(|gs| gs.fortress.as_ref()).and_then(|f| f.attack_target) else { continue };
                match loot_of(session, id).await {
                    Ok(Some((w, s, advice))) if advice <= 1 && w + s >= min_loot && !attacked.contains(&id) => {
                        report!("[fortress] Counter-attack target {name} ({id}): wood {w}, stone {s}");
                        target = Some((id, w, s));
                        break;
                    }
                    Ok(_) => {}
                    Err(o) => return o,
                }
            }
        }
        let Some((id, w, s)) = target else {
            // Nothing worthwhile: the FREE change of the opponent (once per ~5 min, user 2026-10-12), then look again
            if !rerolled && session.game_state().is_some_and(safe::reroll_is_free) {
                rerolled = true;
                report!("[fortress] No worthwhile opponent, using the free change of the opponent");
                if let Err(e) = safe::send(session, Command::FortressNewEnemy { use_mushroom: false }).await {
                    return fail(&e);
                }
                use_offered = true;
                continue;
            }
            // Soldiers full and still nothing worthwhile (after the change, or it is not free): sacrifice ONE soldier on the offered
            // opponent – the attack (normally lost) makes the game offer a new one; the soldier is trained again below (user 2026-10-14)
            let sacrifice = (!sacrificed)
                .then(|| session.game_state().and_then(|gs| gs.fortress.as_ref()))
                .flatten()
                .filter(|f| {
                    let u = &f.units[U::Soldier];
                    u.count > 0 && u.count + u.in_training >= u.limit
                })
                .and_then(|f| f.attack_target)
                .filter(|id| !attacked.contains(id));
            if let Some(id) = sacrifice {
                sacrificed = true;
                report!("[fortress] Soldiers are full and nobody is worth it: sacrificing one soldier on {id} to get a new opponent");
                if let Err(e) = safe::send(session, Command::FortressAttack { soldiers: 1 }).await {
                    return fail(&e);
                }
                attacked.push(id);
                use_offered = true;
                tokio::time::sleep(std::time::Duration::from_secs(fastrand::u64(3..8))).await;
                continue;
            }
            report!("[fortress] No worthwhile opponent right now, soldiers wait");
            break;
        };

        // 3. Attack with one soldier
        let before = session.game_state().and_then(|gs| gs.fortress.as_ref()).map(|f| (f.resources[R::Wood].current, f.resources[R::Stone].current));
        report!("[fortress] Attacking {id} with 1 soldier (loot up to wood {w}, stone {s})");
        if let Err(e) = safe::send(session, Command::FortressAttack { soldiers: 1 }).await {
            return fail(&e);
        }
        attacked.push(id);
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let won = gs.last_fight.as_ref().is_some_and(|l| l.has_player_won);
        let honor = gs.last_fight.as_ref().map_or(0, |l| l.honor_change);
        let gained = gs.fortress.as_ref().zip(before).map(|(f, (bw, bs))| {
            (f.resources[R::Wood].current.saturating_sub(bw), f.resources[R::Stone].current.saturating_sub(bs))
        });
        if won {
            let (gw, gs_) = gained.unwrap_or((0, 0));
            report!("[fortress] Won against {id}: wood +{gw}, stone +{gs_}, fortress honor {honor:+}");
        } else {
            report!("[fortress] Lost against {id} with one soldier, using counter-attacks from now on");
            use_offered = false;
        }
        tokio::time::sleep(std::time::Duration::from_secs(fastrand::u64(3..8))).await;
    }

    // 4. Train soldiers up to the limit as far as wood and stone allow
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    let u = &f.units[U::Soldier];
    let missing = u64::from(u.limit.saturating_sub(u.count + u.in_training));
    if missing > 0 && u.in_training == 0 {
        let (wood, stone) = (f.resources[R::Wood].current, f.resources[R::Stone].current);
        let cost = u.training.cost;
        let afford = [
            if cost.wood > 0 { wood / cost.wood } else { u64::MAX },
            if cost.stone > 0 { stone / cost.stone } else { u64::MAX },
        ]
        .into_iter()
        .min()
        .unwrap_or(0);
        let mut n = missing.min(afford);
        // A Barracks upgrade about to start must not be blocked by soldiers still in training (user 2026-10-14): only as many
        // as finish by then (training is assumed sequential, one `training.cost.time` each; the finish is logged to verify it)
        if let Some(due) = barracks_due(gs) {
            let per = u.training.cost.time.as_secs().max(60) as i64;
            let fit = ((due - Local::now()).num_seconds().max(0) / per) as u64;
            if fit < n {
                report!("[fortress] Barracks upgrade due at {}: training {fit} instead of {n} soldier(s)", due.format("%H:%M"));
                n = fit;
            }
        }
        if n > 0 {
            report!("[fortress] Training {n} soldier(s) ({}/{} now, limit {})", u.count, u.in_training, u.limit);
            if let Err(e) = safe::send(session, Command::FortressBuildUnit { unit: U::Soldier, count: n as u32 }).await {
                return fail(&e);
            }
            if let Some(u) = session.game_state().and_then(|gs| gs.fortress.as_ref()).map(|f| &f.units[U::Soldier])
                && let Some(fin) = u.training.finish
            {
                report!("[check] Training {n} soldier(s) finishes at {} (price table time per soldier: {} s)", fin.format("%H:%M:%S"), u.training.cost.time.as_secs());
            }
        }
    }
    Outcome::Done
}

/// Seconds until this character's fortress needs attention (for the main loop's sleep): a build finishing, the soldiers'
/// next look, the XP/wood/stone production getting close to full. `None` when there is no fortress.
pub fn secs_until_due(gs: &GameState) -> Option<u64> {
    let f = gs.fortress.as_ref()?;
    let now = Local::now();
    let secs = |t: chrono::DateTime<Local>| (t - now).num_seconds().max(0) as u64;
    let mut due: Vec<u64> = Vec::new();
    if let Some(fin) = f.building_upgrade.finish.filter(|_| f.building_upgrade.target.is_some()) {
        due.push(secs(fin));
    }
    if f.units[U::Soldier].limit > 0
        && let Ok(next) = NEXT_MILITARY.lock()
        && let Some(t) = *next
    {
        due.push(secs(t));
    }
    for r in [R::Wood, R::Stone, R::Experience] {
        if let Some(m) = minutes_to_full(gs, r) {
            due.push(m.saturating_sub(WAKE_BEFORE_FULL_MIN) * 60);
        }
    }
    due.into_iter().min()
}

fn building_name(b: B) -> &'static str {
    match b {
        B::Fortress => "Fortress",
        B::LaborersQuarters => "Laborers' Quarters",
        B::WoodcuttersHut => "Woodcutter's Hut",
        B::Quarry => "Quarry",
        B::GemMine => "Gem Mine",
        B::Academy => "Academy",
        B::ArcheryGuild => "Archery Guild",
        B::Barracks => "Barracks",
        B::MagesTower => "Mages' Tower",
        B::Treasury => "Treasury",
        B::Smithy => "Smithy",
        B::Wall => "Wall",
    }
}

/// What the next build still waits for, as a short text ("wood 30 missing", "gold", "Barracks busy training", …).
fn waiting_for(gs: &GameState, f: &Fortress, b: B) -> Option<String> {
    let c = f.buildings[b].upgrade_cost;
    let mut miss = Vec::new();
    if c.wood > f.resources[R::Wood].current {
        miss.push(format!("{} wood", c.wood - f.resources[R::Wood].current));
    }
    if c.stone > f.resources[R::Stone].current {
        miss.push(format!("{} stone", c.stone - f.resources[R::Stone].current));
    }
    if c.silver > gs.character.silver {
        miss.push(format!("{} gold", crate::report::gold(c.silver - gs.character.silver)));
    }
    if f.in_use(b) {
        miss.push("units in training".to_string());
    }
    (!miss.is_empty()).then(|| format!("missing {}", miss.join(", ")))
}

/// The Fortress tab's data (`roster/<character>/fortress.json`, user 2026-10-14): building levels, the running build, the
/// upcoming plan, resources (current/limit, uncollected production), soldiers, Hall of Knights, gem search. `builds` is
/// false for the attack-only character (no plan shown).
pub fn snapshot_json(gs: &GameState, builds: bool) -> Option<serde_json::Value> {
    let f = gs.fortress.as_ref()?;
    let now = Local::now();
    let levels: Vec<serde_json::Value> = [
        B::Fortress,
        B::LaborersQuarters,
        B::WoodcuttersHut,
        B::Quarry,
        B::Academy,
        B::Barracks,
        B::Treasury,
        B::GemMine,
    ]
    .into_iter()
    .map(|b| serde_json::json!({ "name": building_name(b), "level": f.buildings[b].level }))
    .collect();
    let running = f.building_upgrade.target.map(|b| {
        serde_json::json!({
            "name": building_name(b),
            "to": f.buildings[b].level + 1,
            "finish": f.building_upgrade.finish.map(|t| t.to_rfc3339()),
        })
    });
    let next = builds.then(|| choose(f, gs.character.level)).flatten().map(|b| {
        let c = f.buildings[b].upgrade_cost;
        serde_json::json!({
            "name": building_name(b),
            "to": f.buildings[b].level + 1,
            "minutes": c.time.as_secs() / 60,
            "waiting": waiting_for(gs, f, b),
        })
    });
    let after: Vec<serde_json::Value> = if builds {
        // the first of `upcoming` is the next build (or the one after the running one); skip the one shown as "next"
        let skip = usize::from(running.is_none());
        upcoming(f, gs.character.level, 4 + skip)
            .into_iter()
            .skip(skip)
            .map(|(b, l)| serde_json::json!({ "name": building_name(b), "to": l }))
            .collect()
    } else {
        Vec::new()
    };
    let res = |r: R, name: &str| {
        let x = &f.resources[r];
        serde_json::json!({
            "name": name,
            "current": x.current,
            "limit": x.limit,
            "ready": collectable(gs, r),
            "prodLimit": x.production.limit,
            "perHour": x.production.per_hour,
        })
    };
    let u = &f.units[U::Soldier];
    Some(serde_json::json!({
        "updated": now.to_rfc3339(),
        "builds": builds,
        "buildings": levels,
        "running": running,
        "next": next,
        "after": after,
        "hallOfKnights": f.hall_of_knights_level,
        "resources": [res(R::Wood, "Wood"), res(R::Stone, "Stone"), res(R::Experience, "XP")],
        "soldiers": {
            "count": u.count, "training": u.in_training, "limit": u.limit, "level": u.level,
            "trainingFinish": u.training.finish.filter(|t| *t > now).map(|t| t.to_rfc3339()),
        },
        "gemSearch": f.gem_search.target.map(|_| serde_json::json!({
            "finish": f.gem_search.finish.map(|t| t.to_rfc3339()),
        })),
        "honor": f.honor,
        "rank": f.rank,
    }))
}

/// Writes `roster/<character>/fortress.json` (at most every 30 s unless it changed; local only).
pub fn write_snapshot(gs: &GameState, builds: bool) {
    static LAST: crate::ctx::PerChar<Option<(String, std::time::Instant)>> = crate::ctx::PerChar::new();
    let Some(mut j) = snapshot_json(gs, builds) else { return };
    j["updated"] = serde_json::Value::Null;
    let body = j.to_string();
    let Ok(mut last) = LAST.lock() else { return };
    if last.as_ref().is_some_and(|(b, t)| *b == body && t.elapsed() < std::time::Duration::from_secs(30)) {
        return;
    }
    *last = Some((body, std::time::Instant::now()));
    drop(last);
    let Some(j) = snapshot_json(gs, builds) else { return };
    let dir = std::path::Path::new("roster").join(crate::ctx::name());
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("fortress.json"), j.to_string());
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
        // after Fortress 3 + Woodcutter 3 + Quarry 3: Fortress 4 straight away, then the Barracks, only then the rest
        let l3 = &[(B::Fortress, 3), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 2), (B::Treasury, 1)];
        assert_eq!(pick(at(l3), 20), Some(B::Fortress));
        let f4 = &[(B::Fortress, 4), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 2), (B::Treasury, 1)];
        assert_eq!(pick(at(f4), 20), Some(B::Barracks));
        let b1 = &[(B::Fortress, 4), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 2), (B::Treasury, 1), (B::Barracks, 1)];
        assert_eq!(pick(at(b1), 20), Some(B::GemMine));
        let all = &[(B::Fortress, 4), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::GemMine, 1), (B::Treasury, 2), (B::LaborersQuarters, 3), (B::Barracks, 1)];
        assert_eq!(pick(at(all), 20), Some(B::Fortress)); // the plan continues with Fortress 5, 6 and the Academy (user 2026-10-12)
    }

    fn generic(levels: &'static [(B, u16)], char_level: u16) -> Option<B> {
        pick_generic(&at(levels), char_level, 20)
    }

    #[test]
    fn generic_rules_after_the_plan() {
        // end of the dictated plan: Fortress 6 and the Academy are in it (user 2026-10-12: rush Fortress 6)
        let p = &[(B::Fortress, 4), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 3), (B::Treasury, 2), (B::Barracks, 1), (B::GemMine, 1)];
        assert_eq!(pick(at(p), 20), Some(B::Fortress));
        let f6 = &[(B::Fortress, 6), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 3), (B::Treasury, 2), (B::Barracks, 1), (B::GemMine, 1)];
        assert_eq!(pick(at(f6), 20), Some(B::Academy));
        let done = &[(B::Fortress, 6), (B::WoodcuttersHut, 3), (B::Quarry, 3), (B::LaborersQuarters, 3), (B::Treasury, 2), (B::Barracks, 1), (B::GemMine, 1), (B::Academy, 6)];
        assert_eq!(pick(at(done), 20), None);
        // generic: no Woodcutter/Quarry any more; Laborers' Quarters up to the Fortress level first
        assert_eq!(generic(done, 30), Some(B::LaborersQuarters));
        let lq = &[(B::Fortress, 6), (B::LaborersQuarters, 6), (B::Treasury, 2), (B::Barracks, 1), (B::GemMine, 1), (B::Academy, 6)];
        // the Academy is rushed up to the Fortress level before the Barracks (user 2026-10-14)
        let lq1 = &[(B::Fortress, 6), (B::LaborersQuarters, 6), (B::Treasury, 2), (B::Barracks, 1), (B::GemMine, 1), (B::Academy, 1)];
        assert_eq!(generic(lq1, 30), Some(B::Academy));
        // then the Barracks up to 4
        assert_eq!(generic(lq, 30), Some(B::Barracks));
        let b4 = &[(B::Fortress, 6), (B::LaborersQuarters, 6), (B::Treasury, 2), (B::Barracks, 4), (B::GemMine, 1), (B::Academy, 6)];
        // the Treasury to half the Fortress level (3 of 6), then the next Fortress level
        assert_eq!(generic(b4, 30), Some(B::Treasury));
        let t3 = &[(B::Fortress, 6), (B::LaborersQuarters, 6), (B::Treasury, 3), (B::Barracks, 4), (B::GemMine, 1), (B::Academy, 6)];
        assert_eq!(generic(t3, 30), Some(B::Fortress));
        // Fortress 7: Laborers' Quarters first again; the Treasury goes to 4 (half of 7 rounded up)
        let f7 = &[(B::Fortress, 7), (B::LaborersQuarters, 7), (B::Academy, 7), (B::Treasury, 3), (B::Barracks, 4), (B::GemMine, 1)];
        assert_eq!(generic(f7, 30), Some(B::Treasury));
        // the Gem Mine only goes further from character level 125; never the Wall
        let g = &[(B::Fortress, 8), (B::LaborersQuarters, 8), (B::Academy, 8), (B::Treasury, 4), (B::Barracks, 4), (B::GemMine, 1)];
        assert_eq!(generic(g, 30), Some(B::Fortress));
        assert_eq!(generic(g, 125), Some(B::GemMine));
        assert_eq!(generic(&[], 30), Some(B::Fortress));
    }
}
