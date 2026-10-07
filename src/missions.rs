//! Známé mise (úkoly) a cykly setkání v expedicích. Popis a zdroje dat: docs/expedice.md.
//! Hrdinství kroků je z oficiálního FAQ, bonusy za úkol od uživatele.

use sf_api::gamestate::tavern::ExpeditionThing::{self, *};

/// Body za úkol expedice.
#[derive(Debug, Clone, Copy)]
pub enum Bonus {
    /// Za každý sebraný cílový předmět, připíše se až na konci expedice („+5/“).
    PerItem(i32),
    /// Jednorázově v moment splnění úkolu („+10“).
    OnComplete(i32),
}

/// Odhad bonusu za úkol, který ještě nemáme zmapovaný.
const UNKNOWN_BONUS: Bonus = Bonus::OnComplete(5);

#[derive(Debug)]
pub struct Mission {
    pub name: &'static str,
    /// Řetěz kroků (předmět, hrdinství při sebrání). Poslední krok je cílový předmět.
    pub chain: &'static [(ExpeditionThing, i32)],
    pub bonus: Bonus,
    /// Je bonus ověřený uživatelem? (jinak jen odhad)
    pub bonus_known: bool,
    /// Kolik hrdinství se strhne, když úkol na konci není splněný (kladné číslo).
    pub fail_penalty: i32,
    /// Kolik cílových předmětů je potřeba ke splnění (u řetězů 1).
    pub count: u8,
    /// Kolikrát se cyklus může během expedice zopakovat (FAQ „Limit“). Zatím jen informativní.
    #[allow(dead_code)]
    pub limit: u8,
    /// Poslední krok zůstává v nabídce i po dokončení (FAQ „last encounter remains permanently“).
    /// Zatím jen informativní.
    #[allow(dead_code)]
    pub final_repeats: bool,
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

/// Mise se známým bonusem. Ostatní pole jsou společná pro většinu misí.
const fn known(name: &'static str, chain: &'static [(ExpeditionThing, i32)], bonus: Bonus, limit: u8) -> Mission {
    Mission { name, chain, bonus, bonus_known: true, fail_penalty: 0, count: 1, limit, final_repeats: false }
}

/// Mise, jejíž bonus za úkol zatím neznáme (odhad UNKNOWN_BONUS).
const fn unknown(name: &'static str, chain: &'static [(ExpeditionThing, i32)], limit: u8) -> Mission {
    Mission { name, chain, bonus: UNKNOWN_BONUS, bonus_known: false, fail_penalty: 0, count: 1, limit, final_repeats: false }
}

const fn repeats(mut m: Mission) -> Mission {
    m.final_repeats = true;
    m
}

pub const MISSIONS: &[Mission] = &[
    known("Dragon Taming", &[(Bait, -2), (Dragon, 10)], Bonus::PerItem(5), 2),
    repeats(known(
        "Extinguished Fire",
        &[(CampFire, 3), (Phoenix, 5), (BurntCampfire, 0)],
        Bonus::PerItem(4),
        1,
    )),
    // Kuřecí stehno (CupCake) a sele (Cake) zúží další rozcestí na 2, resp. 1 možnost
    known("Hot Carnal Craving", &[(Cake, 5)], Bonus::PerItem(3), u8::MAX),
    known(
        "Unicorn Whisperer",
        &[(UnicornHorn, 1), (Donkey, 3), (Rainbow, 5), (Unicorn, 7)],
        Bonus::OnComplete(10),
        1,
    ),
    known("Podium Climber", &[(SmallHurdle, -1), (BigHurdle, -2), (WinnersPodium, 15)], Bonus::PerItem(10), 2),
    Mission {
        name: "Sanitary Emergency",
        chain: &[(ToiletPaper, 0)],
        bonus: Bonus::OnComplete(20),
        bonus_known: true,
        fail_penalty: 5,
        count: 3,
        limit: 3,
        final_repeats: false,
    },
    repeats(known(
        "The Sword Trial",
        &[(SwordInStone, 5), (BentSword, 2), (BrokenSword, -5)],
        Bonus::PerItem(8),
        1,
    )),
    unknown("Revealing Lady", &[(Socks, 0), (ClothPile, 0), (RevealingCouple, 12)], 1),
    // sf-api: Well = kotel, Girl = čarodějnice, Balloons = čarodějný lektvar
    unknown("Bewitched Stew", &[(Well, 2), (Girl, -5), (Balloons, 15)], 1),
    // sf-api: Prince = vílí fontána, RoyalFrog = znečištěná fontána
    repeats(unknown("Toxic Fountain Cure", &[(Prince, 8), (RoyalFrog, -4)], 1)),
    unknown("Build A Friend", &[(Hand, -5), (Feet, -5), (Body, -5), (Klaus, 35)], 1),
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
        || matches!(t, Key | Suitcase | Dummy1 | Dummy2 | Dummy3 | CupCake)
}
