//! Cost of buying attribute points with gold (`UpgradeSkill`) and which attribute to pick when a task lets us
//! choose freely (`UpgradeAnyAttribute`/`SpendGoldOnUpgrades`) or when there is simply a lot of spare gold
//! (user 2026-10-09: "ty staty budou pořád stát stejně a my budeme vydělávat pořád víc" – no rush to spend on
//! attributes, but pick the best one by cost when we do).
//!
//! The price of the next point for a given attribute depends ONLY on how many times that attribute has ever
//! been bought with gold (the "attribute increasement level"), never on its current value or the character's
//! level (user 2026-10-09, confirmed against the public cost table below). We have no way to read this lifetime
//! counter from the server, so we track it ourselves, persisted per character (`roster/<nick>/attribute_levels.json`,
//! local only like the rest of `roster/`), and self-correct it from the price actually paid whenever it drifts
//! (e.g. a manual purchase outside the bot, or a wrong starting guess).

use std::collections::HashMap;

use sf_api::{command::AttributeType, gamestate::character::Class};

/// Silver cost of the Nth gold-bought attribute point (1-indexed), levels 1..=216. Source: the public cost
/// table at sf.kalais.net/english/attributes.html (1 gold = 100 silver), user-provided 2026-10-09. Not a smooth
/// formula - irregular step sizes - so a lookup table rather than an approximation. Truncated at 216: the
/// site's own table turns unreliable from there (silver column goes blank, some gold values show as "?") -
/// pending: get/verify numbers for higher levels once a character's attribute actually gets there live.
#[rustfmt::skip]
const COST_TABLE: [u32; 216] = [
    25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 75,
    80, 85, 90, 95, 95, 100, 105, 110, 115, 120, 125, 135,
    140, 150, 155, 160, 170, 175, 185, 190, 200, 205, 215, 225,
    230, 240, 250, 260, 270, 280, 290, 300, 310, 325, 335, 345,
    355, 365, 380, 390, 405, 420, 435, 450, 460, 475, 490, 505,
    520, 535, 550, 565, 580, 600, 615, 630, 650, 665, 685, 705,
    725, 745, 765, 785, 805, 825, 845, 865, 885, 910, 935, 960,
    985, 1010, 1035, 1060, 1085, 1110, 1135, 1160, 1190, 1220, 1250, 1280,
    1310, 1340, 1370, 1400, 1430, 1460, 1495, 1530, 1565, 1600, 1630, 1665,
    1700, 1735, 1770, 1810, 1850, 1890, 1930, 1975, 2015, 2055, 2095, 2135,
    2180, 2225, 2270, 2315, 2360, 2405, 2450, 2500, 2545, 2595, 2645, 2695,
    2750, 2800, 2855, 2910, 2960, 3015, 3065, 3120, 3175, 3235, 3295, 3355,
    3415, 3480, 3540, 3600, 3660, 3720, 3785, 3850, 3920, 3985, 4055, 4125,
    4195, 4265, 4335, 4405, 4480, 4555, 4635, 4710, 4790, 4870, 4945, 5025,
    5100, 5180, 5260, 5345, 5435, 5525, 5615, 5705, 5790, 5880, 5970, 6060,
    6150, 6245, 6345, 6440, 6540, 6640, 6740, 6840, 6940, 7040, 7145, 7255,
    7365, 7475, 7585, 7700, 7810, 7920, 8030, 8140, 8255, 8380, 8505, 8630,
    8755, 8880, 9005, 9130, 9255, 9380, 9505, 9640, 9775, 9910, 10000, 10100,
];

/// Silver cost of the Nth gold-bought point (1-indexed). Beyond the table (character far above what's tested),
/// extrapolated with the table's last step so the bot still has an answer instead of panicking.
fn price_for_level(level: u32) -> u32 {
    let idx = level.saturating_sub(1) as usize;
    if let Some(&p) = COST_TABLE.get(idx) {
        return p;
    }
    let last = COST_TABLE[COST_TABLE.len() - 1];
    let step = COST_TABLE[COST_TABLE.len() - 1] - COST_TABLE[COST_TABLE.len() - 2];
    last + step * (level - COST_TABLE.len() as u32)
}

