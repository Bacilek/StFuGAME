//! Scrapbook ("Library of Meticulousness", user 2026-10-12): how full it is, and which items an opponent wears that we
//! still lack. A bigger scrapbook gives bonus XP and attributes, so spare Arena fights go to opponents wearing missing items
//! (`hunt::Hunt::Scrapbook`), and the fill level is charted in the dashboard (`roster::snapshot`).
//!
//! sf-api parses the bitmask into `ScrapBook { items: HashSet<EquipmentIdent>, monster }` but keeps its position table
//! private, so the table of valid item positions (801..4225, from sf-api's `parse_scrapbook_item`) is repeated here to know
//! the TOTAL. The percentages are our own (owned / valid positions); the in-game percentages (e.g. 27.37 % on the main) use
//! a denominator we could not reproduce from a captured bitmask (we get 17.6 % items / 11 % monsters for it), so only
//! compare our numbers with each other, day to day.

use std::collections::HashSet;

use sf_api::gamestate::{
    character::Class,
    items::EquipmentSlot,
    unlockables::{EquipmentIdent, ScrapBook},
};

/// Monsters occupy positions 1..=800.
pub const MONSTERS_TOTAL: usize = 800;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Weapon,
    Armor,
    Accessory,
}

/// (positions, slot, class, ignored positions) – `sf_api::gamestate::unlockables::parse_scrapbook_item`.
fn table() -> Vec<(std::ops::Range<i64>, EquipmentSlot, Option<Class>, &'static [i64])> {
    use Class::*;
    use EquipmentSlot::*;
    vec![
        (801..1011, Amulet, None, &[]),
        (1011..1051, Amulet, None, &[]),
        (1051..1211, Ring, None, &[]),
        (1211..1251, Ring, None, &[]),
        (1251..1325, Talisman, None, &[]),
        (1325..1365, Talisman, None, &[]),
        (1365..1665, Weapon, Some(Warrior), &[]),
        (1665..1705, Weapon, Some(Warrior), &[]),
        (1705..1805, Shield, Some(Warrior), &[]),
        (1805..1845, Shield, Some(Warrior), &[]),
        (1845..1945, BreastPlate, Some(Warrior), &[]),
        (1945..1985, BreastPlate, Some(Warrior), &[1954, 1955]),
        (1985..2085, FootWear, Some(Warrior), &[]),
        (2085..2125, FootWear, Some(Warrior), &[2094, 2095]),
        (2125..2225, Gloves, Some(Warrior), &[]),
        (2225..2265, Gloves, Some(Warrior), &[2234, 2235]),
        (2265..2365, Hat, Some(Warrior), &[]),
        (2365..2405, Hat, Some(Warrior), &[2374, 2375]),
        (2405..2505, Belt, Some(Warrior), &[]),
        (2505..2545, Belt, Some(Warrior), &[2514, 2515]),
        (2545..2645, Weapon, Some(Mage), &[]),
        (2645..2685, Weapon, Some(Mage), &[]),
        (2685..2785, BreastPlate, Some(Mage), &[]),
        (2785..2825, BreastPlate, Some(Mage), &[2794, 2795]),
        (2825..2925, FootWear, Some(Mage), &[]),
        (2925..2965, FootWear, Some(Mage), &[2934, 2935]),
        (2965..3065, Gloves, Some(Mage), &[]),
        (3065..3105, Gloves, Some(Mage), &[3074, 3075]),
        (3105..3205, Hat, Some(Mage), &[]),
        (3205..3245, Hat, Some(Mage), &[3214, 3215]),
        (3245..3345, Belt, Some(Mage), &[]),
        (3345..3385, Belt, Some(Mage), &[3354, 3355]),
        (3385..3485, Weapon, Some(Scout), &[]),
        (3485..3525, Weapon, Some(Scout), &[]),
        (3525..3625, BreastPlate, Some(Scout), &[]),
        (3625..3665, BreastPlate, Some(Scout), &[3634, 3635]),
        (3665..3765, FootWear, Some(Scout), &[]),
        (3765..3805, FootWear, Some(Scout), &[3774, 3775]),
        (3805..3905, Gloves, Some(Scout), &[]),
        (3905..3945, Gloves, Some(Scout), &[3914, 3915]),
        (3945..4045, Hat, Some(Scout), &[]),
        (4045..4085, Hat, Some(Scout), &[4054, 4055]),
        (4085..4185, Belt, Some(Scout), &[]),
        (4185..4225, Belt, Some(Scout), &[4194, 4195]),
    ]
}

fn kind_of(slot: EquipmentSlot) -> Kind {
    match slot {
        EquipmentSlot::Weapon | EquipmentSlot::Shield => Kind::Weapon,
        EquipmentSlot::Amulet | EquipmentSlot::Ring | EquipmentSlot::Talisman => Kind::Accessory,
        _ => Kind::Armor,
    }
}

