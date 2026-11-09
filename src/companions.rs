//! Tower companions' equipment (user 2026-11-07/08, `docs/tower.md`). Bert (warrior), Mark (mage) and Kunigunde (scout) have the
//! character's level and NO gear of their own. The main character is always offered an item first; only what the inventory logic
//! would NOT equip for the main character (another class, or worse than what is worn) is offered to the companion of that class
//! when it beats what the companion wears in that slot (same power formula as the main character, weapons by damage). The old item
//! lands in the backpack and goes through the normal disposal. Free (`PlayerItemMove`), nothing here spends mushrooms.
//! Nobody owns the Tower yet, so `gs.dungeons.companions` is None and this does nothing until then.

use sf_api::{
    command::Command,
    gamestate::{
        GameState,
        character::Class,
        dungeons::CompanionClass,
        items::{BagPosition, EquipmentSlot, Item, ItemType},
    },
};

use crate::{inventory, safe, session::SimpleSession, tavern::Outcome};

/// Equips per pass (a safety limit like the inventory's).
const MAX_PER_PASS: usize = 6;

const CLASSES: [CompanionClass; 3] = [CompanionClass::Warrior, CompanionClass::Mage, CompanionClass::Scout];

/// Value of an item for a companion class: weapons by average damage, everything else by the Arena power formula.
fn value(class: Class, item: &Item) -> f64 {
    match item.typ {
        ItemType::Weapon { min_dmg, max_dmg } => f64::from(min_dmg + max_dmg) / 2.0,
        _ => inventory::score(class, item),
    }
}

/// Does the companion gain from `item` in `slot`? An empty slot always gains; a worn item with a gem is left alone
/// (gems on companions are not understood yet).
fn gains(class: Class, item: &Item, worn: Option<&Item>) -> bool {
    match worn {
        None => true,
        Some(w) => w.gem_slot.is_none_or(|g| matches!(g, sf_api::gamestate::items::GemSlot::Empty)) && value(class, item) > value(class, w) * 1.01,
    }
}

/// The next (backpack position, slot, companion) that is worth an equip.
pub fn next_offer(gs: &GameState, done: &[BagPosition]) -> Option<(BagPosition, EquipmentSlot, CompanionClass, String)> {
    let companions = gs.dungeons.companions.as_ref()?;
    let main = gs.character.class;
    for (pos, item) in gs.character.inventory.iter() {
        let Some(item) = item else { continue };
        let Some(slot) = item.typ.equipment_slot() else { continue };
        if item.is_unique() || done.contains(&pos) || inventory::kept_item(gs) == Some(pos) {
            continue;
        }
        // the main character comes first: an item it would wear is not touched here
        if item.can_be_equipped_by(main) {
            let worn = gs.character.equipment.0[slot].as_ref();
            let (new, cur) = inventory::compare(gs, item, worn, slot);
            if new > cur {
                continue;
            }
        }
        let best = CLASSES
            .into_iter()
            .filter(|c| item.can_be_equipped_by_companion(Class::from(*c)))
            .filter(|c| gains(Class::from(*c), item, companions[*c].equipment.0[slot].as_ref()))
            .max_by(|a, b| {
                let gain = |c: &CompanionClass| value(Class::from(*c), item) - companions[*c].equipment.0[slot].as_ref().map_or(0.0, |w| value(Class::from(*c), w));
                gain(a).total_cmp(&gain(b))
            });
        if let Some(c) = best {
            let what = format!("{:?} -> {:?} (companion {c:?})", item.typ, slot);
            return Some((pos, slot, c, what));
        }
    }
    None
}

pub async fn offer(session: &mut SimpleSession) -> Outcome {
    let mut done: Vec<BagPosition> = Vec::new();
    for _ in 0..MAX_PER_PASS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some((pos, slot, companion, what)) = next_offer(gs, &done) else { return Outcome::Done };
        done.push(pos);
        let Some(Some(item)) = gs.character.inventory.backpack.get(pos.backpack_pos()) else { return Outcome::Done };
        let cmd = Command::EquipCompanion { from_pos: pos.into(), to_slot: slot, item_ident: item.command_ident(), to_companion: companion };
        report!("[companions] Equipping {what}");
        inventory::journal(&format!("companion {what} | {}", inventory::detail(item)));
        if let Err(e) = safe::send(session, cmd).await {
            report!("[companions] Equip failed: {e}");
            return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            report!("[companions] Update failed: {e}");
            return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
    }
    Outcome::Done
}

/// Share of the mined gem's value below which the main character's gain counts as minimal (user 2026-11-08: worn +300, mined +301).
pub const MINIMAL_SHARE: f64 = 0.05;

/// A companion's worn item with an EMPTY gem socket that wants this gem (user 2026-11-08): the gem is the companion class's main
/// attribute gem, the socket is empty (a filled one is never replaced). The weapon first, then the slot order. `None` = no place.
/// Wire shape (user capture 2026-11-09, Mark's item): `PlayerItemMove:2/<backpack pos>/102/<slot>/<gem ident>`, same as an equip.
pub fn gem_target(gs: &GameState, gem: sf_api::gamestate::items::Gem) -> Option<(CompanionClass, EquipmentSlot)> {
    let companions = gs.dungeons.companions.as_ref()?;
    let mut places: Vec<(CompanionClass, EquipmentSlot)> = Vec::new();
    for c in CLASSES {
        if gem.typ != inventory::main_gem(Class::from(c)) {
            continue;
        }
        for (slot, item) in &companions[c].equipment.0 {
            if item.as_ref().is_some_and(|i| i.gem_slot == Some(sf_api::gamestate::items::GemSlot::Empty)) {
                places.push((c, slot));
            }
        }
    }
    places.sort_by_key(|(_, slot)| *slot != EquipmentSlot::Weapon);
    places.into_iter().next()
}

#[cfg(test)]
mod gem_tests {
    use super::*;

    #[test]
    fn nothing_without_the_tower() {
        let gs = GameState::default();
        let gem = sf_api::gamestate::items::Gem { typ: sf_api::gamestate::items::GemType::Intelligence, value: 100 };
        assert!(gem_target(&gs, gem).is_none());
    }
}
