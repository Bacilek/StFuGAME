//! Where and when each of the 100 pets can be found in a Tavern expedition (`docs/pet-conditions.md`, read in the game by the user and
//! verified 2026-10-23). Used by the Tavern to prefer the expedition that passes the location of a pet we still lack while its condition
//! holds, to take the egg of a new pet, and to keep the Thirst for Adventure for the day when only Day pets are missing (`docs/pets.md`).
//!
//! Assumptions that the first live runs must confirm (the `[pets] Egg offered` line prints everything needed to compare):
//! day = 06:00-16:00 and night = 18:00-06:00 (user-confirmed; 16-18 is neither, nothing is planned there), seasons are the meteorological ones,
//! a weekday means the calendar day (Monday 03:00 is "Monday night"), and a condition counts when it holds at the start AND half an
//! expedition later (the egg comes mid-way).

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Timelike, Weekday};
use sf_api::gamestate::{
    GameState,
    rewards::Event,
    tavern::{AvailableExpedition, Location},
};

use crate::pets;

const DAY_FROM: u32 = 6;
const DAY_TO: u32 = 16;
const NIGHT_FROM: u32 = 18;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Time {
    Day,
    Night,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Season {
    Spring,
    Summer,
    Fall,
    Winter,
}

#[derive(Clone, Copy)]
enum Rule {
    /// A time of day.
    At(Time),
    /// A weekday and a time of day.
    On(Weekday, Time),
    /// A season and a time of day.
    In(Season, Time),
    /// Exactly this hour of the day (Jacobu 00-01).
    Hour(u32),
    /// A whole calendar month (Antlar: December).
    Month(u32),
    /// A date rule (April Fools +-3 days, ...).
    Dates(fn(NaiveDate) -> bool),
    /// A server event is running.
    Event(Event),
    /// The last pet of a habitat: only the location, it needs the whole habitat beaten (`pets::explored`).
    Last,
    /// Not an expedition condition we can check (Hall of Fame, shops, Gem Mine, dungeons, legendary dungeon, Pentecost, ...). The egg of
    /// such a pet is still taken when it shows up, but the Tavern never plans for it.
    Manual,
}

struct Entry {
    name: &'static str,
    loc: Option<Location>,
    rule: Rule,
}

const fn p(name: &'static str, loc: Location, rule: Rule) -> Entry {
    Entry { name, loc: Some(loc), rule }
}

const fn manual(name: &'static str, loc: Option<Location>) -> Entry {
    Entry { name, loc, rule: Rule::Manual }
}

fn within(d: NaiveDate, month: u32, day: u32, days: i64) -> bool {
    (d.year() - 1..=d.year() + 1)
        .filter_map(|y| NaiveDate::from_ymd_opt(y, month, day))
        .any(|t| (d - t).num_days().abs() <= days)
}

fn april_fools(d: NaiveDate) -> bool {
    within(d, 4, 1, 3)
}

fn valentine(d: NaiveDate) -> bool {
    within(d, 2, 14, 3)
}

fn new_year(d: NaiveDate) -> bool {
    (d.month() == 12 && d.day() == 31) || (d.month() == 1 && d.day() == 1)
}

fn friday_13(d: NaiveDate) -> bool {
    (-3..=3).any(|k| {
        let x = d + Duration::days(k);
        x.weekday() == Weekday::Fri && x.day() == 13
    })
}

use Location::{
    BlackWaterSwamp as Bws, BustedLands as Busted, Gnarogrim as Gnaro, Maerwynn as Maer, Magmaron as Magma, MoldyForest as Moldy,
    PlainsOfOzKorr as Oz, RottenLands as Rotten, ShadowrockMountain as Shadowrock, SkullIsland as Skull, SprawlingJungle as Jungle,
    SunburnDesert as Desert,
};
use Rule::{At, Dates, Event as Ev, Hour, In, Last, Month, On};
use Season::{Fall, Spring, Summer, Winter};
use Time::{Day, Night};
use Weekday::{Fri, Mon, Sat, Sun, Thu, Tue, Wed};

/// Index = pet id - 1 (Shadow 1-20, Light 21-40, Earth 41-60, Fire 61-80, Water 81-100).
const PETS: [Entry; 100] = [
    // Shadow
    p("Slurp", Moldy, At(Day)),
    p("Digmol", Rotten, At(Night)),
    p("Toothey", Jungle, At(Night)),
    p("Okultacle", Moldy, At(Night)),
    p("Spidor", Rotten, On(Mon, Night)),
    p("Jacobu", Bws, Hour(0)),
    p("Shrimpfly", Jungle, In(Summer, Day)),
    p("Reaprim", Rotten, In(Winter, Day)),
    p("Petdacat", Shadowrock, On(Fri, Night)),
    p("Mykon", Moldy, In(Fall, Day)),
    p("Fishorr", Bws, On(Mon, Night)),
    p("Cuckooly", Rotten, On(Tue, Night)),
    p("Battlutter", Shadowrock, On(Wed, Night)),
    p("Pinklynx", Shadowrock, On(Thu, Day)),
    p("Pharamumm", Rotten, Ev(Event::CrazyMushroomHarvest)),
    manual("Ninstarr", Some(Bws)),
    p("Luchtablong", Shadowrock, Ev(Event::ExceptionalXPEvent)),
    p("Angrack", Rotten, Dates(friday_13)),
    manual("Devilsatt", Some(Bws)),
    p("Poisnake", Jungle, Last),
    // Light
    p("Shaggyl", Oz, At(Day)),
    p("Jellclops", Busted, At(Night)),
    p("Tinck", Maer, At(Day)),
    p("Cloudning", Busted, At(Day)),
    p("Nevorfull", Maer, In(Spring, Night)),
    p("Plutoid", Busted, On(Sat, Day)),
    p("Djinntonic", Desert, In(Summer, Day)),
    p("Blaxta", Oz, In(Fall, Day)),
    p("Lampcess", Desert, On(Mon, Night)),
    p("Teslarr", Busted, On(Tue, Day)),
    p("Sunnya", Desert, On(Sun, Day)),
    p("Buckfoxion", Maer, On(Wed, Night)),
    p("Birdychirp", Oz, On(Thu, Day)),
    p("Eyeorwhat", Busted, On(Fri, Day)),
    manual("Mesmerit", Some(Desert)),
    p("Antlar", Shadowrock, Month(12)),
    p("Liphant", Busted, Dates(april_fools)),
    p("Knilight", Oz, Ev(Event::EpicShoppingSpreeExtravaganza)),
    manual("Heraldon", Some(Maer)),
    p("Unikor", Oz, Last),
    // Earth
    p("Mamoton", Gnaro, At(Day)),
    p("Monkorrage", Jungle, At(Night)),
    p("Smaponyck", Maer, At(Day)),
    p("Bittnutz", Oz, At(Day)),
    p("Roarear", Gnaro, At(Night)),
    p("Muscudon", Oz, In(Summer, Day)),
    p("Apstick", Jungle, In(Fall, Day)),
    p("Horrnington", Gnaro, In(Winter, Day)),
    p("Boaringg", Maer, In(Spring, Day)),
    p("Mameloth", Jungle, On(Sun, Night)),
    p("Rheynooh", Maer, On(Mon, Day)),
    p("Rockastonn", Gnaro, On(Tue, Night)),
    p("Redwoofox", Maer, On(Wed, Day)),
    p("Lilbeatzup", Oz, On(Thu, Day)),
    manual("Forror", Some(Maer)),
    manual("Nipprabs", Some(Jungle)),
    p("Armoruck", Oz, Ev(Event::GloriousGoldGalore)),
    manual("Canocle", Some(Gnaro)),
    manual("Tricerawood", None),
    p("Mouthrexor", Jungle, Last),
    // Fire
    p("Firimp", Moldy, At(Day)),
    p("Gullps", Magma, At(Night)),
    p("Pyrophibus", Bws, At(Night)),
    p("Flamechirr", Desert, At(Day)),
    p("Tectospit", Magma, On(Tue, Night)),
    p("Pyroplant", Moldy, In(Fall, Night)),
    p("Kokofire", Magma, On(Mon, Day)),
    p("Peppryon", Desert, In(Spring, Day)),
    p("Boomywoomy", Magma, In(Winter, Day)),
    p("Tikiricky", Jungle, On(Wed, Night)),
    p("Matchlit", Desert, On(Thu, Day)),
    p("Birdblazey", Desert, In(Summer, Day)),
    p("Infernox", Magma, On(Fri, Day)),
    p("Hambuzzish", Jungle, On(Sat, Night)),
    manual("Dragopyr", None),
    p("Mantiflame", Desert, Ev(Event::TidyToiletTime)),
    p("Finnettle", Magma, Dates(valentine)),
    p("Etrock", Oz, Dates(new_year)),
    manual("Blazingtongues", Some(Magma)),
    p("Devastator", Gnaro, Last),
    // Water
    p("Goldy", Skull, At(Day)),
    p("Orcahle", Skull, At(Day)),
    p("Ocodile", Bws, At(Night)),
    p("Penguwater", Shadowrock, At(Day)),
    p("Walrophin", Shadowrock, On(Mon, Day)),
    p("Colsnail", Moldy, On(Tue, Night)),
    p("Aquaphant", Moldy, On(Wed, Day)),
    p("Naar", Shadowrock, In(Winter, Day)),
    p("Octoboss", Moldy, On(Thu, Night)),
    p("Ewigryn", Skull, On(Fri, Day)),
    p("Seapard", Skull, On(Sat, Day)),
    p("Shellzy", Skull, In(Fall, Night)),
    p("Mingho", Bws, In(Summer, Day)),
    p("Angbite", Shadowrock, On(Sun, Night)),
    manual("Mermoid", Some(Skull)),
    manual("Watsnake", None),
    manual("Cannoturtle", Some(Skull)),
    manual("Ungere", None),
    manual("Tritosting", Some(Moldy)),
    p("Hydrospir", Bws, Last),
];

const HABITATS: [&str; 5] = ["Shadow", "Light", "Earth", "Fire", "Water"];

fn season(month: u32) -> Season {
    match month {
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        9..=11 => Season::Fall,
        _ => Season::Winter,
    }
}

fn time_ok(t: Time, at: DateTime<Local>) -> bool {
    let h = at.hour();
    match t {
        Time::Day => (DAY_FROM..DAY_TO).contains(&h),
        Time::Night => h >= NIGHT_FROM || h < DAY_FROM,
    }
}

fn holds(rule: Rule, at: DateTime<Local>, events: &[Event]) -> bool {
    match rule {
        Rule::At(t) => time_ok(t, at),
        Rule::On(d, t) => at.weekday() == d && time_ok(t, at),
        Rule::In(s, t) => season(at.month()) == s && time_ok(t, at),
        Rule::Hour(h) => at.hour() == h,
        Rule::Month(m) => at.month() == m,
        Rule::Dates(f) => f(at.date_naive()),
        Rule::Event(e) => events.contains(&e),
        Rule::Last => true,
        Rule::Manual => false,
    }
}

/// A pet we could find in an expedition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub id: u32,
    pub name: &'static str,
    pub loc: Location,
}

