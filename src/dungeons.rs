//! Dungeons: one fight whenever they are off cooldown (even during an expedition).
//! Among unlocked dungeons picks the one with the lowest-level enemy, the weaker one at a similar level.
//! Never for mushrooms, never with a full inventory.

use sf_api::{
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        dungeons::{Dungeon, DungeonProgress, LightDungeon},
        rewards::TaskType,
    },
};

use crate::session::SimpleSession;

use crate::{arena::strength, safe, tavern::Outcome};

/// How often at most to retry the Dungeons when no fight happened (full inventory etc.).
pub const RETRY_SEC: u64 = 5 * 60;

/// Level difference considered "similar" (then strength by stats decides).
const SIMILAR_LEVELS: u16 = 2;

/// Dungeon enemy: level and strength (same formula as in the Arena).
#[derive(Debug, Clone)]
pub struct Candidate {
    pub dungeon: Dungeon,
    pub name: String,
    pub level: u16,
    pub strength: f64,
}

/// All open dungeons fightable with FightDungeon (the Tower has its own command).
fn open_dungeons(gs: &GameState) -> Vec<Dungeon> {
    let is_open = |p: &DungeonProgress| matches!(p, DungeonProgress::Open { .. });
    let light = gs.dungeons.light.iter().filter(|(d, p)| *d != LightDungeon::Tower && is_open(p)).map(|(d, _)| Dungeon::Light(d));
    let shadow = gs.dungeons.shadow.iter().filter(|(_, p)| is_open(p)).map(|(d, _)| Dungeon::Shadow(d));
    light.chain(shadow).collect()
}

/// Enemies in all open dungeons.
pub fn candidates(gs: &GameState) -> Vec<Candidate> {
    let ch = &gs.character;
    open_dungeons(gs)
        .into_iter()
        .filter_map(|d| {
            let m = gs.dungeons.current_enemy(d)?;
            // The "mirror image" is a level 0 warrior in sf-api: it is a copy of our character
            let c = if m.level == 0 {
                let stat = |a: AttributeType| f64::from(ch.attribute_basis[a] + ch.attribute_additions[a]);
                Candidate { dungeon: d, name: "mirror image".into(), level: ch.level, strength: strength(ch.class, stat) }
            } else {
                let stat = |a: AttributeType| f64::from(m.attributes[a]);
                Candidate { dungeon: d, name: m.name.into(), level: m.level, strength: strength(m.class, stat) }
            };
            Some(c)
        })
        .collect()
}

