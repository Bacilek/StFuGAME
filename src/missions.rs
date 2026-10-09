//! Known expedition missions (tasks) and encounter cycles. Description and data sources: docs/expeditions.md.
//! Step heroism comes from the official FAQ, task bonuses from the user.

use sf_api::gamestate::tavern::ExpeditionThing::{self, *};

/// Points for the expedition task.
#[derive(Debug, Clone, Copy)]
pub enum Bonus {
    /// Per collected target item, credited at the end of the expedition ("+5/").
    PerItem(i32),
    /// Once, the moment the task is completed ("+10").
    OnComplete(i32),
}

/// Estimated bonus for a task we have not mapped yet.
const UNKNOWN_BONUS: Bonus = Bonus::OnComplete(5);

#[derive(Debug)]
pub struct Mission {
    pub name: &'static str,
    /// Chain of steps (item, heroism when collected). The last step is the target item.
    pub chain: &'static [(ExpeditionThing, i32)],
    pub bonus: Bonus,
    /// Is the bonus confirmed by the user? (otherwise just an estimate)
    pub bonus_known: bool,
    /// Heroism deducted when the task is not completed at the end (positive number).
    pub fail_penalty: i32,
    /// Number of target items needed to complete the task (1 for chains).
    pub count: u8,
    /// How many times the cycle can repeat during an expedition (FAQ "Limit"). Informational only for now.
    #[allow(dead_code)]
    pub limit: u8,
    /// The last step stays on offer even after completion (FAQ "last encounter remains permanently").
    /// Informational only for now.
    #[allow(dead_code)]
    pub final_repeats: bool,
}

impl Mission {
    pub fn target(&self) -> ExpeditionThing {
        self.chain.last().map_or(Unknown, |(t, _)| *t)
    }

    /// How easily the mission yields heroism: average per round when completed
    /// (step heroism + bonus + avoided penalty) / rounds needed. Higher = easier 40.
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

/// Mission with a known bonus. The other fields are shared by most missions.
const fn known(name: &'static str, chain: &'static [(ExpeditionThing, i32)], bonus: Bonus, limit: u8) -> Mission {
    Mission { name, chain, bonus, bonus_known: true, fail_penalty: 0, count: 1, limit, final_repeats: false }
}

/// Mission whose task bonus is not known yet (estimate UNKNOWN_BONUS).
const fn unknown(name: &'static str, chain: &'static [(ExpeditionThing, i32)], limit: u8) -> Mission {
    Mission { name, chain, bonus: UNKNOWN_BONUS, bonus_known: false, fail_penalty: 0, count: 1, limit, final_repeats: false }
}

/// Mission that deducts heroism when not completed (user data "[+bonus,-penalty]").
const fn penalty(mut m: Mission, p: i32) -> Mission {
    m.fail_penalty = p;
    m
}

const fn repeats(mut m: Mission) -> Mission {
    m.final_repeats = true;
    m
}

// Values verified on the server take precedence over the FAQ (see docs/expeditions.md, "Verification status").
pub const MISSIONS: &[Mission] = &[
    known("Dragon Taming", &[(Bait, -2), (Dragon, 10)], Bonus::PerItem(5), 2),
    repeats(known(
        "Extinguished Fire",
        &[(CampFire, 3), (Phoenix, 5), (BurntCampfire, 0)],
        Bonus::PerItem(4),
        1,
    )),
    // Chicken drumstick (CupCake) and suckling pig (Cake) narrow the next crossroads to 2 and 1 options
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
    // FAQ: 5, 2, -5; server 2026-10-07: 6, 3, -4
    repeats(known(
        "The Sword Trial",
        &[(SwordInStone, 6), (BentSword, 3), (BrokenSword, -4)],
        Bonus::PerItem(8),
        1,
    )),
    unknown("Revealing Lady", &[(Socks, 0), (ClothPile, 0), (RevealingCouple, 12)], 1),
    // sf-api: Well = cauldron, Girl = witch, Balloons = witch's brew
    // user 2026-10-10: cauldron -2, witch -5, brew +15 (server-verified), bonus +5 on completion, -5 when failed
    penalty(known("Bewitched Stew", &[(Well, -2), (Girl, -5), (Balloons, 15)], Bonus::OnComplete(5), 1), 5),
    // user 2026-10-10: mugs 0, draft beer +6, tapping bartender +6, bonus +5 on completion, -5 when failed
    penalty(known("Running Dry", &[(Mugs, 0), (DraftBeer, 6), (Barkeeper, 6)], Bonus::OnComplete(5), 1), 5),
    // sf-api ids: Prince = fairy fountain, RoyalFrog = polluted fairy fountain
    // user 2026-10-10: fountain +8, polluted fountain -4, bonus +8 per piece (net +4 each), 0 when failed
    repeats(known("Toxic Fountain Cure", &[(Prince, 8), (RoyalFrog, -4)], Bonus::PerItem(8), 1)),
    unknown("Build A Friend", &[(Hand, -5), (Feet, -5), (Body, -5), (Klaus, 35)], 1),
];

/// Mission whose target is the given item.
pub fn for_target(t: ExpeditionThing) -> Option<&'static Mission> {
    MISSIONS.iter().find(|m| m.target() == t)
}

/// Mission and the step's position in its chain.
pub fn chain_position(t: ExpeditionThing) -> Option<(&'static Mission, usize)> {
    MISSIONS.iter().find_map(|m| m.chain.iter().position(|(c, _)| *c == t).map(|i| (m, i)))
}

/// Things we can evaluate. Everything else is logged to the journal as "unmapped".
pub fn is_known(t: ExpeditionThing) -> bool {
    chain_position(t).is_some()
        || t.is_bounty_for().is_some()
        || matches!(t, Key | Suitcase | Dummy1 | Dummy2 | Dummy3 | CupCake)
}
