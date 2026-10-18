//! Cost of buying attribute points with gold (`UpgradeSkill`) and which attribute to pick when a task lets us
//! choose freely (`UpgradeAnyAttribute`/`SpendGoldOnUpgrades`) or when there is simply a lot of spare gold
//! (user 2026-10-09: "ty staty budou pořád stát stejně a my budeme vydělávat pořád víc" – no rush to spend on
//! attributes, but pick the best one by cost when we do).
//!
//! The price of the next point for a given attribute depends ONLY on how many times that attribute has ever
//! been bought with gold (the "attribute increasement level"), never on its current value or the character's
//! level (user 2026-10-09, confirmed against the public cost table below). The server DOES send that lifetime counter
//! (`attribute_times_bought`, found 2026-10-18; the old own counter drifted and was a pure guess beyond the table), so the next
//! level is read from it. The public table ends at level 216; beyond it the real prices paid are remembered per level and
//! the rest estimated from them (`roster/attribute_prices.json`, shared by all characters).

use std::collections::BTreeMap;

use sf_api::{command::AttributeType, gamestate::{GameState, character::Class}};

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

/// Prices actually paid, by the server's own purchase counter (`attribute_times_bought`) + 1 (user 2026-10-18: the old own
/// counter was a guess and drifted). The price depends only on that level, so the file is shared by all characters
/// (`roster/attribute_prices.json`, local only). Levels beyond the public table are estimated from these.
static LEARNED: std::sync::Mutex<Option<BTreeMap<u32, u32>>> = std::sync::Mutex::new(None);

fn prices_path() -> std::path::PathBuf {
    std::path::Path::new("roster").join("attribute_prices.json")
}

fn with_learned<R>(f: impl FnOnce(&mut BTreeMap<u32, u32>) -> R) -> R {
    let mut guard = LEARNED.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(|| {
        std::fs::read_to_string(prices_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    });
    f(map)
}

/// Growth of the price beyond the table (rough: seen exponents 2.0–2.6 between the table's end and level ~450).
const GROWTH: f64 = 2.3;

/// Silver cost of the Nth gold-bought point (1-indexed): the table, a price actually paid, or an estimate (interpolated
/// between paid prices, else a power law from the nearest known one).
fn price_for_level(level: u32) -> u32 {
    let idx = level.saturating_sub(1) as usize;
    if let Some(&p) = COST_TABLE.get(idx) {
        return p;
    }
    with_learned(|m| {
        if let Some(&p) = m.get(&level) {
            return p;
        }
        let lo = m.range(..level).next_back().map(|(&l, &p)| (l, p));
        let hi = m.range(level..).next().map(|(&l, &p)| (l, p));
        let power = |from: (u32, u32)| (f64::from(from.1) * (f64::from(level) / f64::from(from.0)).powf(GROWTH)).round() as u32;
        match (lo, hi) {
            (Some(a), Some(b)) => {
                let t = f64::from(level - a.0) / f64::from(b.0 - a.0);
                (f64::from(a.1) + t * (f64::from(b.1) - f64::from(a.1))).round() as u32
            }
            (Some(a), None) => power(a),
            (None, Some(b)) => power(b),
            (None, None) => power((COST_TABLE.len() as u32, COST_TABLE[COST_TABLE.len() - 1])),
        }
    })
}

const ATTRS: [AttributeType; 5] = [
    AttributeType::Strength,
    AttributeType::Dexterity,
    AttributeType::Intelligence,
    AttributeType::Constitution,
    AttributeType::Luck,
];

/// The level (Nth gold-bought point) the NEXT purchase of this attribute is, from the server's own counter.
pub fn next_level(gs: &GameState, a: AttributeType) -> u32 {
    gs.character.attribute_times_bought[a] + 1
}

/// Silver cost of the next gold-bought point for this attribute.
pub fn next_price(gs: &GameState, a: AttributeType) -> u32 {
    price_for_level(next_level(gs, a))
}

/// After actually buying the point of `level` for `paid`: remember the real price (only beyond the public table, which is
/// trusted below it). Silent: a price that differs from the estimate is normal there.
pub fn record_purchase(level: u32, paid: u32) {
    if level as usize <= COST_TABLE.len() || paid == 0 {
        return;
    }
    with_learned(|m| {
        m.insert(level, paid);
        if let Ok(json) = serde_json::to_string_pretty(m) {
            let _ = std::fs::write(prices_path(), json);
        }
    });
}

/// Luck's weight when BUYING attributes (user 2026-10-16: buying stays at 100 : 80 : 40; the Arena power and item values use
/// `arena::LUCK_WEIGHT` = 0.27, the simulated value).
const BUY_LUCK_WEIGHT: f64 = 0.4;

/// Weight of an attribute for buying: main 100 %, CON 80 %, LCK 40 %, the other two side attributes 2 % each (`arena::SIDE_WEIGHT`).
fn weight(class: Class, a: AttributeType) -> f64 {
    if a == AttributeType::Luck { BUY_LUCK_WEIGHT } else { crate::arena::weight(class, a) }
}

/// Which attribute gives the best weight-per-gold right now, among the main attribute, CON and LCK (user 2026-10-14: the
/// secondary attributes are never bought, not even for their 2 %). Ties favor the main attribute, then CON, LCK.
pub fn best_attribute(gs: &GameState) -> AttributeType {
    let class = gs.character.class;
    ATTRS
        .into_iter()
        .filter(|&a| a == class.main_attribute() || matches!(a, AttributeType::Constitution | AttributeType::Luck))
        .max_by(|&a, &b| {
            let ratio = |x: AttributeType| weight(class, x) / f64::from(next_price(gs, x));
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