const ATTRS: [AttributeType; 5] = [
    AttributeType::Strength,
    AttributeType::Dexterity,
    AttributeType::Intelligence,
    AttributeType::Constitution,
    AttributeType::Luck,
];

fn code(a: AttributeType) -> &'static str {
    match a {
        AttributeType::Strength => "STR",
        AttributeType::Dexterity => "DEX",
        AttributeType::Intelligence => "INT",
        AttributeType::Constitution => "CON",
        AttributeType::Luck => "LCK",
    }
}

fn path() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("attribute_levels.json")
}

fn load() -> HashMap<String, u32> {
    std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(levels: &HashMap<String, u32>) {
    if let Some(dir) = path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(levels) {
        let _ = std::fs::write(path(), json);
    }
}

fn level_of(levels: &HashMap<String, u32>, a: AttributeType) -> u32 {
    levels.get(code(a)).copied().unwrap_or(0)
}

/// Silver cost of the next gold-bought point for this attribute.
pub fn next_price(a: AttributeType) -> u32 {
    price_for_level(level_of(&load(), a) + 1)
}

/// After actually buying a point for `paid` silver: advance our counter for `a`, re-syncing it from the paid
/// price if it drifted from what we expected (e.g. a purchase made outside the bot) by searching nearby levels
/// in the table for a match, logged like the other `[check]` verifications in this project.
pub fn record_purchase(a: AttributeType, paid: u32) {
    let mut levels = load();
    let current = level_of(&levels, a);
    let expected = price_for_level(current + 1);
    let next_level = if paid == expected {
        current + 1
    } else {
        let window = 50u32;
        let lo = current.saturating_sub(window).max(1);
        let hi = current + window;
        match (lo..=hi).find(|&l| price_for_level(l) == paid) {
            Some(found) => {
                report!(
                    "[check] MISMATCH attribute {a:?} level: expected to pay {} (level {}), actually paid {} \
                     (resyncing to level {})",
                    crate::report::gold(u64::from(expected)),
                    current + 1,
                    crate::report::gold(u64::from(paid)),
                    found
                );
                found
            }
            None => {
                report!(
                    "[check] MISMATCH attribute {a:?} level: expected to pay {} (level {}), actually paid {} \
                     (no nearby level matches, keeping our counter)",
                    crate::report::gold(u64::from(expected)),
                    current + 1,
                    crate::report::gold(u64::from(paid))
                );
                current + 1
            }
        }
    };
    levels.insert(code(a).to_string(), next_level);
    save(&levels);
}

/// Weight of an attribute towards the user's strength formula (same as `arena::weight`): main 100 %, CON 80 %,
/// LCK 40 %, the other two side attributes 10 % each.
fn weight(class: Class, a: AttributeType) -> f64 {
    crate::arena::weight(class, a)
}

/// Which attribute gives the best weight-per-gold right now, among all 5. Ties favor the main attribute, then
/// the table order above (CON, LCK, side).
pub fn best_attribute(class: Class) -> AttributeType {
    ATTRS
        .into_iter()
        .max_by(|&a, &b| {
            let ratio = |x: AttributeType| weight(class, x) / f64::from(next_price(x));
            ratio(a).partial_cmp(&ratio(b)).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(class.main_attribute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_table_is_nondecreasing() {
        for i in 1..COST_TABLE.len() {
            assert!(COST_TABLE[i] >= COST_TABLE[i - 1]);
        }
    }

    #[test]
    fn extrapolates_beyond_the_table() {
        let last = price_for_level(216);
        let beyond = price_for_level(217);
        assert!(beyond > last);
    }
}