/// Every valid scrapbook item with its kind (the same ident formula as sf-api).
pub fn all_items() -> Vec<(EquipmentIdent, Kind)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (pos, (range, typ, class, ignore)) in table().into_iter().enumerate() {
        let is_epic = pos % 2 == 1;
        for idx in range.clone() {
            if ignore.contains(&idx) {
                continue;
            }
            let rel = idx - range.start + 1;
            let color = match rel % 10 {
                _ if typ == EquipmentSlot::Talisman || is_epic => 1,
                0 => 5,
                1..=5 => rel % 10,
                _ => rel % 10 - 5,
            } as u8;
            let model_id = match () {
                () if is_epic => rel + 49,
                () if typ == EquipmentSlot::Talisman => rel,
                () if rel % 5 != 0 => rel / 5 + 1,
                () => rel / 5,
            } as u16;
            let ident = EquipmentIdent { class, typ, model_id, color };
            // sf-api's own table maps some talisman positions (models 50-74) twice; they are one item in the set
            if seen.insert(ident) {
                out.push((ident, kind_of(typ)));
            }
        }
    }
    out
}

/// How full the scrapbook is.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    pub owned: usize,
    pub total: usize,
    pub weapons: (usize, usize),
    pub armor: (usize, usize),
    pub accessories: (usize, usize),
    pub monsters: usize,
}

/// Denominator of the game's own percentage (Library of Meticulousness, shown on the profile as the XP bonus), fitted
/// 2026-10-12 from the profiles the user read off five characters (Sanek 16 %, Chlamydie 14 %, Filminy 12 %, Pjotr 12 %, Mimi 12 %
/// with 416 / 370 / 321 / 316 / 309 owned items + monsters): every one of them is consistent with any value between 2568 and 2684,
/// so 2600 for now. Exact (2-decimal) profile values would pin it down: D = (items + monsters) / percentage.
pub const GAME_TOTAL: f64 = 2600.0;

impl Stats {
    /// The game's percentage: (owned items + seen monsters) / `GAME_TOTAL`.
    pub fn game_pct(self) -> f64 {
        (self.owned + self.monsters) as f64 * 100.0 / GAME_TOTAL
    }

    pub fn pct(self) -> f64 {
        if self.total == 0 { 0.0 } else { self.owned as f64 * 100.0 / self.total as f64 }
    }
}

fn part((o, t): (usize, usize)) -> f64 {
    if t == 0 { 0.0 } else { o as f64 * 100.0 / t as f64 }
}

pub fn stats(sb: &ScrapBook) -> Stats {
    let mut s = Stats { monsters: sb.monster.len(), ..Stats::default() };
    for (ident, kind) in all_items() {
        let owned = sb.items.contains(&ident);
        let (o, t) = match kind {
            Kind::Weapon => &mut s.weapons,
            Kind::Armor => &mut s.armor,
            Kind::Accessory => &mut s.accessories,
        };
        *t += 1;
        s.total += 1;
        if owned {
            *o += 1;
            s.owned += 1;
        }
    }
    s
}

/// JSON for the day snapshot: percentages and counts.
pub fn json(sb: &ScrapBook) -> serde_json::Value {
    let s = stats(sb);
    serde_json::json!({
        "pct": s.game_pct(), "items_pct": s.pct(), "owned": s.owned, "total": s.total,
        "weapons_pct": part(s.weapons), "armor_pct": part(s.armor), "accessories_pct": part(s.accessories),
        "monsters": s.monsters, "monsters_pct": s.monsters as f64 * 100.0 / MONSTERS_TOTAL as f64,
    })
}

/// How many of the items this player wears are missing from our scrapbook.
pub fn missing_on(equipment: &sf_api::gamestate::items::Equipment, sb: &ScrapBook) -> usize {
    let valid: HashSet<EquipmentIdent> = all_items().into_iter().map(|(i, _)| i).collect();
    equipment
        .0
        .values()
        .flatten()
        .filter_map(|i| i.equipment_ident())
        .filter(|id| valid.contains(id) && !sb.items.contains(id))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_the_expected_number_of_items() {
        let all = all_items();
        // 564 accessories + 3 classes (760 + ... ) – the numbers only need to be stable and unique
        let unique: HashSet<_> = all.iter().map(|(i, _)| *i).collect();
        assert_eq!(unique.len(), all.len(), "idents must be unique");
        assert_eq!(all.len(), 3358);
        assert_eq!(all.iter().filter(|(_, k)| *k == Kind::Accessory).count(), 539); // 564 positions, 25 talisman duplicates
        assert_eq!(all.iter().filter(|(_, k)| *k == Kind::Weapon).count(), 749); // 760 positions, 11 duplicate idents in sf-api's table
    }

    #[test]
    fn game_percentage_matches_the_profiles_the_user_read_off() {
        // (items + monsters, percentage shown in the game, rounded to whole percent)
        for (n, shown) in [(416.0, 16.0), (370.0, 14.0), (321.0, 12.0), (316.0, 12.0), (309.0, 12.0)] {
            let pct = n * 100.0 / GAME_TOTAL;
            assert!((pct - shown).abs() < 0.5, "{n} -> {pct}");
        }
    }

    #[test]
    fn empty_scrapbook_is_zero_percent() {
        let sb = ScrapBook { items: HashSet::new(), monster: HashSet::new() };
        assert_eq!(stats(&sb).pct(), 0.0);
        assert_eq!(stats(&sb).total, 3358);
    }
}
