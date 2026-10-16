//! Inventory management: compares every equippable item in the backpack with what the character wears.
//! Better ones get equipped, worse ones sold. Epic items are never sold (kept for later).
//! "Better" = higher value by the Arena formula (100 % main + 80 % CON + 27 % LCK + 10 % secondary),
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

use crate::{arena::{LUCK_WEIGHT, power}, safe, tavern::Outcome};

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

/// (2026-10-15, user) An EMPTY gem slot counts only while a loose gem lies in the backpack, ready to be socketed: then it is
/// worth that gem. Without one the slot is worth nothing (before: half of the best gem we had, which let junk-stat items
/// with a slot beat real upgrades and epics).

/// The gem type of the class's main attribute.
pub fn main_gem(class: Class) -> GemType {
    match class.main_attribute() {
        AttributeType::Strength => GemType::Strength,
        AttributeType::Dexterity => GemType::Dexterity,
        _ => GemType::Intelligence,
    }
}

/// What a gem adds for this class: (bonus to the main attribute, value of the other stats with the Arena percentages
/// 80 % CON, 27 % LCK, 10 % each secondary). "All" counts as every attribute, Legendary as main + CON (user 2026-10-15).
pub fn gem_effect(class: Class, g: Gem) -> (f64, f64) {
    let v = f64::from(g.value);
    if g.typ == main_gem(class) {
        return (v, 0.0);
    }
    match g.typ {
        GemType::All => (v, v * (0.8 + LUCK_WEIGHT + 0.2)),
        GemType::Legendary => (v, v * 0.8),
        GemType::Constitution => (0.0, v * 0.8),
        GemType::Luck => (0.0, v * LUCK_WEIGHT),
        _ => (0.0, v * 0.1),
    }
}

/// The gem lying loose in the backpack (ready to be socketed) that is worth the most to this class, with its REAL type
/// (user 2026-10-15: Pjotr, a Necromancer, held a Strength gem +86 that was valued like an Intelligence one).
pub fn best_backpack_gem(gs: &GameState) -> Option<Gem> {
    let class = gs.character.class;
    gs.character
        .inventory
        .backpack
        .iter()
        .flatten()
        .filter_map(|i| match i.typ {
            ItemType::Gem(g) => Some(g),
            _ => None,
        })
        .max_by(|a, b| {
            let worth = |g: &Gem| {
                let (m, o) = gem_effect(class, *g);
                m + o
            };
            worth(a).total_cmp(&worth(b))
        })
}

/// Whether an item deserves a gem (user 2026-10-15): a weapon, or gear with main attribute, CON or LCK of its own. Gems are
/// never taken out again, so a side-stat-only item would waste its own slot's worth for good.
pub fn worth_a_gem(class: Class, item: &Item) -> bool {
    matches!(item.typ, ItemType::Weapon { .. })
        || item.attributes[class.main_attribute()] + item.attributes[AttributeType::Constitution] + item.attributes[AttributeType::Luck] > 0
}

