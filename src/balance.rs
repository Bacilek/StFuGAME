//! Main attribute / Constitution balance (user 2026-10-16). Experts (and the user) hold that a well built character has
//! Constitution at about 80 % of the main attribute (1000 INT : 800 CON). Item and gem values are therefore judged on the
//! RESULTING character: `value_of(main, con)` scores a (main, CON) pair and gets slightly smaller the further the pair is from
//! 100 : 80, so of two items the one that leaves us closer to the ideal gains a bit more. Raw numbers only (bought points + worn
//! items + their gems; no potions, pets, ...). Luck and the secondary attributes do not take part: they keep their fixed
//! weights in `inventory.rs`, so being unbalanced never makes a Luck item better than a main-attribute item.
//!
//! TO CALIBRATE: change the two constants below. `BALANCE_STRENGTH = 0.0` switches the balancing off (plain 100 % main + 80 % CON).

/// The ideal Constitution as a share of the main attribute.
pub const CON_TARGET: f64 = 0.8;
/// How much a character that is off the target loses in value: `value × (1 − BALANCE_STRENGTH × d²)`, where `d` is the relative
/// deviation of CON from its target (d = −0.5: CON is half a target too low). 0.25 = a 50 % deviation costs 6 % of the value
/// (agreed with the user: balance a bit more than before, never at the expense of clearly better items).
pub const BALANCE_STRENGTH: f64 = 0.25;
/// The CON weight in the power formula (`arena::power`).
const CON_WEIGHT: f64 = 0.8;

/// Value of a (main attribute, Constitution) pair.
pub fn value_of(main: f64, con: f64) -> f64 {
    let base = main + CON_WEIGHT * con;
    if main <= 0.0 || BALANCE_STRENGTH == 0.0 {
        return base;
    }
    let d = (con / (CON_TARGET * main) - 1.0).clamp(-1.0, 1.0);
    base * (1.0 - BALANCE_STRENGTH * d * d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_the_ideal_nothing_changes() {
        // 1000 : 799 is not a problem (user)
        let loss = 1.0 - value_of(1000.0, 799.0) / (1000.0 + 0.8 * 799.0);
        assert!(loss < 1e-5, "{loss}");
        assert!((value_of(1000.0, 800.0) - 1640.0).abs() < 1e-9);
    }

    #[test]
    fn the_item_that_leaves_us_closer_gains_a_bit_more() {
        // 1200 INT : 600 CON (user's example): CON +50 about +54, INT +60 about +48, both clearly positive
        let base = value_of(1200.0, 600.0);
        let con = value_of(1200.0, 650.0) - base;
        let int = value_of(1260.0, 600.0) - base;
        assert!((con - 54.0).abs() < 1.5, "{con}");
        assert!((int - 48.0).abs() < 1.5, "{int}");
        // Mimimimi11 1582 : 823
        let base = value_of(1582.0, 823.0);
        assert!((value_of(1582.0, 873.0) - base - 54.0).abs() < 1.5);
        assert!((value_of(1642.0, 823.0) - base - 48.0).abs() < 1.5);
    }

    #[test]
    fn too_much_constitution_is_balanced_the_other_way() {
        let base = value_of(600.0, 1200.0);
        assert!(value_of(660.0, 1200.0) - base > value_of(600.0, 1260.0) - base);
    }

    #[test]
    fn extremes_stay_sane() {
        assert!(value_of(0.0, 100.0) > 0.0);
        assert!(value_of(100.0, 0.0) > 0.0);
        assert!(value_of(100.0, 100_000.0) > 0.0);
        // adding a main attribute point never lowers the value
        for (m, c) in [(1500.0, 800.0), (1000.0, 100.0), (500.0, 2000.0), (10.0, 0.0)] {
            assert!(value_of(m + 1.0, c) > value_of(m, c), "{m} {c}");
        }
    }
}
