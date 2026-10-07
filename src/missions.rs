//! Známé mise (úkoly) expedic. Popis a zdroj dat: docs/expedice.md.

use sf_api::gamestate::tavern::ExpeditionThing::{self, *};

/// Body za úkol expedice.
#[derive(Debug, Clone, Copy)]
pub enum Bonus {
    /// Za každý sebraný cílový předmět, připíše se až na konci expedice („+5/“).
    PerItem(i32),
    /// Jednorázově v moment splnění úkolu („+10“).
    OnComplete(i32),
}

#[derive(Debug)]
pub struct Mission {
    pub name: &'static str,
    /// Řetěz kroků (předmět, hrdinství při sebrání). Poslední krok je cílový předmět.
    pub chain: &'static [(ExpeditionThing, i32)],
    pub bonus: Bonus,
    /// Kolik hrdinství se strhne, když úkol na konci není splněný (kladné číslo).
    pub fail_penalty: i32,
}

impl Mission {
    pub fn target(&self) -> ExpeditionThing {
        self.chain.last().map_or(Unknown, |(t, _)| *t)
    }
}

pub const MISSIONS: &[Mission] = &[
    Mission { name: "Dragon Taming", chain: &[(Bait, -2), (Dragon, 10)], bonus: Bonus::PerItem(5), fail_penalty: 0 },
    Mission {
        name: "Extinguished Fire",
        chain: &[(CampFire, 3), (Phoenix, 5), (BurntCampfire, 0)],
        bonus: Bonus::PerItem(4),
        fail_penalty: 0,
    },
    Mission { name: "Hot Carnival Craving", chain: &[(Cake, 5)], bonus: Bonus::PerItem(3), fail_penalty: 0 },
    Mission {
        name: "Unicorn Whisperer",
        chain: &[(UnicornHorn, 1), (Donkey, 2), (Rainbow, 5), (Unicorn, 7)],
        bonus: Bonus::OnComplete(10),
        fail_penalty: 0,
    },
    Mission {
        name: "Podium Climber",
        chain: &[(SmallHurdle, -1), (BigHurdle, -2), (WinnersPodium, 15)],
        bonus: Bonus::PerItem(10),
        fail_penalty: 0,
    },
    Mission { name: "Sanitary Experiment", chain: &[(ToiletPaper, 0)], bonus: Bonus::OnComplete(20), fail_penalty: 5 },
    // Neúplné: známe jen samotný rozbitý meč, předchozí kroky zatím ne
    Mission { name: "Broken Sword", chain: &[(BrokenSword, -4)], bonus: Bonus::PerItem(8), fail_penalty: 0 },
];

/// Mise, jejímž cílem je daný předmět.
pub fn for_target(t: ExpeditionThing) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.target() == t)
}

/// Mise a pozice kroku v jejím řetězu.
pub fn chain_position(t: ExpeditionThing) -> Option<(&'static Mission, usize)> {
    MISSIONS.iter().find_map(|m| m.chain.iter().position(|(c, _)| *c == t).map(|i| (m, i)))
}

/// Věci, které umíme vyhodnotit. Na cokoli jiného se bot zastaví a zeptá.
pub fn is_known(t: ExpeditionThing) -> bool {
    chain_position(t).is_some()
        || t.is_bounty_for().is_some()
        || matches!(t, Key | Suitcase | Dummy1 | Dummy2 | Dummy3)
}
