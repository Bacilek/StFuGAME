//! Hall of Fame hunt for Gleeman/event fight tasks (user 2026-10-07): "win N fights against <class>",
//! "win fights bare-handed" (weapon off), "win fights without a chest plate" (chest plate off).
//! Hall of Fame fights do not count towards the 10 Arena wins for XP, but the tasks come first (user 2026-10-10): the hunt
//! runs BEFORE the Arena XP fights whenever such a task is open and the Arena is free (same cooldown). Opponent: from the
//! very bottom of the Hall of Fame upwards (level 1 players live there), lowest honor, wanted class, clearly weaker.

use sf_api::{
    command::{AttributeType, Command},
    gamestate::{GameState, character::Class, items::EquipmentSlot, rewards::TaskType},
};

use crate::{
    arena::{self, power},
    safe,
    session::SimpleSession,
    tavern::Outcome,
};

/// How many Hall of Fame pages (51 players each) to look through, starting at the very bottom and going up.
const BOTTOM_PAGES: u32 = 6;
/// How many of the lowest-honor players of one page to inspect (each one is a `ViewPlayer`).
const INSPECT: usize = 3;

/// What the fight has to look like.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hunt {
    Class(Class),
    /// Fight with these equipment slots empty
    Without(EquipmentSlot),
    /// Fight with every epic/legendary item taken off (user 2026-10-10)
    NoEpics,
}

impl Hunt {
    /// Required opponent power relative to ours: without a weapon we are much weaker.
    fn max_ratio(self) -> f64 {
        match self {
            Hunt::Class(_) => 0.6,
            Hunt::Without(EquipmentSlot::Weapon) => 0.3,
            Hunt::Without(_) | Hunt::NoEpics => 0.5,
        }
    }
}

/// Every kind of hunt an open task asks for, in task order (the first one that fits in the backpack is fought).
fn wanted_hunts(gs: &GameState) -> Vec<Hunt> {
    crate::tasks::open_tasks(gs).filter_map(|t| match t.typ {
        TaskType::WinFightsAgainst(c) => Some(Hunt::Class(c)),
        TaskType::WinFightsBareHands => Some(Hunt::Without(EquipmentSlot::Weapon)),
        TaskType::WinFightsNoChestplate => Some(Hunt::Without(EquipmentSlot::BreastPlate)),
        TaskType::WinFightsNoEpicsLegendaries => Some(Hunt::NoEpics),
        _ => None,
    })
    .collect()
}

/// Our own power by the Arena formula.
pub fn own_power(gs: &GameState) -> f64 {
    let ch = &gs.character;
    power(ch.class, |a: AttributeType| f64::from(ch.attribute_basis[a] + ch.attribute_additions[a]))
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[hunt] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Finds a clearly weaker opponent for the hunt. Returns the name.
async fn find_opponent(session: &mut SimpleSession, hunt: Hunt) -> Result<Option<String>, Outcome> {
    let Some(gs) = session.game_state() else { return Err(Outcome::Done) };
    let (our_rank, our_level, ours) = (gs.character.rank, u32::from(gs.character.level), own_power(gs));
    let mut total = gs.hall_of_fames.players_total;
    let limit = ours * hunt.max_ratio();
    if total <= our_rank {
        // Size of the Hall of Fame not known yet: one page near our own rank tells it
        let page = (our_rank.saturating_sub(26) / 51) as usize;
        let gs = safe::send(session, Command::HallOfFamePage { page }).await.map_err(|e| fail(&e))?;
        total = gs.hall_of_fames.players_total.max(our_rank);
    }
    // From the very bottom upwards: the lowest ranks are level 1 players, a clearly weaker opponent is always there
    for k in 0..BOTTOM_PAGES {
        let target = total.saturating_sub(25 + 51 * k);
        if target <= our_rank || target < 26 {
            break;
        }
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
                        Hunt::Without(_) | Hunt::NoEpics => true,
                    }
            })
            .map(|p| (p.honor, p.name.clone()))
            .collect();
        list.sort();
        for (_, name) in list.into_iter().take(INSPECT) {
            let gs = safe::send(session, Command::ViewPlayer { ident: name.clone() }).await.map_err(|e| fail(&e))?;
            let Some(p) = gs.lookup.lookup_name(&name) else { continue };
            let s = power(p.class, |a| arena::total(p, a));
            if s <= limit {
                report!("[hunt] Found {name} (rank {}, lvl {}, {:?}): power {s:.0}, ours {ours:.0}", p.rank, p.level, p.class);
                return Ok(Some(name));
            }
        }
    }
    Ok(None)
}

/// The equipment slots to empty for the fight (`None` = nothing to take off for this kind of hunt).
/// An Assassin carries a weapon in both the Weapon and Shield slot (sf-api types the off-hand item as a Weapon too),
/// so "bare hands" must take both off, not just the main hand.
fn strip_slots(gs: Option<&GameState>, hunt: Hunt) -> Option<Vec<EquipmentSlot>> {
    let gs = gs?;
    match hunt {
        Hunt::Class(_) => None,
        Hunt::Without(slot) => {
            let mut slots = vec![slot];
            if slot == EquipmentSlot::Weapon && gs.character.class == Class::Assassin {
                slots.push(EquipmentSlot::Shield);
            }
            Some(slots)
        }
        Hunt::NoEpics => Some(
            gs.character.equipment.0.iter().filter(|(_, i)| i.as_ref().is_some_and(|i| i.is_epic())).map(|(s, _)| s).collect(),
        ),
    }
}

/// Makes sure the items this hunt takes off fit into the backpack, freeing slots with potions when short (drink a
/// better/stacking one or sell the least important, `potions::make_room`). `false` = does not fit, try another hunt.
async fn ensure_room(session: &mut SimpleSession, hunt: Hunt) -> Result<bool, Outcome> {
    let Some(slots) = strip_slots(session.game_state(), hunt) else { return Ok(true) };
    let count = |s: &SimpleSession| s.game_state().map_or(0, |gs| gs.character.inventory.count_free_slots());
    let mut free = count(session);
    // E.g. an Assassin needs 2 slots for bare hands; Sanek's fight tasks stayed blocked all day with 1 free slot
    while free < slots.len() {
        if !crate::potions::make_room(session).await? {
            break;
        }
        free = count(session);
    }
    if free < slots.len() {
        let held = session.game_state().map_or_else(String::new, |gs| {
            gs.character.inventory.backpack.iter().flatten().map(crate::inventory::detail).collect::<Vec<_>>().join("; ")
        });
        report!("[hunt] {hunt:?}: backpack has {free} free slot(s), {} needed to take the items off, skipping (holding: {held})", slots.len());
        return Ok(false);
    }
    Ok(true)
}

/// One hunt fight if a fight task is open and the Arena is free (tasks go before the XP fights, user 2026-10-10).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let hunts = wanted_hunts(gs);
    if hunts.is_empty() || !safe::arena_is_free(gs) {
        return Outcome::Done;
    }
    let mut chosen = None;
    for hunt in hunts {
        match ensure_room(session, hunt).await {
            Ok(true) => {
                chosen = Some(hunt);
                break;
            }
            Ok(false) => {}
            Err(o) => return o,
        }
    }
    let Some(hunt) = chosen else { return Outcome::Done };
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
    if let Some(slots) = strip_slots(session.game_state(), hunt) {
        // (room for all of them was made in `ensure_room` before the opponent search)
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
