//! Blacksmith (unlocks at character level 90), phase 1 (user 2026-10-24): dismantling only, Metal and Arcane Splinters are just
//! collected (no upgrades yet). Where an item used to be sold, it is dismantled when that pays Arcane Splinters:
//! - a plain item the witch wants today -> cauldron (unchanged, she goes first),
//! - a plain item whose dismantling pays Arcane Splinters (sf-api `Item::dismantle_reward`) -> Blacksmith instead of the shop,
//! - a plain item that would give only Metal -> cauldron or shop as before,
//! - the last-resort epic sale (backpack full, `inventory::last_resort_epic`) -> the epic with the MOST splinters is dismantled
//!   instead of the most expensive one sold.
//! Dismantling is limited per day (5, 15 during the Forge Frenzy Festival); the server's `dismantle_left` decides, a refusal
//! stops it for the rest of the day. See `docs/blacksmith.md`.

use chrono::{Local, NaiveDate};
use sf_api::{
    command::{BlacksmithAction, Command},
    error::SFError,
    gamestate::{GameState, items::{BagPosition, Item}},
};

use crate::{safe, session::SimpleSession};

/// The Blacksmith opens at this character level.
pub const MIN_LEVEL: u16 = 90;

/// The day on which the server refused a dismantle (no more left today); per character.
static REFUSED_ON: crate::ctx::PerChar<Option<NaiveDate>> = crate::ctx::PerChar::new();

/// What happens to a backpack item that is to be got rid of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposal {
    Shop,
    Cauldron,
    Dismantle,
}

/// Can we dismantle one more item now? Level 90+, not refused today and the server says dismantles are left. The server's counter is
/// only known once a response carried `smith:`, so an unknown counter (never dismantled) or one from an earlier day counts as "try".
pub fn available(gs: &GameState) -> bool {
    if gs.character.level < MIN_LEVEL || REFUSED_ON.lock().is_ok_and(|r| *r == Some(Local::now().date_naive())) {
        return false;
    }
    gs.blacksmith.as_ref().is_none_or(|b| {
        b.dismantle_left > 0 || b.last_dismantled.is_none_or(|t| t.date_naive() < Local::now().date_naive())
    })
}

/// Arcane Splinters a dismantle should pay (the sf-api port of the community smith simulator; verified against the game in the log).
pub fn splinters(item: &Item) -> u64 {
    item.dismantle_reward().arcane
}

/// Where a PLAIN item goes instead of the shop sale: the witch first, then the Blacksmith when it pays Arcane Splinters.
pub fn disposal(gs: &GameState, item: &Item) -> Disposal {
    if crate::witch::wants(gs, item) {
        Disposal::Cauldron
    } else if !item.is_epic() && !item.is_unique() && available(gs) && splinters(item) > 0 {
        Disposal::Dismantle
    } else {
        Disposal::Shop
    }
}

/// Where the EPIC of the forced free-slot sale goes (user 2026-10-24): dismantled whenever possible, else as before the cauldron
/// (twice the price for her type) or the shop.
pub fn epic_disposal(gs: &GameState, item: &Item) -> Disposal {
    if available(gs) {
        Disposal::Dismantle
    } else if crate::witch::type_matches(gs, item) {
        Disposal::Cauldron
    } else {
        Disposal::Shop
    }
}

/// What a dismantle was expected to pay and the stock before it, to compare with the answer.
pub struct Before {
    predicted_metal: u64,
    predicted_arcane: u64,
    metal: u64,
    arcane: u64,
    what: String,
}

/// The dismantle command for a backpack item, plus the numbers to check it against (`log_result`).
pub fn command(gs: &GameState, pos: BagPosition, item: &Item, what: &str) -> (Command, Before) {
    let reward = item.dismantle_reward();
    let (metal, arcane) = gs.blacksmith.as_ref().map_or((0, 0), |b| (b.metal, b.arcane));
    let cmd = Command::Blacksmith { item_pos: pos.into(), action: BlacksmithAction::Dismantle, item_ident: item.command_ident() };
    (cmd, Before { predicted_metal: reward.metal, predicted_arcane: reward.arcane, metal, arcane, what: what.to_string() })
}