/// Item value core. A socketed gem counts fully (it cannot be taken out, so swapping the item loses it); an empty gem
/// slot counts `potential` (a gem we expect to put there), if given.
fn value_core(gs: &GameState, item: &Item, slot: EquipmentSlot, potential: Option<Gem>) -> f64 {
    let ch = &gs.character;
    // (user 2026-10-15) An empty slot only counts on an item that is worth keeping without the gem: main attribute, CON or
    // at least LCK of its own. Side-stat-only gear never wins on a gem we merely hope to socket. Weapons are exempt (damage).
    let gem = match item.gem_slot {
        Some(GemSlot::Filled(g)) => Some(g),
        Some(GemSlot::Empty) if worth_a_gem(ch.class, item) => potential,
        Some(GemSlot::Empty) | None => None,
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
    // An Assassin hits with both hands every round: (L + R) × 0.625 × (1 + A / 10), so one weapon only changes its share of the
    // total (user 2026-10-14). The other hand counts with its own bare-hands floor.
    let other_hand = if ch.class == Class::Assassin {
        let other = if slot == EquipmentSlot::Weapon { EquipmentSlot::Shield } else { EquipmentSlot::Weapon };
        match ch.equipment.0[other].as_ref().map(|w| &w.typ) {
            Some(&ItemType::Weapon { min_dmg, max_dmg }) => effective_avg_damage(ch.class, ch.level, !secondary, min_dmg, max_dmg),
            _ => effective_avg_damage(ch.class, ch.level, !secondary, 0, 0),
        }
    } else {
        0.0
    };
    let damage = weapon_value(effective_avg_damage(ch.class, ch.level, secondary, min_dmg, max_dmg) + other_hand, total);
    // Reference: the weapon worn in this slot (bare hands without one) with the character's current main attribute
    let ref_avg = match ch.equipment.0[slot].as_ref().map(|w| &w.typ) {
        Some(&ItemType::Weapon { min_dmg, max_dmg }) => effective_avg_damage(ch.class, ch.level, secondary, min_dmg, max_dmg),
        _ => effective_avg_damage(ch.class, ch.level, secondary, 0, 0),
    };
    let damage_ref = weapon_value(ref_avg + other_hand, total);
    // A main attribute bonus of the weapon (or its gem) is worth exactly one point each, it is not weighed like damage
    damage_points(damage, damage_ref, total) + (with_this - total) + others
}

/// Weapon damage on the same scale as attribute points (user 2026-10-14: a weapon with Luck/secondary stats must not win on
/// damage alone). One point of the main attribute raises damage by 1 / (20 + M) of the reference damage, so
/// `ratio × (20 + M)` is the damage expressed in main attribute points.
/// Simulated 2026-10-14 (`weaponsim.rs`): +1 % damage is worth 0.75 of the (20 + M) / 100 main attribute points above
/// (a main point adds 1/(20+M) of the damage against a stronger and 2/(20+M) against a weaker enemy, 1.5 on average).
const DAMAGE_WEIGHT: f64 = 0.75;

fn damage_points(damage: f64, damage_ref: f64, main_total: f64) -> f64 {
    if damage_ref <= 0.0 {
        return damage;
    }
    DAMAGE_WEIGHT * damage / damage_ref * (20.0 + main_total)
}

/// Item value for our character in a specific equipment slot: non-weapons by attributes; weapons by damage
/// + their other stats (80 % CON, 27 % LCK, 10 % secondary; the main one is already in the damage via M).
/// The slot only matters for weapons: it decides which currently equipped item's main attribute gets
/// subtracted from the total (relevant for Assassins, who have a weapon in both `Weapon` and `Shield`).
/// A socketed gem is counted fully, an empty gem slot only as the loose backpack gem it can take right now (user 2026-10-15).
fn value_in(gs: &GameState, item: &Item, slot: EquipmentSlot) -> f64 {
    value_core(gs, item, slot, best_backpack_gem(gs))
}

/// Item value as it really is: a socketed gem counts, an empty slot counts nothing (no hoped-for loose gem). For the
/// dashboard's day-to-day comparison, which must not jump when a gem merely lies in the backpack at snapshot time.
pub fn value_as_worn(gs: &GameState, item: &Item) -> f64 {
    value_core(gs, item, EquipmentSlot::Weapon, None)
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

/// A kept item (or one that takes the place of a worn item) that depends on a gem we do not have yet must beat the worn
/// one by at least this many main-attribute points once the gem is in (user 2026-10-16: not worth the gem hunt, the
/// backpack slot and the fuss for +2).
const KEEP_MIN_GAIN: f64 = 20.0;

/// An item whose value counts on a gem in its still empty slot.
fn gem_dependent(class: Class, item: &Item) -> bool {
    matches!(item.gem_slot, Some(GemSlot::Empty)) && worth_a_gem(class, item)
}

/// The gem we expect to get for an empty slot: the loose one, else a main-attribute gem as strong as the best one this
/// character has ever socketed (the mine produces about that), None when it has never had any.
fn expected_gem(gs: &GameState) -> Option<Gem> {
    if let Some(g) = best_backpack_gem(gs) {
        return Some(g);
    }
    let class = gs.character.class;
    gs.character
        .equipment
        .0
        .iter()
        .filter_map(|(_, i)| match i.as_ref()?.gem_slot? {
            GemSlot::Filled(g) => Some(g.value),
            GemSlot::Empty => None,
        })
        .max()
        .map(|value| Gem { typ: main_gem(class), value })
}

/// The slot an item competes for (an Assassin's weapon goes into whichever hand is weaker).
fn target_slot(gs: &GameState, base_slot: EquipmentSlot) -> EquipmentSlot {
    let class = gs.character.class;
    if class == Class::Assassin && base_slot == EquipmentSlot::Weapon {
        let worth = |s: EquipmentSlot| gs.character.equipment.0[s].as_ref().map_or(f64::MIN, |i| value_in(gs, i, s));
        if worth(EquipmentSlot::Weapon) <= worth(EquipmentSlot::Shield) { EquipmentSlot::Weapon } else { EquipmentSlot::Shield }
    } else {
        base_slot
    }
}

/// The ONE backpack item kept while we wait for a gem (user 2026-10-12, 2026-10-16): an item with an empty gem slot that
/// would beat the worn one by at least `KEEP_MIN_GAIN` once the expected gem sits in it. Patient: it stays even when the
/// backpack gets tight (`swap_kept_item` is the way out), but only one item is ever kept.
fn keep_candidate(gs: &GameState) -> Option<(BagPosition, EquipmentSlot)> {
    let class = gs.character.class;
    let gem = expected_gem(gs)?;
    gs.character
        .inventory
        .iter()
        .filter_map(|(pos, item)| {
            let item = item?;
            let base = item.typ.equipment_slot()?;
            if item.is_unique() || item.is_epic() || !item.can_be_equipped_by(class) || !gem_dependent(class, item) {
                return None;
            }
            let slot = target_slot(gs, base);
            let cur = gs.character.equipment.0[slot].as_ref().map_or(-1.0, |c| value_in(gs, c, slot));
            let gain = value_with_gem(gs, item, slot, gem) - cur;
            (gain >= KEEP_MIN_GAIN).then_some((pos, slot, gain))
        })
        .max_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(pos, slot, _)| (pos, slot))
}

/// Is an item lying in the backpack waiting for a gem (the kept one, see `keep_candidate`)?
pub fn waiting_for_gem(gs: &GameState) -> bool {
    keep_candidate(gs).is_some()
}

/// Backpack completely full and nothing else can make room (user 2026-10-16): put on the item kept for its gem and sell the
/// worn one with its gem. We are weaker until a gem turns up, but a slot is saved. Returns true when it was done.
pub async fn swap_kept_item(session: &mut SimpleSession) -> Result<bool, Outcome> {
    let Some(gs) = session.game_state() else { return Ok(false) };
    if gs.character.inventory.free_slot().is_some() {
        return Ok(false);
    }
    let Some((pos, slot)) = keep_candidate(gs) else { return Ok(false) };
    let Some(Some(item)) = gs.character.inventory.backpack.get(pos.backpack_pos()) else { return Ok(false) };
    let ident = item.command_ident();
    let what = format!("backpack completely full: putting on {} (kept for a gem) and selling the worn {slot:?}", describe(item));
    report!("[inventory] {what}");
    journal(&format!("{what} | {}", detail(item)));
    if let Err(e) = safe::send(session, Command::Equip { from_pos: pos.into(), to_slot: slot, item_ident: ident }).await {
        return err_of(&e);
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        return err_of(&e);
    }
    // The worn item took the place of the one we put on; sell it right away (else the pass would put it back)
    let Some(gs) = session.game_state() else { return Ok(true) };
    let Some(Some(old)) = gs.character.inventory.backpack.get(pos.backpack_pos()) else { return Ok(true) };
    if old.command_ident() == ident {
        return Ok(true); // the move did not happen
    }
    let (old_ident, price) = (old.command_ident(), old.price);
    journal(&format!("selling the swapped-out worn item | {}", detail(old)));
    if let Err(e) = safe::send(session, Command::SellShop { item_pos: pos.into(), item_ident: old_ident }).await {
        return err_of(&e);
    }
    report!("[inventory] Sold for {}", crate::report::gold(u64::from(price)));
    if let Err(e) = safe::send(session, Command::Update).await {
        return err_of(&e);
    }
    Ok(true)
}

/// Last resort for a COMPLETELY full backpack (0 free slots; user 2026-10-16) when no potion can make room: sell a loose
/// gem (the weakest one – it has no socket to go to). With one free slot nothing is sold.
pub fn last_resort_gem(gs: &GameState) -> Option<(BagPosition, String)> {
    if gs.character.inventory.free_slot().is_some() {
        return None;
    }
    let class = gs.character.class;
    let worth = |g: &Gem| {
        let (m, o) = gem_effect(class, *g);
        m + o
    };
    gs.character
        .inventory
        .iter()
        .filter_map(|(pos, i)| match i?.typ {
            ItemType::Gem(g) => Some((pos, g)),
            _ => None,
        })
        .min_by(|a, b| worth(&a.1).total_cmp(&worth(&b.1)))
        .map(|(pos, g)| (pos, format!("backpack completely full: selling the loose {:?} gem", g.typ)))
}

/// The very last resort: the most expensive epic equipment (for the future toilet only the number of epics counts, not
/// their price).
pub fn last_resort_epic(gs: &GameState) -> Option<(BagPosition, String)> {
    if gs.character.inventory.free_slot().is_some() {
        return None;
    }
    gs.character
        .inventory
        .iter()
        .filter_map(|(pos, i)| Some((pos, i?)))
        .filter(|(_, i)| i.is_epic() && !i.is_unique() && i.typ.equipment_slot().is_some())
        .max_by_key(|(_, i)| i.price)
        .map(|(pos, i)| (pos, format!("backpack completely full of epics: selling the most expensive one, {} ({})", describe(i), i.price)))
}

/// Next inventory management step, or None when there is nothing to do.
fn next_action(gs: &GameState) -> Option<(Action, String)> {
    let class = gs.character.class;
    let keep = keep_candidate(gs).map(|k| k.0);
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
        let slot = target_slot(gs, base_slot);
        let new = value_in(gs, item, slot);
        let current = gs.character.equipment.0[slot].as_ref();
        let cur = current.map_or(-1.0, |i| value_in(gs, i, slot));
        // An item that only wins on a gem it does not have yet must win clearly over a worn one that stands on its own
        let margin = if gem_dependent(class, item) && current.is_some_and(|c| !gem_dependent(class, c)) { KEEP_MIN_GAIN } else { 0.0 };
        if new > cur + margin {
            let what = match current {
                Some(c) => format!("equipping {} (value {new:.1} > {cur:.1} of {})", describe(item), describe(c)),
                None => format!("equipping {} into the empty {slot:?} slot (value {new:.1})", describe(item)),
            };
            return Some((Action::Equip { pos, slot }, what));
        }
        if !item.is_epic() {
            if keep == Some(pos) {
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
        "{:?} class={:?} epic={} price={} mushrooms={} [{}] gem slot: {}",
        item.typ,
        item.class,
        item.is_epic(),
        item.price,
        item.mushroom_price,
        attrs.join(" "),
        match item.gem_slot {
            None => "none".to_string(),
            Some(GemSlot::Empty) => "EMPTY (counts as a potential gem)".to_string(),
            Some(GemSlot::Filled(g)) => format!("filled {:?}+{}", g.typ, g.value),
        }
    )
}

/// For the `Result<bool, Outcome>` helpers: a lost session propagates, any other error just means "nothing done".
fn err_of(e: &sf_api::error::SFError) -> Result<bool, Outcome> {
    match fail(e) {
        Outcome::SessionLost => Err(Outcome::SessionLost),
        _ => Ok(false),
    }
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
        assert_eq!(gem_effect(Class::Bard, g(GemType::Luck)), (0.0, 100.0 * LUCK_WEIGHT));
        assert_eq!(gem_effect(Class::Bard, g(GemType::Strength)), (0.0, 10.0));
        let (main, other) = gem_effect(Class::Bard, g(GemType::All));
        assert_eq!(main, 100.0);
        assert!((other - (80.0 + 100.0 * LUCK_WEIGHT + 20.0)).abs() < 1e-9);
    }

    #[test]
    fn damage_points_share_the_scale_of_attributes() {
        // +10 % damage at M = 100 is worth 12 points × DAMAGE_WEIGHT = 9, so +15 LCK (2.25 points) does not beat it
        let base = weapon_value(30.0, 100.0);
        let better = weapon_value(33.0, 100.0);
        assert!((damage_points(better, base, 100.0) - damage_points(base, base, 100.0) - 12.0 * DAMAGE_WEIGHT).abs() < 1e-9);
    }

    #[test]
    fn assassin_weapon_is_only_its_share_of_both_hands() {
        // Both hands 40 + 40: +10 % on one weapon is +5 % of the total damage, half of what a single-weapon class gets
        let (m, ratio_single, ratio_pair): (f64, f64, f64) = (100.0, 1.10, (44.0 + 40.0) / 80.0);
        assert!((ratio_pair - 1.05).abs() < 1e-9);
        let single = damage_points(weapon_value(40.0 * ratio_single, m), weapon_value(40.0, m), m);
        let pair = damage_points(weapon_value(80.0 * ratio_pair, m), weapon_value(80.0, m), m);
        assert!(((pair - 120.0 * DAMAGE_WEIGHT) * 2.0 - (single - 120.0 * DAMAGE_WEIGHT)).abs() < 1e-9);
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
