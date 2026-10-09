//! Inventory management: compares every equippable item in the backpack with what the character wears.
//! Better ones get equipped, worse ones sold. Epic items are never sold (kept for later).
//! "Better" = higher value by the Arena formula (100 % main + 80 % CON + 40 % LCK + 10 % secondary),
//! weapons by expected damage (see `weapon_value`).

use sf_api::{
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        character::Class,
        items::{BagPosition, EquipmentSlot, Item, ItemType},
    },
};

use crate::session::SimpleSession;

use crate::{arena::power, safe, tavern::Outcome};

/// Safety limit against an endless loop in one pass.
const MAX_ACTIONS: usize = 30;

/// Item value for our class.
pub fn score(class: Class, item: &Item) -> f64 {
    power(class, |a: AttributeType| f64::from(item.attributes[a]))
}

/// Weapon value = average damage × (1 + M / 20).
/// From the sf-api fight simulation (simulate/damage.rs): hit damage = weapon × (1 + A / 10),
/// where A = max(M / 2, M − M_enemy / 2); against an equally strong enemy A = M / 2.
/// M = the character's total main attribute with exactly this weapon equipped.
pub fn weapon_value(avg_damage: f64, main_attr_with_weapon: f64) -> f64 {
    avg_damage * (1.0 + main_attr_with_weapon / 20.0)
}

/// Item value for our character in a specific equipment slot: non-weapons by attributes; weapons by damage
/// + their other stats (80 % CON, 40 % LCK, 10 % secondary; the main one is already in the damage).
/// The slot only matters for weapons: it decides which currently equipped item's main attribute gets
/// subtracted from the total (relevant for Assassins, who have a weapon in both `Weapon` and `Shield`).
fn value_in(gs: &GameState, item: &Item, slot: EquipmentSlot) -> f64 {
    let ch = &gs.character;
    let ItemType::Weapon { min_dmg, max_dmg } = item.typ else {
        return score(ch.class, item);
    };
    let main = ch.class.main_attribute();
    let total = f64::from(ch.attribute_basis[main] + ch.attribute_additions[main]);
    let equipped = ch.equipment.0[slot].as_ref().map_or(0.0, |w| f64::from(w.attributes[main]));
    let with_this = total - equipped + f64::from(item.attributes[main]);
    // The weapon's other stats with the same percentages as everywhere else; the main attribute is already in the damage via M
    let others = score(ch.class, item) - f64::from(item.attributes[main]);
    weapon_value(f64::from(min_dmg + max_dmg) / 2.0, with_this) + others
}

/// Item value for our character in the main `Weapon` slot (everyone except an Assassin's off-hand).
pub fn value(gs: &GameState, item: &Item) -> f64 {
    value_in(gs, item, EquipmentSlot::Weapon)
}

#[derive(Debug)]
enum Action {
    Equip { pos: BagPosition, slot: EquipmentSlot },
    Sell { pos: BagPosition },
}

fn describe(item: &Item) -> String {
    format!("{:?}{}", item.typ, if item.is_epic() { " (epic)" } else { "" })
}

/// Next inventory management step, or None when there is nothing to do.
fn next_action(gs: &GameState) -> Option<(Action, String)> {
    let class = gs.character.class;
    for (pos, item) in gs.character.inventory.iter() {
        let Some(item) = item else { continue };
        let Some(base_slot) = item.typ.equipment_slot() else { continue };
        if item.is_unique() {
            continue;
        }
        if !item.can_be_equipped_by(class) {
            if !item.is_epic() {
                return Some((Action::Sell { pos }, format!("selling {} (for another class)", describe(item))));
            }
            continue;
        }
        // Assassins dual-wield (a weapon in both Weapon and Shield): a weapon drop can go into either hand,
        // so it competes against whichever of the two is currently weaker (or empty).
        let slot = if class == Class::Assassin && base_slot == EquipmentSlot::Weapon {
            let worth = |s: EquipmentSlot| gs.character.equipment.0[s].as_ref().map_or(f64::MIN, |i| value_in(gs, i, s));
            if worth(EquipmentSlot::Weapon) <= worth(EquipmentSlot::Shield) { EquipmentSlot::Weapon } else { EquipmentSlot::Shield }
        } else {
            base_slot
        };
        let new = value_in(gs, item, slot);
        let current = gs.character.equipment.0[slot].as_ref();
        let cur = current.map_or(-1.0, |i| value_in(gs, i, slot));
        if new > cur {
            let what = match current {
                Some(c) => format!("equipping {} (value {new:.1} > {cur:.1} of {})", describe(item), describe(c)),
                None => format!("equipping {} into the empty {slot:?} slot (value {new:.1})", describe(item)),
            };
            return Some((Action::Equip { pos, slot }, what));
        }
        if !item.is_epic() {
            return Some((
                Action::Sell { pos },
                format!("selling {} (value {new:.1} ≤ {cur:.1} of the equipped one)", describe(item)),
            ));
        }
    }
    None
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[inventory] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Goes through the inventory and does all equips/sales. Sends nothing when there is nothing to do.
pub async fn manage(session: &mut SimpleSession) -> Outcome {
    for _ in 0..MAX_ACTIONS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some((action, what)) = next_action(gs) else { return Outcome::Done };
        let pos_idx = match &action {
            Action::Equip { pos, .. } | Action::Sell { pos } => pos.backpack_pos(),
        };
        let Some(Some(item)) = gs.character.inventory.backpack.get(pos_idx) else { return Outcome::Done };
        let item_ident = item.command_ident();
        let price = item.price;

        report!("[inventory] {what}");
        let cmd = match action {
            Action::Equip { pos, slot } => Command::Equip { from_pos: pos.into(), to_slot: slot, item_ident },
            Action::Sell { pos } => Command::SellShop { item_pos: pos.into(), item_ident },
        };
        let selling = matches!(cmd, Command::SellShop { .. });
        if let Err(e) = safe::send(session, cmd).await {
            return fail(&e);
        }
        if selling {
            report!("[inventory] Sold for {}", crate::report::gold(u64::from(price)));
        }
        // Always decide on a fresh state
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
    }
    report!("[inventory] Action limit for one pass reached");
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-07: weapon 13–15 vs 9–15. Higher damage wins unless the new one adds a lot of main attribute.
    #[test]
    fn weapon_damage_matters() {
        // same main attribute of the character: 14 × 3.5 > 12 × 3.5
        assert!(weapon_value(14.0, 50.0) > weapon_value(12.0, 50.0));
        // the new one adds +6 main attribute: 12 × (1 + 56/20) = 45.6 < 14 × (1 + 50/20) = 49
        assert!(weapon_value(14.0, 50.0) > weapon_value(12.0, 56.0));
        // with a small character attribute the attribute matters more: 12 × (1 + 16/20) = 21.6 > 14 × (1 + 10/20) = 21
        assert!(weapon_value(12.0, 16.0) > weapon_value(14.0, 10.0));
    }
}
