//! Toilet of the Arcane Gods (user 2026-10-25, `docs/toilet.md`): the epics we keep go here. One sacrifice a day (preferably a gem),
//! two during Tidy Toilet Time (preferably an epic); a full backpack makes an epic the preferred sacrifice. Flushing (+1 aura, a random
//! item that the normal inventory pass then equips, dismantles, enchants or sells) whenever the mana reaches the flush cost.
//! Potions and plain items are never thrown.

use chrono::{Local, NaiveDate};
use sf_api::{
    command::Command,
    error::SFError,
    gamestate::{GameState, items::{BagPosition, Item, ItemType}, rewards::Event},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// Most flushes in one pass (the mana pool is small, this is only a loop guard).
const MAX_FLUSHES: usize = 20;

/// Marker for "the server refused a sacrifice today".
const REFUSED: u32 = 99;

fn day_file() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("toilet_day.txt")
}

/// Sacrifices made today (`REFUSED` = the server said no); persisted so a restart does not lose count.
fn thrown_today() -> u32 {
    let text = std::fs::read_to_string(day_file()).unwrap_or_default();
    let mut it = text.split_whitespace();
    let (Some(day), Some(n)) = (it.next(), it.next()) else { return 0 };
    if day.parse::<NaiveDate>().ok() != Some(Local::now().date_naive()) {
        return 0;
    }
    n.parse().unwrap_or(0)
}

fn set_thrown_today(n: u32) {
    let _ = std::fs::write(day_file(), format!("{} {n}", Local::now().date_naive()));
}

pub fn event(gs: &GameState) -> bool {
    crate::tasks::event_active(gs, Event::TidyToiletTime)
}

/// An epic equipment piece or a gem may be thrown in; never a unique, a legendary, a potion or a plain item.
pub fn throwable(item: &Item) -> bool {
    match item.typ {
        ItemType::Gem(_) => true,
        _ => item.is_epic() && !item.is_legendary() && !item.is_unique() && item.typ.equipment_slot().is_some(),
    }
}

fn describe(item: &Item) -> String {
    match item.typ {
        ItemType::Gem(g) => format!("{:?} gem", g.typ),
        t => format!("epic {t:?}"),
    }
}

/// The cheapest backpack epic (the dearer ones may still be worth something to the witch or the shop).
fn spare_epic(gs: &GameState) -> Option<(BagPosition, String)> {
    gs.character
        .inventory
        .iter()
        .filter_map(|(pos, i)| Some((pos, i?)))
        .filter(|(_, i)| i.is_epic() && throwable(i) && !matches!(i.typ, ItemType::Gem(_)))
        .min_by_key(|(_, i)| i.price)
        .map(|(pos, i)| (pos, describe(i)))
}

/// What to throw now, or None. `thrown` = sacrifices made today. One a day outside the event (a gem; an epic only with a full backpack),
/// two during Tidy Toilet Time (epics first). The server's `sacrifices_left` decides too, except that a day with no throw yet is tried once
/// (the counter is only refreshed by responses that carry it).
pub fn candidate(gs: &GameState, thrown: u32) -> Option<(BagPosition, String)> {
    let toilet = gs.tavern.toilet.as_ref()?;
    let event = event(gs);
    if thrown >= if event { 2 } else { 1 } || (thrown > 0 && toilet.sacrifices_left == 0) {
        return None;
    }
    let gem = || crate::inventory::spare_gem(gs).map(|(pos, _)| (pos, "gem".to_string()));
    let full = gs.character.inventory.free_slot().is_none();
    if event || full {
        spare_epic(gs).or_else(gem)
    } else {
        gem()
    }
}

/// Is the toilet in charge of getting rid of epics (unlocked)? Then no epic is sold or dismantled for a free slot, it waits for the next
/// sacrifice (`inventory::last_resort_epic`).
pub fn owns_epics(gs: &GameState) -> bool {
    gs.tavern.toilet.is_some()
}

fn state(gs: &GameState) -> String {
    gs.tavern.toilet.as_ref().map_or("none".to_string(), |t| {
        format!("aura {}, mana {}/{}, {} sacrifices left", t.aura, t.mana_currently, t.mana_total, t.sacrifices_left)
    })
}

fn fail(e: &SFError) -> Outcome {
    if crate::tavern::is_session_error(e) {
        return Outcome::SessionLost;
    }
    report!("[toilet] Failed: {e}");
    Outcome::Done
}

/// One sacrifice if there is something to throw, then flushes while the mana allows (and a backpack slot is free for the new item).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    if session.game_state().is_none_or(|gs| gs.tavern.toilet.is_none()) {
        return Outcome::Done;
    }
    let thrown = thrown_today();
    if let Some((pos, what)) = session.game_state().and_then(|gs| candidate(gs, thrown)) {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let ident = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()).map(Item::command_ident);
        let was = state(gs);
        match safe::send(session, Command::ToiletDrop { item_pos: pos.into() }).await {
            Err(e) => {
                if matches!(e, SFError::ServerError(_)) {
                    set_thrown_today(REFUSED);
                }
                return fail(&e);
            }
            Ok(_) => {}
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let gone = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()).map(Item::command_ident) != ident;
        if gone {
            set_thrown_today(thrown + 1);
            report!("[toilet] Threw a {what} in{}: {was} -> {}", if event(gs) { " (Tidy Toilet Time)" } else { "" }, state(gs));
        } else {
            set_thrown_today(REFUSED);
            report!("[toilet] The {what} is still in the backpack after the sacrifice, no more today ({})", state(gs));
        }
    }
    flush(session).await
}

async fn flush(session: &mut SimpleSession) -> Outcome {
    for _ in 0..MAX_FLUSHES {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some(t) = gs.tavern.toilet.as_ref() else { return Outcome::Done };
        if t.mana_total == 0 || t.mana_currently < t.mana_total || gs.character.inventory.free_slot().is_none() {
            return Outcome::Done;
        }
        let before: Vec<_> = gs.character.inventory.iter().map(|(_, i)| i.map(Item::command_ident)).collect();
        let was = state(gs);
        if let Err(e) = safe::send(session, Command::ToiletFlush).await {
            return fail(&e);
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let got: Vec<String> = gs
            .character
            .inventory
            .iter()
            .zip(before)
            .filter_map(|((_, i), b)| (i.map(Item::command_ident) != b).then_some(i).flatten())
            .map(|i| format!("{:?}{}", i.typ, if i.is_epic() { " (epic)" } else { "" }))
            .collect();
        let line = format!("[toilet] Flushed: {was} -> {}; got {}", state(gs), if got.is_empty() { "nothing visible".into() } else { got.join(", ") });
        report!("{line}");
        crate::inventory::journal(&line);
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_api::gamestate::items::ItemType;

    #[test]
    fn only_epic_equipment_and_gems_may_be_thrown() {
        assert!(throwable(&crate::smith::test_item(ItemType::Belt, 60)));
        assert!(!throwable(&crate::smith::test_item(ItemType::Belt, 5)), "a plain item");
        assert!(!throwable(&crate::smith::test_item(ItemType::Belt, 95)), "a legendary");
    }
}
