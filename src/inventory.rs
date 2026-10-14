//! Inventory management: compares every equippable item in the backpack with what the character wears.
//! Better ones get equipped, worse ones sold. Epic items are never sold (kept for later).
//! "Better" = higher value by the Arena formula (100 % main + 80 % CON + 40 % LCK + 10 % secondary),
//! weapons by expected damage (see `weapon_value`).

use sf_api::{
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        character::Class,
        items::{BagPosition, EquipmentSlot, Gem, GemSlot, GemType, Item, ItemType},
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

/// Unarmed damage range of the game (sf-api `simulate/damage.rs`, `get_hand_damage`): 1–2 up to level 10, then it grows
/// with the level and the class. The fight simulation uses it instead of a weapon whose min AND max are both below it.
pub fn hand_damage(class: Class, level: u16, secondary: bool) -> (f64, f64) {
    if level <= 10 {
        return (1.0, 2.0);
    }
    let multiplier = if class == Class::Assassin { if secondary { 1.25 } else { 0.875 } } else { 0.7 };
    let damage = multiplier * (f64::from(level) - 9.0) * class.weapon_multiplier();
    (1.0f64.max((damage * 2.0 / 3.0).ceil()), 2.0f64.max((damage * 4.0 / 3.0).round()))
}

/// Average damage a weapon really deals: a weapon weaker than bare hands (both min and max below) is ignored by the game.
pub fn effective_avg_damage(class: Class, level: u16, secondary: bool, min_dmg: u32, max_dmg: u32) -> f64 {
    let (hand_min, hand_max) = hand_damage(class, level, secondary);
    if f64::from(min_dmg) < hand_min && f64::from(max_dmg) < hand_max {
        (hand_min + hand_max) / 2.0
    } else {
        f64::from(min_dmg + max_dmg) / 2.0
    }
}

/// Share of the gems we currently mine that an EMPTY gem slot is worth when comparing two items (user 2026-10-12: "a part
/// of it, not all items have a slot").
const POTENTIAL_SHARE: f64 = 0.5;

/// The gem type of the class's main attribute.
pub fn main_gem(class: Class) -> GemType {
    match class.main_attribute() {
        AttributeType::Strength => GemType::Strength,
        AttributeType::Dexterity => GemType::Dexterity,
        _ => GemType::Intelligence,
    }
}

/// What a gem adds for this class: (bonus to the main attribute, value of the other stats with the Arena percentages
/// 80 % CON, 40 % LCK, 10 % each secondary). "All" and Legendary count as every attribute.
pub fn gem_effect(class: Class, g: Gem) -> (f64, f64) {
    let v = f64::from(g.value);
    if g.typ == main_gem(class) {
        return (v, 0.0);
    }
    match g.typ {
        GemType::All | GemType::Legendary => (v, v * (0.8 + 0.4 + 0.2)),
        GemType::Constitution => (0.0, v * 0.8),
        GemType::Luck => (0.0, v * 0.4),
        _ => (0.0, v * 0.1),
    }
}

/// Strength of the best gem the character has seen lately (socketed or in the backpack), 0 without any.
pub fn best_gem_power(gs: &GameState) -> u32 {
    let socketed = gs.character.equipment.0.values().flatten().filter_map(|i| match i.gem_slot {
        Some(GemSlot::Filled(g)) => Some(g.value),
        _ => None,
    });
    let loose = gs.character.inventory.backpack.iter().flatten().filter_map(|i| match i.typ {
        ItemType::Gem(g) => Some(g.value),
        _ => None,
    });
    socketed.chain(loose).max().unwrap_or(0)
}

/// Item value core. A socketed gem counts fully (it cannot be taken out, so swapping the item loses it); an empty gem
/// slot counts `potential` (a gem we expect to put there), if given.
fn value_core(gs: &GameState, item: &Item, slot: EquipmentSlot, potential: Option<Gem>) -> f64 {
    let ch = &gs.character;
    let gem = match item.gem_slot {
        Some(GemSlot::Filled(g)) => Some(g),
        Some(GemSlot::Empty) => potential,
        None => None,
    };
    let (gem_main, gem_other) = gem.map_or((0.0, 0.0), |g| gem_effect(ch.class, g));
    let ItemType::Weapon { min_dmg, max_dmg } = item.typ else {
        return score(ch.class, item) + gem_main + gem_other;
    };
    let main = ch.class.main_attribute();
    let total = f64::from(ch.attribute_basis[main] + ch.attribute_additions[main]);
    let equipped = ch.equipment.0[slot].as_ref().map_or(0.0, |w| f64::from(w.attributes[main]));
    let with_this = total - equipped + f64::from(item.attributes[main]) + gem_main;
    // The weapon's other stats with the same percentages as everywhere else; the main attribute is already in the damage via M
    let others = score(ch.class, item) - f64::from(item.attributes[main]) + gem_other;
    let secondary = ch.class == Class::Assassin && slot == EquipmentSlot::Shield;
    let damage = weapon_value(effective_avg_damage(ch.class, ch.level, secondary, min_dmg, max_dmg), with_this);
    // Reference: the weapon worn in this slot (bare hands without one) with the character's current main attribute
    let ref_avg = match ch.equipment.0[slot].as_ref().map(|w| &w.typ) {
        Some(&ItemType::Weapon { min_dmg, max_dmg }) => effective_avg_damage(ch.class, ch.level, secondary, min_dmg, max_dmg),
        _ => effective_avg_damage(ch.class, ch.level, secondary, 0, 0),
    };
    let damage_ref = weapon_value(ref_avg, total);
    damage_points(damage, damage_ref, total) + others
}

/// Weapon damage on the same scale as attribute points (user 2026-10-14: a weapon with Luck/secondary stats must not win on
/// damage alone). One point of the main attribute raises damage by 1 / (20 + M) of the reference damage, so
/// `ratio × (20 + M)` is the damage expressed in main attribute points.
fn damage_points(damage: f64, damage_ref: f64, main_total: f64) -> f64 {
    if damage_ref <= 0.0 {
        return damage;
    }
    damage / damage_ref * (20.0 + main_total)
}

/// A gem of the class's main attribute worth `power` (the stand-in for "a gem we will get").
fn expected_gem(class: Class, power: f64) -> Gem {
    Gem { typ: main_gem(class), value: power.max(0.0).round() as u32 }
}

/// Item value for our character in a specific equipment slot: non-weapons by attributes; weapons by damage
/// + their other stats (80 % CON, 40 % LCK, 10 % secondary; the main one is already in the damage via M).
/// The slot only matters for weapons: it decides which currently equipped item's main attribute gets
/// subtracted from the total (relevant for Assassins, who have a weapon in both `Weapon` and `Shield`).
/// A socketed gem is counted fully, an empty gem slot at `POTENTIAL_SHARE` of the best gem we have (user 2026-10-12).
fn value_in(gs: &GameState, item: &Item, slot: EquipmentSlot) -> f64 {
    let power = f64::from(best_gem_power(gs)) * POTENTIAL_SHARE;
    value_core(gs, item, slot, (power >= 1.0).then(|| expected_gem(gs.character.class, power)))
}

/// The value the item would have with `gem` in its (empty) slot.
pub fn value_with_gem(gs: &GameState, item: &Item, slot: EquipmentSlot, gem: Gem) -> f64 {
    value_core(gs, item, slot, Some(gem))
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

/// An item that is worse only because its gem slot is still empty (user 2026-10-12): with a gem of the strength we mine now
/// it would beat the equipped one and there is room in the backpack (2 free slots) -> keep it and wait for a gem instead of selling.
fn keep_for_gem(gs: &GameState, item: &Item, slot: EquipmentSlot, cur: f64) -> bool {
    let power = best_gem_power(gs);
    matches!(item.gem_slot, Some(GemSlot::Empty))
        && power > 0
        && gs.character.inventory.count_free_slots() >= 2
        && value_with_gem(gs, item, slot, expected_gem(gs.character.class, f64::from(power))) > cur
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
            if keep_for_gem(gs, item, slot, cur) {
                continue;
            }
            return Some((
                Action::Sell { pos },
                format!("selling {} (value {new:.1} ≤ {cur:.1} of the equipped one)", describe(item)),
            ));
        }
    }
    None
}