/// Lowest level; among enemies of a similar level (up to +SIMILAR_LEVELS) the one with the lowest strength.
pub fn choose(cands: &[Candidate]) -> Option<&Candidate> {
    let min_level = cands.iter().map(|c| c.level).min()?;
    cands
        .iter()
        .filter(|c| c.level <= min_level + SIMILAR_LEVELS)
        .min_by(|a, b| a.strength.total_cmp(&b.strength))
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[dungeons] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Pending unlocks the bot accepts: 30 = the dungeons (captured from the game client, `UnlockFeature` `30/1`) and
/// 5 = the Scrapbook (user 2026-10-09: wants it on every character; inferred from its `scrapbook.r` arriving when `5/1`
/// left the pending list). Other pending idents (seen: 9/1, 40/1) are unknown and left alone.
const ACCEPTED_UNLOCK_IDENTS: [i64; 2] = [30, 5];

/// One Dungeons fight if possible right now. Otherwise does nothing.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    // The Dungeons timer is only refreshed by UpdateDungeons (neither Update nor a fight refreshes it)
    let gs = match safe::send(session, Command::UpdateDungeons).await {
        Ok(gs) => gs,
        Err(e) => return fail(&e),
    };
    // A pending dungeon unlock (ident 30) must be accepted first, otherwise the dungeon stays Locked (the game client
    // does the same when the Dungeons screen is opened); then the Dungeons state is refreshed
    let todo: Vec<_> = gs.pending_unlocks.iter().filter(|u| ACCEPTED_UNLOCK_IDENTS.contains(&u.main_ident)).copied().collect();
    let gs = if todo.is_empty() {
        gs
    } else {
        for u in todo {
            report!("[dungeons] Unlocking feature ({}/{})", u.main_ident, u.sub_ident);
            if let Err(e) = safe::send(session, Command::UnlockFeature { unlockable: u }).await {
                return fail(&e);
            }
        }
        match safe::send(session, Command::UpdateDungeons).await {
            Ok(gs) => gs,
            Err(e) => return fail(&e),
        }
    };
    if !safe::dungeon_is_free(gs) {
        return Outcome::Done;
    }
    if gs.character.inventory.count_free_slots() == 0 {
        report!("[dungeons] Inventory full, skipping the fight");
        return Outcome::Done;
    }
    let cands = candidates(gs);
    // A Gleeman/event task "defeat monsters in <dungeon>" → fight there while the task is open
    let wanted = cands.iter().find(|c| {
        crate::tasks::remaining(gs, |t| matches!(t, TaskType::DefeatMonstersLightDungeon(l) if c.dungeon == Dungeon::Light(l))) > 0
    });
    if let Some(w) = wanted {
        report!("[dungeons] Task: defeat monsters in {:?}", w.dungeon);
    }
    let Some(pick) = wanted.or_else(|| choose(&cands)).cloned() else {
        // Diagnostics (user 2026-10-09: every character is level 16 but only Training Camp shows up): the server lists
        // features waiting for an explicit unlock in `pending_unlocks`; this bot never sends UnlockFeature yet.
        let pending: Vec<String> = gs.pending_unlocks.iter().map(|u| format!("{}/{}", u.main_ident, u.sub_ident)).collect();
        report!("[dungeons] No open dungeon (pending unlocks: {})", if pending.is_empty() { "none".to_string() } else { pending.join(", ") });
        return Outcome::Done;
    };
    report!("[dungeons] Fighting: {:?} – {} (lvl {})", pick.dungeon, pick.name, pick.level);

    let gs = match safe::send(session, Command::FightDungeon { dungeon: pick.dungeon, use_mushroom: false }).await {
        Ok(gs) => gs,
        Err(e) => return fail(&e),
    };
    match &gs.last_fight {
        Some(f) => report!(
            "[dungeons] {}: xp +{}, gold {}, item {}",
            if f.has_player_won { "Win" } else { "Loss" },
            f.xp_change,
            crate::report::gold_change(f.silver_change),
            if f.item_won.is_some() { "yes" } else { "no" }
        ),
        None => report!("[dungeons] Fight done, the server sent no result"),
    }
    // New cooldown time (without it the guard in safe.rs will not allow another fight)
    match safe::send(session, Command::UpdateDungeons).await {
        Ok(gs) => {
            if let Some(t) = gs.dungeons.next_free_fight {
                report!("[dungeons] Next free fight at {}", t.format("%H:%M:%S"));
            }
            Outcome::Done
        }
        Err(e) => fail(&e),
    }
}

/// Seconds until the Dungeons are free (with margin). An estimate without a fresh state.
pub fn secs_until_ready(gs: &GameState) -> u64 {
    let free_at = gs.dungeons.next_free_fight.map_or_else(chrono::Local::now, |t| {
        t + chrono::Duration::seconds(safe::COOLDOWN_SAFETY_SEC)
    });
    u64::try_from((free_at - chrono::Local::now()).num_seconds()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(level: u16, strength: f64) -> Candidate {
        Candidate { dungeon: Dungeon::Light(LightDungeon::DesecratedCatacombs), name: String::new(), level, strength }
    }

    #[test]
    fn lowest_level_wins() {
        let cands = [c(20, 100.0), c(10, 900.0), c(30, 50.0)];
        assert_eq!(choose(&cands).unwrap().level, 10);
    }

    #[test]
    fn similar_level_weaker_stats() {
        let cands = [c(10, 900.0), c(11, 500.0), c(15, 100.0)];
        assert_eq!(choose(&cands).unwrap().level, 11);
    }
}
