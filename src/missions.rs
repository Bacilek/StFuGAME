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
    /// Kolik cílových předmětů je potřeba ke splnění (u řetězů 1).
    pub count: u8,
}

impl Mission {
    pub fn target(&self) -> ExpeditionThing {
        self.chain.last().map_or(Unknown, |(t, _)| *t)
    }

    /// Jak snadno mise dává hrdinství: průměr na jedno kolo, když ji splníme
    /// (hrdinství kroků + bonus + odvrácený trest) / počet potřebných kol. Víc = snazší 40.
    pub fn ease(&self) -> f64 {
        let steps: i32 = self.chain.iter().map(|(_, h)| h).sum();
        let per_round = steps + match self.bonus {
            Bonus::PerItem(b) => b,
            Bonus::OnComplete(_) => 0,
        };
        let once = match self.bonus {
            Bonus::PerItem(_) => 0,
            Bonus::OnComplete(b) => b + self.fail_penalty,
        };
        let count = i32::from(self.count);
        let rounds = self.chain.len() as f64 * f64::from(count);
        f64::from(per_round * count + once) / rounds
    }
}

pub const MISSIONS: &[Mission] = &[
    Mission { name: "Dragon Taming", chain: &[(Bait, -2), (Dragon, 10)], bonus: Bonus::PerItem(5), fail_penalty: 0, count: 1 },
    Mission {
        name: "Extinguished Fire",
        chain: &[(CampFire, 3), (Phoenix, 5), (BurntCampfire, 0)],
        bonus: Bonus::PerItem(4),
        fail_penalty: 0,
        count: 1,
    },
    Mission { name: "Hot Carnival Craving", chain: &[(Cake, 5)], bonus: Bonus::PerItem(3), fail_penalty: 0, count: 1 },
    Mission {
        name: "Unicorn Whisperer",
        chain: &[(UnicornHorn, 1), (Donkey, 2), (Rainbow, 5), (Unicorn, 7)],
        bonus: Bonus::OnComplete(10),
        fail_penalty: 0,
        count: 1,
    },
    Mission {
        name: "Podium Climber",
        chain: &[(SmallHurdle, -1), (BigHurdle, -2), (WinnersPodium, 15)],
        bonus: Bonus::PerItem(10),
        fail_penalty: 0,
        count: 1,
    },
    Mission { name: "Sanitary Experiment", chain: &[(ToiletPaper, 0)], bonus: Bonus::OnComplete(20), fail_penalty: 5, count: 3 },
    // Neúplné: známe jen samotný rozbitý meč, předchozí kroky zatím ne
    Mission { name: "Broken Sword", chain: &[(BrokenSword, -4)], bonus: Bonus::PerItem(8), fail_penalty: 0, count: 1 },
];

/// Mise, jejímž cílem je daný předmět.
pub fn for_target(t: ExpeditionThing) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.target() == t)
}

/// Mise a pozice kroku v jejím řetězu.
pub fn chain_position(t: ExpeditionThing) -> Option<(&'static Mission, usize)> {
    MISSIONS.iter().find_map(|m| m.chain.iter().position(|(c, _)| *c == t).map(|i| (m, i)))
}

/// Věci, které umíme vyhodnotit. Ostatní se zapisují do deníku jako „nezmapované“.
pub fn is_known(t: ExpeditionThing) -> bool {
    chain_position(t).is_some()
        || t.is_bounty_for().is_some()
        || matches!(t, Key | Suitcase | Dummy1 | Dummy2 | Dummy3)
}
