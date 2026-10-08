//! Hall of Fame hunt for Gleeman/event fight tasks (user 2026-10-07): "win N fights against <class>",
//! "win fights bare-handed" (weapon off), "win fights without a chest plate" (chest plate off).
//! Hall of Fame fights do not count towards the 10 Arena wins for XP, so the hunt runs only when those are done,
//! or late in the evening when the task would otherwise stay open. It uses the Arena cooldown like any fight.
//! Opponent: far below us in the Hall of Fame (rank + offset), lowest honor, of the wanted class, clearly weaker.

use chrono::{Local, Timelike};
use sf_api::{
    command::{AttributeType, Command},
    gamestate::{GameState, character::Class, items::EquipmentSlot, rewards::TaskType},
};

use crate::{
    arena::{self, strength},
    safe,
    session::SimpleSession,
    tavern::Outcome,
};

/// From this hour the hunt runs even when the 10 XP wins are not done yet.
const LATE_HOUR: u32 = 21;
/// How far below our rank to look (tried in this order).
const RANK_OFFSETS: [u32; 3] = [1500, 3000, 6000];
/// How many of the lowest-honor players of one page to inspect (each one is a `ViewPlayer`).
const INSPECT: usize = 3;

/// What the fight has to look like.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hunt {
    Class(Class),
    /// Fight with these equipment slots empty
    Without(EquipmentSlot),
}

impl Hunt {
    /// Required opponent strength relative to ours: without a weapon we are much weaker.
    fn max_ratio(self) -> f64 {
        match self {
            Hunt::Class(_) => 0.6,
            Hunt::Without(EquipmentSlot::Weapon) => 0.3,
            Hunt::Without(_) => 0.5,
        }
    }
}

fn wanted_hunt(gs: &GameState) -> Option<Hunt> {
    crate::tasks::open_tasks(gs).find_map(|t| match t.typ {
        TaskType::WinFightsAgainst(c) => Some(Hunt::Class(c)),
        TaskType::WinFightsBareHands => Some(Hunt::Without(EquipmentSlot::Weapon)),
        TaskType::WinFightsNoChestplate => Some(Hunt::Without(EquipmentSlot::BreastPlate)),
        _ => None,
    })
}

/// Our own strength by the Arena formula.
pub fn own_strength(gs: &GameState) -> f64 {
    let ch = &gs.character;
    strength(ch.class, |a: AttributeType| f64::from(ch.attribute_basis[a] + ch.attribute_additions[a]))
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[hunt] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Finds a clearly weaker opponent for the hunt. Returns the name.
async fn find_opponent(session: &mut SimpleSession, hunt: Hunt) -> Result<Option<String>, Outcome> {
    let Some(gs) = session.game_state() else { return Err(Outcome::Done) };
    let (our_rank, our_level, ours) = (gs.character.rank, u32::from(gs.character.level), own_strength(gs));
    let total = gs.hall_of_fames.players_total.max(our_rank);
    let limit = ours * hunt.max_ratio();
    for offset in RANK_OFFSETS {
        let target = (our_rank + offset).min(total.saturating_sub(25)).max(26);
        let page = ((target - 26) / 51) as usize;
        let gs = safe::send(session, Command::HallOfFamePage { page }).await.map_err(|e| fail(&e))?;
        let mut list: Vec<_> = gs
            .hall_of_fames
            .players
            .iter()
            .filter(|p| {
                p.level <= our_level
                    && match hunt {
                        Hunt::Class(c) => p.class == c,
                        Hunt::Without(_) => true,
                    }
            })
            .map(|p| (p.honor, p.name.clone()))
            .collect();
        list.sort();
        for (_, name) in list.into_iter().take(INSPECT) {
            let gs = safe::send(session, Command::ViewPlayer { ident: name.clone() }).await.map_err(|e| fail(&e))?;
            let Some(p) = gs.lookup.lookup_name(&name) else { continue };
            let s = strength(p.class, |a| arena::total(p, a));
            if s <= limit {
                report!("[hunt] Found {name} (rank {}, lvl {}, {:?}): strength {s:.0}, ours {ours:.0}", p.rank, p.level, p.class);
                return Ok(Some(name));
            }
        }
    }
    Ok(None)
}

/// One hunt fight if a fight task is open, the Arena is free and the XP wins are done (or it is late).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(hunt) = wanted_hunt(gs) else { return Outcome::Done };
    let late = Local::now().hour() >= LATE_HOUR;
    if !safe::arena_is_free(gs) || (usize::from(gs.arena.fights_for_xp) < arena::MAX_WINS_PER_DAY && !late) {
        return Outcome::Done;
    }
    report!("[hunt] Task {hunt:?}: looking for a clearly weaker opponent in the Hall of Fame");
    let name = match find_opponent(session, hunt).await {
        Ok(Some(n)) => n,
        Ok(None) => {
            report!("[hunt] No clearly weaker opponent found");
            return Outcome::Done;
        }
        Err(o) => return o,
    };

    // Take the item(s) off (weapon / chest plate) into free backpack slots. An Assassin carries a weapon in
    // both the Weapon and Shield slot (sf-api types the off-hand item as a Weapon too), so "bare hands" must
    // take both off, not just the main hand.
    let mut stripped: Vec<(EquipmentSlot, sf_api::gamestate::items::BagPosition, sf_api::gamestate::items::ItemCommandIdent)> =
        Vec::new();
    if let Hunt::Without(slot) = hunt {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let mut slots = vec![slot];
        if slot == EquipmentSlot::Weapon && gs.character.class == Class::Assassin {
            slots.push(EquipmentSlot::Shield);
        }
        for slot in slots {
            let Some(gs) = session.game_state() else { return Outcome::Done };
            let Some(item) = gs.character.equipment.0[slot].as_ref() else { continue };
            let Some(bag) = gs.character.inventory.free_slot() else {
                report!("[hunt] Backpack full, cannot take off the {slot:?}");
                // Put back whatever was already stripped this run before giving up
                for (slot, bag, item_ident) in stripped {
                    let _ = safe::send(session, Command::Equip { from_pos: bag.into(), to_slot: slot, item_ident }).await;
                }
                return Outcome::Done;
            };
            let item_ident = item.command_ident();
            report!("[hunt] Taking off the {slot:?}");
            if let Err(e) =
                safe::send(session, Command::PlayerItemMove { from: slot.into(), to: bag.into(), item_ident }).await
            {
                return fail(&e);
            }
            stripped.push((slot, bag, item_ident));
        }
    }

    report!("[hunt] Fighting {name}");
    let session_lost = match safe::send(session, Command::Fight { name: name.clone(), use_mushroom: false }).await {
        Ok(gs) => {
            match &gs.last_fight {
                Some(f) => report!("[hunt] {} against {name}", if f.has_player_won { "Win" } else { "Loss" }),
                None => report!("[hunt] Fight done, the server sent no result"),
            }
            false
        }
        Err(e) => {
            report!("[hunt] Fight failed: {e}");
            crate::tavern::is_session_error(&e)
        }
    };

    // Always put the item(s) back on
    for (slot, bag, item_ident) in stripped {
        report!("[hunt] Putting the {slot:?} back on");
        if let Err(e) = safe::send(session, Command::Equip { from_pos: bag.into(), to_slot: slot, item_ident }).await {
            // inventory::manage will equip it on the next pass anyway (better than an empty slot)
            return fail(&e);
        }
    }
    if session_lost { Outcome::SessionLost } else { Outcome::Done }
}
