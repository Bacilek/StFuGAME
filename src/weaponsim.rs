//! Throwaway experiment (2026-10-14, user: "verify the weapon formula by simulation"): measures with sf-api's fight
//! simulator how much win rate +1 main attribute, +1 CON, +1 LCK, +1 secondary and +1 % weapon damage are worth, using the
//! real roster characters (`roster/<nick>/now.json`) and opponents of comparable strength built from them.
//! Run: `cargo test --release weaponsim -- --ignored --nocapture` (separate CARGO_TARGET_DIR, never touch the bot's exe).

use enum_map::EnumMap;
use sf_api::{
    command::AttributeType,
    gamestate::character::Class,
    simulate::{DamageRange, Fighter, Weapon, simulate_battle},
};

use crate::tournament::parse_class;

const ATTRS: [(&str, AttributeType); 5] = [
    ("STR", AttributeType::Strength),
    ("DEX", AttributeType::Dexterity),
    ("INT", AttributeType::Intelligence),
    ("CON", AttributeType::Constitution),
    ("LCK", AttributeType::Luck),
];

fn dmg_of(item: &serde_json::Value) -> Option<(f64, f64)> {
    let d = item["d"].as_str()?;
    let part = d.split(", ").find_map(|p| p.strip_suffix(" dmg"))?;
    let (a, b) = part.split_once('–')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn weapon(d: (f64, f64)) -> Weapon {
    Weapon { rune_value: 0, rune_type: None, damage: DamageRange { min: d.0, max: d.1 } }
}

fn load(nick: &str) -> Option<Fighter> {
    let text = std::fs::read_to_string(format!("roster/{nick}/now.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let class = parse_class(v["class"].as_str()?)?;
    let level = u16::try_from(v["level"].as_u64()?).ok()?;
    let mut attributes: EnumMap<AttributeType, u32> = EnumMap::default();
    for (k, a) in ATTRS {
        attributes[a] = u32::try_from(v["attrs"][k]["total"].as_u64()?).ok()?;
    }
    let first = dmg_of(&v["equip"]["Weapon"]);
    let second = (class == Class::Assassin).then(|| dmg_of(&v["equip"]["Shield"])).flatten();
    Some(Fighter {
        ident: Default::default(),
        name: std::sync::Arc::from(nick),
        class,
        level,
        attributes,
        max_health: v["derived"]["hp"].as_f64()?,
        armor: u32::try_from(v["derived"]["armor"].as_u64()?).ok()?,
        first_weapon: first.map(weapon),
        second_weapon: second.map(weapon),
        has_reaction_enchant: false,
        crit_dmg_multi: 2.0,
        resistances: EnumMap::default(),
        portal_dmg_bonus: 0.0,
        is_companion: false,
        gladiator_lvl: 0,
    })
}


/// A copy of `o` scaled by `g` (attributes, hit points, weapon damage, armor) at level `level`.
fn scaled(o: &Fighter, g: f64, level: u16) -> Fighter {
    let mut f = o.clone();
    for (_, v) in f.attributes.iter_mut() {
        *v = (f64::from(*v) * g).round() as u32;
    }
    f.max_health = o.max_health * g * f64::from(level + 1) / f64::from(o.level + 1);
    f.armor = (f64::from(o.armor) * g) as u32;
    f.level = level;
    for w in [&mut f.first_weapon, &mut f.second_weapon].into_iter().flatten() {
        w.damage = DamageRange { min: w.damage.min * g, max: w.damage.max * g };
    }
    f
}

fn win_rate(me: &Fighter, pool: &[Fighter], iters: u32) -> f64 {
    pool.iter().map(|o| simulate_battle(std::slice::from_ref(me), std::slice::from_ref(o), iters, true).win_ratio).sum::<f64>()
        / pool.len() as f64
}

fn main_attr(c: Class) -> AttributeType {
    c.main_attribute()
}

/// Opponents of similar strength: every roster character (class mix) scaled so the mean win rate is about 50 %.
fn pool_for(me: &Fighter, all: &[Fighter]) -> Vec<Fighter> {
    let ma = main_attr(me.class);
    let build = |g: f64| -> Vec<Fighter> {
        [0.8, 0.9, 1.0, 1.1, 1.2]
            .iter()
            .flat_map(|s| {
                all.iter().map(move |o| {
                    let k = f64::from(me.attributes[ma]) / f64::from(o.attributes[main_attr(o.class)]).max(1.0) * g * s;
                    scaled(o, k, me.level)
                })
            })
            .collect()
    };
    let (mut lo, mut hi) = (0.4, 2.5);
    for _ in 0..9 {
        let mid = f64::sqrt(lo * hi);
        let w = win_rate(me, &build(mid), 600);
        if w > 0.5 { lo = mid } else { hi = mid }
    }
    build(f64::sqrt(lo * hi))
}

fn add(v: u32, d: f64) -> u32 {
    (f64::from(v) + d).max(0.0).round() as u32
}

fn bump(f: &Fighter, what: &str, amount: f64) -> Fighter {
    let mut f = f.clone();
    let ma = main_attr(f.class);
    match what {
        "main" => f.attributes[ma] = add(f.attributes[ma], amount),
        "con" => {
            let c = f64::from(f.attributes[AttributeType::Constitution]);
            f.max_health *= (c + amount) / c;
            f.attributes[AttributeType::Constitution] = add(f.attributes[AttributeType::Constitution], amount);
        }
        "lck" => f.attributes[AttributeType::Luck] += amount as u32,
        "sec" => {
            // the weakest non-main, non-CON, non-LCK attribute
            let a = [AttributeType::Strength, AttributeType::Dexterity, AttributeType::Intelligence]
                .into_iter()
                .filter(|a| *a != ma)
                .min_by_key(|a| f.attributes[*a])
                .unwrap();
            f.attributes[a] = add(f.attributes[a], amount);
        }
        "dmg" => {
            for w in [&mut f.first_weapon, &mut f.second_weapon].into_iter().flatten() {
                w.damage = DamageRange { min: w.damage.min * (1.0 + amount / 100.0), max: w.damage.max * (1.0 + amount / 100.0) };
            }
        }
        _ => unreachable!(),
    }
    f
}

#[test]
#[ignore]
fn exchange_rates() {
    let nicks = ["Chlamydie", "Filminy", "Květoš", "Mimimimi11", "Mrožik", "Novotné", "Pagan", "PajaRizz", "Pjotr", "Sanek", "Wecros"];
    let all: Vec<Fighter> = nicks.iter().filter_map(|n| load(n)).collect();
    println!("loaded {} characters", all.len());
    println!("{:<12}{:>6}{:>7}{:>8}{:>8}{:>8}{:>8}{:>9}{:>9}{:>9}", "nick", "class", "lvl", "main", "base W", "con/m", "lck/m", "sec/m", "dmg1%/m", "formula");
    let iters = 12000;
    let mut sums = [0.0f64; 4];
    let mut count = 0.0;
    for me in &all {
        let pool = pool_for(me, &all);
        let m = f64::from(me.attributes[main_attr(me.class)]);
        let con = f64::from(me.attributes[AttributeType::Constitution]);
        // step sizes: ~6 % of the relevant quantity so that the effect is well above the noise
        let (dm, dc, dl, ds, dd) = (m * 0.06, con * 0.06, m * 0.1, m * 0.2, 6.0);
        let w0 = win_rate(me, &pool, iters * 2);
        let w0b = win_rate(me, &pool, iters * 2);
        let base = (w0 + w0b) / 2.0;
        let eff = |what: &str, amt: f64| {
            let (fp, fm) = (bump(me, what, amt), bump(me, what, -amt));
            let w = (win_rate(&fp, &pool, iters * 2) - win_rate(&fm, &pool, iters * 2)) / 2.0;
            let _ = base;
            w / amt
        };
        let per_main = eff("main", dm);
        let r_con = eff("con", dc) / per_main;
        let r_lck = eff("lck", dl) / per_main;
        let r_sec = eff("sec", ds) / per_main;
        let r_dmg = eff("dmg", dd) / per_main;
        println!(
            "{:<12}{:>6}{:>7}{:>8.0}{:>8.3}{:>8.2}{:>8.2}{:>9.2}{:>9.2}{:>9.2}",
            me.name,
            format!("{:?}", me.class).chars().take(5).collect::<String>(),
            me.level,
            m,
            base,
            r_con,
            r_lck,
            r_sec,
            r_dmg,
            (20.0 + m) / 100.0
        );
        if per_main > 0.0 {
            sums[0] += r_con;
            sums[1] += r_lck;
            sums[2] += r_sec;
            sums[3] += r_dmg / ((20.0 + m) / 100.0);
            count += 1.0;
        }
    }
    println!(
        "MEAN: con {:.2} (formula 0.8), lck {:.2} (0.4), secondary {:.2} (0.1), dmg1% vs (20+M)/100: {:.2}x",
        sums[0] / count,
        sums[1] / count,
        sums[2] / count,
        sums[3] / count
    );

}

/// Random weapon swaps: truth = simulated change of the win rate, predictions = the formulas.
#[test]
#[ignore]
fn validate_formulas() {
    let nicks = ["Chlamydie", "Filminy", "Květoš", "Mimimimi11", "Mrožik", "Novotné", "Pagan", "PajaRizz", "Pjotr", "Sanek", "Wecros"];
    let all: Vec<Fighter> = nicks.iter().filter_map(|n| load(n)).collect();
    let mut rng = fastrand::Rng::with_seed(7);
    // (name, predictions per sample), truth
    let names = ["old (avg dmg raw)", "new (damage_points)", "cal 0.7/1.2/0.13/0.15", "cal2 0.7/1.0/0.2/0.15"];
    let mut pred: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
    let mut truth: Vec<f64> = Vec::new();
    for me in &all {
        let pool = pool_for(me, &all);
        let ma = main_attr(me.class);
        let m = f64::from(me.attributes[ma]);
        let con = f64::from(me.attributes[AttributeType::Constitution]);
        let avg = me.first_weapon.as_ref().map_or(30.0, |w| (w.damage.min + w.damage.max) / 2.0);
        let w_base = win_rate(me, &pool, 3000);
        for _ in 0..40 {
            let r = 0.8 + rng.f64() * 0.45;
            let dm = (rng.f64() * 60.0 - 30.0).round();
            let dc = (rng.f64() * 90.0 - 30.0).round();
            let dl = (rng.f64() * 80.0 - 20.0).round();
            let ds = (rng.f64() * 40.0).round();
            let mut f = bump(me, "dmg", (r - 1.0) * 100.0);
            f = bump(&f, "main", dm);
            f = bump(&f, "con", dc);
            f = bump(&f, "lck", dl);
            f = bump(&f, "sec", ds);
            truth.push(win_rate(&f, &pool, 3000) - w_base);
            let ratio = r * (1.0 + (m + dm) / 20.0) / (1.0 + m / 20.0);
            let old = avg * r * (1.0 + (m + dm) / 20.0) - avg * (1.0 + m / 20.0);
            let dmg_pts = (ratio - 1.0) * (20.0 + m);
            let _ = con;
            pred[0].push(old + 0.8 * dc + 0.4 * dl + 0.1 * ds);
            pred[1].push(dmg_pts + 0.8 * dc + 0.4 * dl + 0.1 * ds);
            pred[2].push(0.7 * dmg_pts + 1.2 * dc + 0.13 * dl + 0.15 * ds);
            pred[3].push(0.7 * dmg_pts + 1.0 * dc + 0.2 * dl + 0.15 * ds);
        }
    }
    let n = truth.len() as f64;
    let mean = |v: &[f64]| v.iter().sum::<f64>() / n;
    let corr = |a: &[f64], b: &[f64]| {
        let (ma, mb) = (mean(a), mean(b));
        let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
        let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum();
        let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
        cov / (va * vb).sqrt()
    };
    for (i, name) in names.iter().enumerate() {
        let (mut ok, mut tot) = (0, 0);
        for (p, t) in pred[i].iter().zip(&truth) {
            if t.abs() > 0.01 {
                tot += 1;
                if (*p > 0.0) == (*t > 0.0) {
                    ok += 1;
                }
            }
        }
        println!("{name:<28} corr {:.3}  sign {ok}/{tot} ({:.1} %)", corr(&pred[i], &truth), 100.0 * f64::from(ok) / f64::from(tot));
    }
}