/// Name of pet `id` (1..=100).
pub fn name(id: u32) -> &'static str {
    PETS[id as usize - 1].name
}

fn describe(id: u32) -> String {
    let e = &PETS[id as usize - 1];
    format!("{} (#{} {}, id {id})", e.name, (id - 1) % 20 + 1, HABITATS[(id as usize - 1) / 20])
}

/// Server events running now.
pub fn events(gs: &GameState) -> Vec<Event> {
    let ev = &gs.specials.events;
    if ev.ends.is_some_and(|e| e > Local::now()) { ev.active.iter().copied().collect() } else { Vec::new() }
}

/// The first three pets of every habitat can be found from the start whatever the dungeon counter says (user finding 2026-10-21; verified in the
/// raw data 2026-10-26: Pjotr owns Ocodile, position 3, with the Water counter at 0). Before, the sf-api rule `explored >= position - 1` hid all
/// of them (real counters are 0 or 1), so nobody hunted Ocodile, Smaponyck, Toothey, Tinck or Gullps in the Tavern.
const FREE_POSITIONS: u32 = 3;

/// Pets we do not own, whose habitat dungeon is far enough (the sf-api rule `explored >= position - 1`, except the first three), with a condition we can check.
fn candidates(nums: &[i64]) -> impl Iterator<Item = (u32, &'static Entry)> + '_ {
    let ready = nums.len() > 250;
    (1..=100u32).filter_map(move |id| {
        let e = &PETS[id as usize - 1];
        let h = (id as usize - 1) / 20;
        let pos0 = (id - 1) % 20;
        (ready && pets::level(nums, id) == 0 && (pos0 < FREE_POSITIONS || pets::explored(nums, h) >= pos0) && !matches!(e.rule, Rule::Manual) && e.loc.is_some())
            .then_some((id, e))
    })
}

/// The pets we still lack that can be found at `at`.
pub fn catchable(nums: &[i64], events: &[Event], at: DateTime<Local>) -> Vec<Target> {
    candidates(nums)
        .filter(|(_, e)| holds(e.rule, at, events))
        .filter_map(|(id, e)| Some(Target { id, name: e.name, loc: e.loc? }))
        .collect()
}

/// Missing pets that an offered expedition can bring: a wanted location AND the condition at the start and half an expedition later.
pub fn hits(nums: &[i64], events: &[Event], e: &AvailableExpedition, at: DateTime<Local>) -> Vec<Target> {
    let later = at + Duration::seconds(i64::from(e.thirst_for_adventure_sec / 2));
    let second = catchable(nums, events, later);
    catchable(nums, events, at)
        .into_iter()
        .filter(|t| t.loc == e.location_1 || t.loc == e.location_2)
        .filter(|t| second.contains(t))
        .collect()
}

/// Hunting score of an offered expedition: 0 = nothing wanted, 1 = passes the location of a catchable missing pet, 2 = and the game
/// flags an egg for this expedition.
pub fn score(nums: &[i64], events: &[Event], e: &AvailableExpedition, at: DateTime<Local>) -> u8 {
    if hits(nums, events, e, at).is_empty() {
        0
    } else if e.special == Some(sf_api::gamestate::tavern::ExpeditionSpecial::Egg) {
        2
    } else {
        1
    }
}

/// When the Tavern should wait instead of burning Thirst for Adventure (user 2026-10-23): no missing pet can be found right now, one can
/// later today, and from then on until midnight (when the Thirst for Adventure resets) there is still room for all of it (+ 1 h). The
/// typical case: it is night, only Day pets are missing -> wait for 06:00. Returns when the window opens.
pub fn wait_until(nums: &[i64], events: &[Event], now: DateTime<Local>, thirst_sec: u32) -> Option<DateTime<Local>> {
    if candidates(nums).next().is_none() || !catchable(nums, events, now).is_empty() {
        return None;
    }
    let midnight = now.date_naive().succ_opt()?.and_hms_opt(0, 0, 0)?.and_local_timezone(Local).earliest()?;
    let mut t = now;
    while t < midnight {
        t += Duration::minutes(10);
        if t < midnight && !catchable(nums, events, t).is_empty() {
            let room = (midnight - t).num_seconds();
            return (room >= i64::from(thirst_sec) + 3600).then_some(t);
        }
    }
    None
}

/// How much an egg of pet `id` is worth to us (reward choice): owned or unknown (nothing), a pet outside the user's chains, a chain pet,
/// a chain pet that is hard to get.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EggKind {
    Useless,
    Other,
    Chain,
    HardChain,
}