/// Uncapped per-character journal of item decisions (`roster/<character>/logs/inventory.log`): the progress log only
/// keeps the last 100 messages, which is not enough to answer "why does this character still have that weapon"
/// (user 2026-10-10, Wecros). `shops.rs` writes the weapons it saw there too.
pub fn journal(line: &str) {
    use std::io::Write;
    let path = crate::ctx::log_path("inventory.log");
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// Everything about an item that matters for judging an equip/sell/buy decision.
pub fn detail(item: &Item) -> String {
    let attrs: Vec<String> = [
        (AttributeType::Strength, "STR"),
        (AttributeType::Dexterity, "DEX"),
        (AttributeType::Intelligence, "INT"),
        (AttributeType::Constitution, "CON"),
        (AttributeType::Luck, "LCK"),
    ]
    .iter()
    .filter(|(a, _)| item.attributes[*a] > 0)
    .map(|(a, n)| format!("{n}+{}", item.attributes[*a]))
    .collect();
    format!(
        "{:?} class={:?} epic={} price={} mushrooms={} [{}]",
        item.typ,
        item.class,
        item.is_epic(),
        item.price,
        item.mushroom_price,
        attrs.join(" ")
    )
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[inventory] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Goes through the inventory and does all equips/sales. Sends nothing when there is nothing to do.
/// Epic items already known in the backpack (per character); `None` until the first look.
static SEEN_EPICS: crate::ctx::PerChar<Option<Vec<String>>> = crate::ctx::PerChar::new();

/// An epic item that dropped (expedition, chest, Gleeman…) is announced once (user 2026-10-12: Gloom event set). The very
/// first look only records what the character already owns, so old epics do not pop up at every start.
fn announce_epics(gs: &GameState) {
    let Ok(mut seen) = SEEN_EPICS.lock() else { return };
    let baseline = seen.is_none();
    let list = seen.get_or_insert_with(Vec::new);
    for item in gs.character.inventory.backpack.iter().flatten().filter(|i| i.is_epic()) {
        let key = format!("{:?}{:?}", item.command_ident(), item.typ);
        if list.contains(&key) {
            continue;
        }
        list.push(key);
        if !baseline {
            crate::shops::alert(&format!("{}: an EPIC item is in the backpack: {}", crate::ctx::name(), detail(item)));
        }
    }
}

pub async fn manage(session: &mut SimpleSession) -> Outcome {
    if let Some(gs) = session.game_state() {
        announce_epics(gs);
    }
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
        journal(&format!("{what} | {}", detail(item)));
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
    /// 2026-10-10 (Wecros): from level 11 on bare hands deal more than a starter weapon, the game then ignores the weapon.
    #[test]
    fn weapon_below_bare_hands_is_ignored() {
        // Berserker level 16: hands 0.7 × 7 × 2 = 9.8 → 7–13
        assert_eq!(hand_damage(Class::Berserker, 16, false), (7.0, 13.0));
        assert_eq!(effective_avg_damage(Class::Berserker, 16, false, 3, 9), 10.0); // hands
        assert_eq!(effective_avg_damage(Class::Berserker, 16, false, 15, 45), 30.0); // a real weapon
        assert_eq!(effective_avg_damage(Class::Berserker, 16, false, 3, 20), 11.5); // only one bound is lower → the weapon
        assert_eq!(hand_damage(Class::Mage, 10, false), (1.0, 2.0));
        // Assassin off-hand hits harder than the main hand
        assert!(hand_damage(Class::Assassin, 17, true).1 > hand_damage(Class::Assassin, 17, false).1);
    }

    #[test]
    fn gem_effect_uses_the_arena_percentages() {
        let g = |typ| Gem { typ, value: 100 };
        assert_eq!(gem_effect(Class::Bard, g(GemType::Intelligence)), (100.0, 0.0));
        assert_eq!(gem_effect(Class::Bard, g(GemType::Constitution)), (0.0, 80.0));
        assert_eq!(gem_effect(Class::Bard, g(GemType::Luck)), (0.0, 40.0));
        assert_eq!(gem_effect(Class::Bard, g(GemType::Strength)), (0.0, 10.0));
        let (main, other) = gem_effect(Class::Bard, g(GemType::All));
        assert_eq!(main, 100.0);
        assert!((other - 140.0).abs() < 1e-9);
    }

    #[test]
    fn damage_points_share_the_scale_of_attributes() {
        // +1 main attribute on the same weapon = +1 point
        let base = weapon_value(30.0, 100.0);
        let plus = weapon_value(30.0, 101.0);
        assert!((damage_points(plus, base, 100.0) - damage_points(base, base, 100.0) - 1.0).abs() < 0.05);
        // +10 % damage at M = 100 is worth 12 points, so +15 LCK (6 points) does not beat it but +40 LCK would
        let better = weapon_value(33.0, 100.0);
        assert!((damage_points(better, base, 100.0) - damage_points(base, base, 100.0) - 12.0).abs() < 1e-9);
    }

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
