//! Blacksmith upgrade of the worn equipment to level 2 (user 2026-11-08, `docs/blacksmith.md`): only the main character's own worn items
//! (never a companion's), right after they are put on. Cheap (2/20); when the Metal / Arcane Splinters are missing the bell rings.
//! Wire format (user capture 2026-11-08): `PlayerItemUpgrade:1/<slot position>/2` = place 1 (equipment), position = `EquipmentSlot as usize`
//! (Weapon 9), 2 = the wanted upgrade level. The answer carries `ownplayersaveequipment`. Costs metal and splinters only, no mushrooms.

use chrono::{Local, NaiveDate};
use sf_api::{
    command::Command,
    gamestate::{GameState, items::EquipmentSlot},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// The level the bot upgrades worn items to.
pub const TARGET: u8 = 2;

/// Day on which an upgrade did not do what was expected (nothing more is tried that day).
static HALTED: crate::ctx::PerChar<Option<NaiveDate>> = crate::ctx::PerChar::new();

fn halted_today() -> bool {
    HALTED.lock().is_ok_and(|d| *d == Some(Local::now().date_naive()))
}

fn halt() {
    if let Ok(mut d) = HALTED.lock() {
        *d = Some(Local::now().date_naive());
    }
}

/// Wire text of the command for a worn slot.
pub fn args(slot: EquipmentSlot) -> Vec<String> {
    vec!["1".to_string(), (slot as usize).to_string(), TARGET.to_string()]
}

/// Metal and splinters to get a worn item from its current level to `TARGET` (None = nothing to do / not upgradable).
fn cost(item: &sf_api::gamestate::items::Item) -> Option<(u64, u64)> {
    if item.upgrade_count >= TARGET {
        return None;
    }
    let mut it = item.clone();
    let (mut metal, mut arcane) = (0, 0);
    while it.upgrade_count < TARGET {
        let c = it.upgrade_costs()?;
        metal += u64::from(c.metal);
        arcane += u64::from(c.arcane);
        it.upgrade_count += 1;
    }
    Some((metal, arcane))
}

/// The worn slot the command may upgrade right now (the safe.rs check): the main character wears an upgradable item there that is below `TARGET`.
pub fn allowed(gs: &GameState, slot_pos: &str) -> bool {
    let Ok(n) = slot_pos.parse::<usize>() else { return false };
    gs.blacksmith.is_some()
        && gs
            .character
            .equipment
            .0
            .iter()
            .any(|(slot, item)| slot as usize == n && item.as_ref().is_some_and(|i| cost(i).is_some()))
}

pub async fn run(session: &mut SimpleSession) -> Outcome {
    for _ in 0..10 {
        if halted_today() {
            return Outcome::Done;
        }
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some(smith) = gs.blacksmith.as_ref() else { return Outcome::Done };
        let (metal, arcane) = (smith.metal, smith.arcane);
        let Some((slot, item, (need_m, need_a))) =
            gs.character.equipment.0.iter().find_map(|(slot, item)| item.as_ref().and_then(|i| cost(i).map(|c| (slot, i.clone(), c))))
        else {
            return Outcome::Done;
        };
        if metal < need_m || arcane < need_a {
            crate::alerts::raise_limited(
                "smith",
                &format!("upgrade-{slot:?}"),
                &format!("Chybí materiál na upgrade {slot:?} na level {TARGET}."),
                &format!("potřeba {need_m} Metal / {need_a} Arcane Splinters, je {metal} / {arcane}"),
                720,
            );
            return Outcome::Done;
        }
        let was = item.upgrade_count;
        report!("[smith] Upgrading the worn {slot:?} {was} -> {TARGET} ({need_m} Metal / {need_a} splinters)");
        let raw = safe::send_raw(session, Command::Custom { cmd_name: "PlayerItemUpgrade".to_string(), arguments: args(slot) }).await;
        if let Err(e) = raw {
            if crate::tavern::is_session_error(&e) {
                return Outcome::SessionLost;
            }
            report!("[smith] Upgrade of the {slot:?} failed: {e}");
            halt();
            return Outcome::Done;
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            if crate::tavern::is_session_error(&e) {
                return Outcome::SessionLost;
            }
            return Outcome::Done;
        }
        let now = session.game_state().and_then(|gs| gs.character.equipment.0[slot].as_ref().map(|i| i.upgrade_count));
        if now == Some(TARGET) {
            report!("[smith] OK: the {slot:?} is now at upgrade level {TARGET}");
        } else {
            report!("[smith] The {slot:?} shows upgrade level {now:?} after the upgrade (expected {TARGET}), no more today");
            crate::alerts::raise_limited(
                "smith",
                "upgrade-mismatch",
                "Upgrade vybavení nedopadl podle očekávání.",
                &format!("{slot:?}: {was} -> {now:?}, čekáno {TARGET}"),
                720,
            );
            halt();
            return Outcome::Done;
        }
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_text_matches_the_capture() {
        // user capture 2026-11-08: PlayerItemUpgrade:1/9/2 for the weapon
        assert_eq!(args(EquipmentSlot::Weapon), vec!["1", "9", "2"]);
    }

    #[test]
    fn nothing_to_do_at_the_target_level() {
        let mut item = crate::smith::test_item(sf_api::gamestate::items::ItemType::Hat, 5);
        item.upgrade_count = TARGET;
        assert!(cost(&item).is_none());
    }
}