pub fn egg_kind(nums: &[i64], id: u64) -> EggKind {
    let Ok(id) = u32::try_from(id) else { return EggKind::Useless };
    if !(1..=100).contains(&id) || nums.len() <= 250 || pets::level(nums, id) > 0 {
        return EggKind::Useless;
    }
    if !pets::in_chain(id) {
        return EggKind::Other;
    }
    let hard = (id - 1) % 20 >= 14 || matches!(PETS[id as usize - 1].rule, Rule::In(..) | Rule::Dates(_) | Rule::Month(_) | Rule::Event(_));
    if hard { EggKind::HardChain } else { EggKind::Chain }
}

/// The last expedition we started: (locations, start, pets we expected), for the verification line when an egg shows up.
static LAST_START: crate::ctx::PerChar<Option<(Location, Location, DateTime<Local>, Vec<Target>)>> = crate::ctx::PerChar::new();

pub fn note_start(e: &AvailableExpedition, at: DateTime<Local>, expected: Vec<Target>) {
    if let Ok(mut s) = LAST_START.lock() {
        *s = Some((e.location_1, e.location_2, at, expected));
    }
}

/// Logs an egg on offer with what we predicted, to confirm the conditions of `docs/pet-conditions.md` and the assumptions above.
pub fn note_egg(nums: &[i64], id: u64) {
    let Ok(id32) = u32::try_from(id) else { return };
    if !(1..=100).contains(&id32) {
        report!("[pets] Egg offered with an unknown pet id {id}");
        return;
    }
    let now = Local::now();
    let start = LAST_START.lock().ok().and_then(|s| s.clone());
    let started = start.map_or("start unknown".to_string(), |(a, b, at, exp)| {
        let exp: Vec<String> = exp.iter().map(|t| t.name.to_string()).collect();
        format!(
            "expedition started {} through {a:?}/{b:?}, expected [{}]",
            at.format("%a %d.%m. %H:%M"),
            if exp.is_empty() { "none".to_string() } else { exp.join(", ") }
        )
    });
    report!("[pets] Egg offered: {} ({:?}), {started}; now {}", describe(id32), egg_kind(nums, id), now.format("%a %H:%M"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, mo, d, h, 0, 0).unwrap()
    }

    /// A fresh account: the dungeon counters are 0 and the first three pets of every habitat can still be found, nothing owned.
    fn fresh() -> Vec<i64> {
        let mut n = vec![0; 265];
        for h in 0..5 {
            n[210 + h] = 0;
        }
        n
    }

    fn names(v: Vec<Target>) -> Vec<&'static str> {
        v.into_iter().map(|t| t.name).collect()
    }

    #[test]
    fn table_matches_the_ids() {
        assert_eq!(PETS[22].name, "Tinck");
        assert_eq!(PETS[82].name, "Ocodile");
        assert_eq!(PETS[99].name, "Hydrospir");
        assert_eq!(PETS[40].name, "Mamoton");
    }

    #[test]
    fn day_and_night_pets_follow_the_clock() {
        let n = fresh();
        // Friday 2026-10-23 10:00: the Day pets among the first three of each habitat
        let day = names(catchable(&n, &[], at(2026, 10, 23, 10)));
        assert!(day.contains(&"Slurp") && day.contains(&"Tinck") && day.contains(&"Smaponyck") && day.contains(&"Goldy"));
        assert!(!day.contains(&"Toothey") && !day.contains(&"Ocodile"));
        let night = names(catchable(&n, &[], at(2026, 10, 23, 22)));
        assert!(night.contains(&"Toothey") && night.contains(&"Ocodile") && night.contains(&"Gullps"));
        assert!(!night.contains(&"Tinck"));
        // 17:00 is neither
        assert!(catchable(&n, &[], at(2026, 10, 23, 17)).is_empty());
    }

    #[test]
    fn owned_or_locked_pets_are_not_wanted() {
        let mut n = fresh();
        n[24] = 5; // Tinck (id 23) owned
        assert!(!names(catchable(&n, &[], at(2026, 10, 23, 10))).contains(&"Tinck"));
        // Pinklynx (Thursday day, position 14) is locked at explored 2
        assert!(!names(catchable(&n, &[], at(2026, 10, 22, 10))).contains(&"Pinklynx"));
        n[210] = 13;
        assert!(names(catchable(&n, &[], at(2026, 10, 22, 10))).contains(&"Pinklynx"));
    }

    #[test]
    fn date_rules() {
        assert!(april_fools(NaiveDate::from_ymd_opt(2027, 3, 29).unwrap()));
        assert!(!april_fools(NaiveDate::from_ymd_opt(2027, 3, 28).unwrap()));
        assert!(friday_13(NaiveDate::from_ymd_opt(2026, 11, 13).unwrap()));
        assert!(friday_13(NaiveDate::from_ymd_opt(2026, 11, 10).unwrap()));
        assert!(!friday_13(NaiveDate::from_ymd_opt(2026, 11, 9).unwrap()));
        assert!(valentine(NaiveDate::from_ymd_opt(2027, 2, 11).unwrap()));
        assert!(new_year(NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()));
    }

    #[test]
    fn only_day_pets_missing_at_night_waits_for_the_morning() {
        let mut n = fresh();
        // own every Night pet of the first three positions (Toothey 3, Digmol 2, Ocodile 83, Gullps 62, Pyrophibus 63, Jellclops 22,
        // Monkorrage 42) -> only Day ones are left
        for id in [2, 3, 22, 42, 62, 63, 83] {
            n[id + 1] = 1;
        }
        let w = wait_until(&n, &[], at(2026, 10, 23, 2), 36_000).unwrap();
        assert_eq!(w.hour(), 6);
        // a regular night with a Night pet missing: no waiting
        assert!(wait_until(&fresh(), &[], at(2026, 10, 23, 2), 36_000).is_none());
        // evening: the next Day window is tomorrow (after the Thirst for Adventure reset) -> no waiting
        assert!(wait_until(&n, &[], at(2026, 10, 23, 20), 36_000).is_none());
    }

    #[test]
    fn eggs_rank_by_chain() {
        let n = fresh();
        assert_eq!(egg_kind(&n, 3), EggKind::Chain); // Toothey
        assert_eq!(egg_kind(&n, 1), EggKind::Other); // Slurp is not in the chain
        assert_eq!(egg_kind(&n, 40), EggKind::HardChain); // Unikor
        let mut owned = fresh();
        owned[4] = 3;
        assert_eq!(egg_kind(&owned, 3), EggKind::Useless);
    }
}