/// After the `Update` that follows a dismantle: log what was really paid next to the prediction (`[smith] MISMATCH` when the
/// splinters differ, then the formula needs a look) and how many dismantles are left.
pub fn log_result(gs: &GameState, before: &Before) {
    let (metal, arcane, left) = gs.blacksmith.as_ref().map_or((0, 0, 0), |b| (b.metal, b.arcane, b.dismantle_left));
    let (dm, da) = (metal.saturating_sub(before.metal), arcane.saturating_sub(before.arcane));
    let verdict = if da == before.predicted_arcane && dm == before.predicted_metal { "as predicted" } else { "MISMATCH" };
    let line = format!(
        "[smith] Dismantled {}: Metal +{dm} (predicted {}), Arcane Splinters +{da} (predicted {}) - {verdict}; stock {metal} Metal / {arcane} splinters, {left} dismantles left",
        before.what, before.predicted_metal, before.predicted_arcane
    );
    report!("{line}");
    crate::inventory::journal(&line);
}

/// A failed dismantle: out of dismantles (or anything else the server dislikes) stops dismantling for today and the item is sold
/// instead. A lost session is the caller's business (returns true).
pub fn refused(e: &SFError) -> bool {
    if crate::tavern::is_session_error(e) {
        return true;
    }
    if let Ok(mut r) = REFUSED_ON.lock() {
        *r = Some(Local::now().date_naive());
    }
    report!("[smith] The Blacksmith refused ({e}), no more dismantling today");
    false
}

/// Dismantles the backpack item at `pos` (Update afterwards, result logged). `Ok(false)` = refused, caller sells instead.
pub async fn dismantle(session: &mut SimpleSession, pos: BagPosition, what: &str) -> Result<bool, SFError> {
    let Some(gs) = session.game_state() else { return Ok(false) };
    let Some(Some(item)) = gs.character.inventory.backpack.get(pos.backpack_pos()) else { return Ok(false) };
    let (cmd, before) = command(gs, pos, item, what);
    if let Err(e) = safe::send(session, cmd).await {
        return if refused(&e) { Err(e) } else { Ok(false) };
    }
    safe::send(session, Command::Update).await?;
    if let Some(gs) = session.game_state() {
        log_result(gs, &before);
    }
    Ok(true)
}

/// A bare item for tests (model id 50+ = epic).
#[cfg(test)]
pub fn test_item(typ: sf_api::gamestate::items::ItemType, model_id: u16) -> Item {
    Item {
        typ,
        price: 100,
        mushroom_price: 0,
        full_model_id: u32::from(model_id),
        model_id,
        class: None,
        type_specific_val: 0,
        attributes: enum_map::enum_map! { _ => 40 },
        gem_slot: None,
        rune: None,
        enchantment: None,
        color: 0,
        upgrade_count: 0,
        item_quality: 0,
        is_washed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_api::gamestate::{items::ItemType, unlockables::Blacksmith};

    fn gs_at(level: u16) -> GameState {
        let mut gs = GameState::default();
        gs.character.level = level;
        gs
    }

    #[test]
    fn locked_below_level_90() {
        assert!(!available(&gs_at(89)));
        assert!(available(&gs_at(90)), "counter never seen: try");
    }

    #[test]
    fn server_counter_decides() {
        let mut gs = gs_at(100);
        gs.blacksmith = Some(Blacksmith { dismantle_left: 0, last_dismantled: Some(Local::now()), ..Default::default() });
        assert!(!available(&gs), "none left today");
        gs.blacksmith.as_mut().unwrap().dismantle_left = 5;
        assert!(available(&gs));
        gs.blacksmith = Some(Blacksmith { dismantle_left: 0, last_dismantled: Some(Local::now() - chrono::Duration::days(2)), ..Default::default() });
        assert!(available(&gs), "the counter is from an earlier day");
    }

    #[test]
    fn epics_are_never_taken_by_the_plain_rule() {
        let gs = gs_at(100);
        let epic = test_item(ItemType::Belt, 60);
        assert!(epic.is_epic());
        assert_ne!(disposal(&gs, &epic), Disposal::Dismantle);
        assert_eq!(epic_disposal(&gs, &epic), Disposal::Dismantle);
    }
}
