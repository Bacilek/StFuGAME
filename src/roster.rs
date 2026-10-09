//! Character challenge report (user 2026-10-07). Everything goes to `roster/` (local only, gitignored):
//! - during the day `roster/<nick>/notes.log`: successes, issues and gains (gold, mushrooms, XP),
//! - every day at ~23:50 (`due`): `roster/<nick>/history.csv` (one line a day), `days/<date>.issues`,
//!   `roster/issues.txt` (issues of all characters), `roster/leaderboard.md` and `roster/dashboard.html` (charts).
//!
//! Details: `roster/README.md`.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{Local, NaiveDate, NaiveTime};
use sf_api::{
    command::AttributeType,
    gamestate::{
        GameState,
        character::Class,
        dungeons::DungeonProgress,
        items::{EquipmentSlot, Item, ItemType},
        tavern::{CurrentAction, ExpeditionStage},
    },
};

const ROOT: &str = "roster";
/// The daily report is written at this time.
const REPORT_HOUR: u32 = 23;
const REPORT_MINUTE: u32 = 50;

static NICK: Mutex<Option<String>> = Mutex::new(None);
/// Gold (silver) and mushrooms gained by one command (all income incl. sales; spending is not counted).
pub fn ledger(silver: u64, mushrooms: u32, xp: u64) {
    if xp > 0 {
        note("XP", 0, &xp.to_string());
    }
    if silver > 0 {
        note("GOLD", 0, &silver.to_string());
    }
    if mushrooms > 0 {
        note("MUSH", 0, &mushrooms.to_string());
    }
}

/// Sets the character whose folder the notes go to (after login).
pub fn set_character(name: &str) {
    if let Ok(mut n) = NICK.lock() {
        *n = Some(name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"));
    }
}

fn dir() -> Option<PathBuf> {
    // The character of the current task; `set_character` only for demos/tests outside of a character task
    let ctx = crate::ctx::name();
    let nick = if ctx.is_empty() { NICK.lock().ok()?.clone()? } else { ctx };
    Some(Path::new(ROOT).join(nick))
}

fn append(path: &Path, line: &str) {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn note(kind: &str, prio: u8, msg: &str) {
    let Some(d) = dir() else { return };
    let line = format!("{}\t{kind}\t{prio}\t{}", Local::now().format("%Y-%m-%d %H:%M:%S"), msg.replace('\n', " "));
    append(&d.join("notes.log"), &line);
}

/// An explicit success of the day (higher `prio` = bigger).
pub fn win(prio: u8, msg: &str) {
    note("WIN", prio, msg);
}

/// Successes worth noting, recognised in the progress messages (prio, pattern).
/// Purchases, chest claims and potions are routine, not successes (user 2026-10-07).
const WINS: &[(u8, &str)] = &[(4, "(epic)"), (3, "[guild] Joined"), (2, "[dungeons] Win"), (2, "[hunt] Win")];

/// Issues and questions for the user, recognised in the progress messages.
const ISSUES: &[&str] = &[
    "Error", "error", "MISMATCH", "unmapped", "Unknown", "failed", "did not", "could not", "Could not",
    "not whitelisted", "!!!", "limit", "full", "verify", "No ", "not supported",
];

/// Is this progress message a success (`WIN`, prio) or an issue (`ISSUE`)?
fn classify(msg: &str) -> Option<(&'static str, u8)> {
    if msg.starts_with("[control]") || msg.starts_with("[roster]") {
        return None;
    }
    // An epic item counts only when it gets equipped
    if let Some((prio, _)) = WINS.iter().find(|(_, p)| msg.contains(p))
        && !(msg.contains("(epic)") && !msg.contains("equipping"))
    {
        return Some(("WIN", *prio));
    }
    ISSUES.iter().any(|p| msg.contains(p)).then_some(("ISSUE", 0))
}

/// Called for every progress message: notes successes and issues.
pub fn observe(msg: &str) {
    if let Some((kind, prio)) = classify(msg) {
        note(kind, prio, msg);
    }
}

fn report_time(date: NaiveDate) -> chrono::DateTime<Local> {
    let t = NaiveTime::from_hms_opt(REPORT_HOUR, REPORT_MINUTE, 0).unwrap_or_default();
    date.and_time(t).and_local_timezone(Local).earliest().unwrap_or_else(Local::now)
}

fn day_file(date: NaiveDate) -> Option<PathBuf> {
    Some(dir()?.join("days").join(format!("{date}.final")))
}

/// The most recent date this character already has a finalized report for, if any (scans `days/*.final`).
fn last_final_date(d: &Path) -> Option<NaiveDate> {
    let mut best: Option<NaiveDate> = None;
    for e in fs::read_dir(d.join("days")).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(date) = name.strip_suffix(".final").and_then(|s| s.parse::<NaiveDate>().ok()) {
            best = Some(best.map_or(date, |b| b.max(date)));
        }
    }
    best
}

/// Dates (oldest first) whose daily report is overdue and not yet written: every date between the day after
/// the last finalized one and yesterday (always overdue, regardless of time of day), plus today once its
/// report time has passed. Catches up a day the bot was switched off for entirely (e.g. turned back on the
/// next morning, user 2026-10-09): since nothing happens to the character while the bot isn't running, today's
/// otherwise-unchanged state IS that missed day's end-of-day state, so it's safe to backdate it rather than
/// just lose the data point.
pub fn overdue_days() -> Vec<NaiveDate> {
    let Some(d) = dir() else { return Vec::new() };
    let today = Local::now().date_naive();
    let mut day = last_final_date(&d).map_or(today, |d| d + chrono::Duration::days(1));
    let mut out = Vec::new();
    while day < today {
        out.push(day);
        day += chrono::Duration::days(1);
    }
    if Local::now() >= report_time(today) && day_file(today).is_some_and(|f| !f.exists()) {
        out.push(today);
    }
    out
}

/// Seconds until today's report (for the main loop's wait). None when already written / past.
pub fn secs_until_due() -> Option<u64> {
    let now = Local::now();
    let at = report_time(now.date_naive());
    (now < at).then(|| u64::try_from((at - now).num_seconds()).unwrap_or(0) + 5)
}

/// Notes since the last final report; the final report also empties the notes file (a preview keeps them).
fn take_notes(empty: bool) -> Vec<(String, String, u8, String)> {
    let Some(d) = dir() else { return Vec::new() };
    let path = d.join("notes.log");
    let text = fs::read_to_string(&path).unwrap_or_default();
    if empty {
        let _ = fs::write(&path, "");
    }
    text.lines()
        .filter_map(|l| {
            let mut p = l.splitn(4, '\t');
            Some((p.next()?.to_string(), p.next()?.to_string(), p.next()?.parse().ok()?, p.next()?.to_string()))
        })
        .collect()
}

/// Today's numbers (also one line of history.csv).
struct Snapshot {
    date: NaiveDate,
    class: String,
    level: u16,
    rank: u32,
    honor: u32,
    strength: f64,
    gold: f64,
    mushrooms: u32,
    lucky_coins: u32,
    hourglasses: u32,
    arena_wins: u8,
    guild: String,
    /// Gold / mushrooms gained today (income only)
    gold_gained: f64,
    mushrooms_gained: u64,
    xp_gained: u64,
    /// Dungeon enemies defeated so far (all dungeons)
    dungeons: u32,
}

const CSV_HEADER: &str = "date,level,rank,honor,strength,gold,mushrooms,lucky_coins,hourglasses,arena_wins,guild,\
gold_gained,mushrooms_gained,xp_gained,dungeons,class";

/// Dungeon enemies defeated so far (a finished dungeon counts as 10).
fn dungeons_defeated(gs: &GameState) -> u32 {
    let count = |p: &DungeonProgress| match p {
        DungeonProgress::Open { finished } => u32::from(*finished),
        DungeonProgress::Finished => 10,
        DungeonProgress::Locked => 0,
    };
    gs.dungeons.light.values().map(count).sum::<u32>() + gs.dungeons.shadow.values().map(count).sum::<u32>()
}

impl Snapshot {
    fn of(gs: &GameState, gold_gained: f64, mushrooms_gained: u64, xp_gained: u64) -> Self {
        let c = &gs.character;
        Snapshot {
            date: Local::now().date_naive(),
            class: format!("{:?}", c.class),
            level: c.level,
            rank: c.rank,
            honor: c.honor,
            strength: crate::hunt::own_strength(gs),
            gold: c.silver as f64 / 100.0,
            mushrooms: c.mushrooms,
            lucky_coins: gs.specials.wheel.lucky_coins,
            hourglasses: gs.tavern.quicksand_glasses,
            arena_wins: gs.arena.fights_for_xp,
            guild: gs.guild.as_ref().map_or_else(|| "-".to_string(), |g| g.name.replace(',', " ")),
            gold_gained,
            mushrooms_gained,
            xp_gained,
            dungeons: dungeons_defeated(gs),
        }
    }

    fn csv(&self) -> String {
        format!(
            "{},{},{},{},{:.0},{:.2},{},{},{},{},{},{:.2},{},{},{},{}",
            self.date,
            self.level,
            self.rank,
            self.honor,
            self.strength,
            self.gold,
            self.mushrooms,
            self.lucky_coins,
            self.hourglasses,
            self.arena_wins,
            self.guild,
            self.gold_gained,
            self.mushrooms_gained,
            self.xp_gained,
            self.dungeons,
            self.class
        )
    }
}

/// The day before the challenge's Day 1 (same inference as `tournament::start_date()`), used to backdate every
/// character's "Day 0" baseline so they all line up on the same point on the dashboard regardless of when each is
/// actually handed to the bot. For a brand new roster this is just "today − 1 day"; that first baseline row then
/// becomes the earliest `history.csv` date, so it is what every later character's Day 0 lines up with too.
pub fn day0_date() -> NaiveDate {
    let today = Local::now().date_naive();
    crate::tournament::start_date().unwrap_or(today) - chrono::Duration::days(1)
}

/// This character's class (`history.csv`'s `class` column, e.g. "BattleMage") and Day 0 snapshot
/// (`days/<day0_date>.json`: level/base attributes/equipped items), if it has one – lets
/// `tournament::run_day0` build a synthetic fighter from the frozen Day 0 data instead of a live, possibly
/// already-leveled-up `ViewPlayer` snapshot.
pub fn day0_snapshot(nick: &str) -> Option<(String, serde_json::Value)> {
    let dir = Path::new(ROOT).join(nick);
    let date = day0_date().to_string();
    let rows = read_history(&dir.join("history.csv"));
    let row = rows.iter().find(|r| r.get("date").map(String::as_str) == Some(date.as_str()))?;
    let class = row.get("class")?.clone();
    let text = fs::read_to_string(dir.join("days").join(format!("{date}.json"))).ok()?;
    Some((class, serde_json::from_str(&text).ok()?))
}

/// Above this level a character is no longer "fresh" (tutorial just done) – the guard that keeps a character
/// which already leveled up before its first bot run from getting a falsely low "Day 0" snapshot (see Chlamydie,
/// user 2026-10-08: leveled from 2 to 13+ within the first run, before any report was ever written).
const DAY0_MAX_LEVEL: u16 = 3;

/// One-off "Day 0" baseline for a character handed to the bot for the first time (right after the tutorial,
/// still near level 2): backdated to `day0_date()` so every character's chart starts at the same point, no
/// matter when it is actually added. Does nothing once the character has its first real report, or if it is
/// already past `DAY0_MAX_LEVEL` (catches the baseline immediately, before dungeons/Tavern/Arena can level the
/// character past it). Returns the backdated date used, so the caller can also run the Day 0 tournament round.
pub fn write_day0(gs: &GameState) -> Option<NaiveDate> {
    let d = dir()?;
    if d.join("history.csv").exists() {
        return None;
    }
    if gs.character.level > DAY0_MAX_LEVEL {
        report!(
            "[roster] Day 0 baseline skipped: already level {} (past the hand-off point) \
             – if you have older data from before the bot started, ask Claude to backfill it manually",
            gs.character.level
        );
        return None;
    }
    let date = day0_date();
    let mut s = Snapshot::of(gs, 0.0, 0, 0);
    s.date = date;
    let _ = fs::create_dir_all(d.join("days"));
    let _ = fs::write(d.join("history.csv"), format!("{CSV_HEADER}\n{}\n", s.csv()));
    let _ = fs::write(d.join("days").join(format!("{date}.json")), snapshot(gs).to_string());
    write_shared(date);
    report!("[roster] Day 0 baseline captured ({date}): level {}, rank {}", s.level, s.rank);
    Some(date)
}

/// Writes today's report for the logged-in character and refreshes the shared files. Returns a summary line.
/// (user 2026-10-07: only the dashboard is wanted – no character card / daily HTML pages.)
///
/// `fin` = the real end-of-day report (23:50). A manual one from the icon menu is a preview: it writes the same files
/// (today's history line is replaced, not added) but keeps the notes and does not count as the day's report.
///
/// `date` is normally today, but a date in the past is how `overdue_days()` backfills a day the bot was off for
/// entirely (always written as final, never a preview) – gains for that date come out as 0 (accurate: nothing
/// happened to the character while the bot wasn't running).
pub fn write_day(gs: &GameState, date: NaiveDate, fin: bool) -> String {
    let Some(d) = dir() else { return "no character set".to_string() };
    let notes = take_notes(fin);
    let sum = |kind: &str| notes.iter().filter(|n| n.1 == kind).filter_map(|n| n.3.parse::<u64>().ok()).sum::<u64>();
    let mut s = Snapshot::of(gs, sum("GOLD") as f64 / 100.0, sum("MUSH"), sum("XP"));
    s.date = date;
    let history = d.join("history.csv");

    // Biggest success + issues grouped by message (with a count and the first time) → days/<date>.issues
    let best = notes.iter().filter(|n| n.1 == "WIN").max_by_key(|n| n.2).map(|n| n.3.clone());
    let mut issues: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for n in notes.iter().filter(|n| n.1 == "ISSUE") {
        issues.entry(n.3.clone()).or_insert((0, n.0[11..16].to_string())).0 += 1;
    }
    let mut lines: Vec<String> = best.map(|b| format!("Biggest success: {b}")).into_iter().collect();
    lines.extend(issues.iter().map(|(m, (c, t))| if *c > 1 { format!("{t} {m} ({c}×)") } else { format!("{t} {m}") }));
    let days = d.join("days");
    let _ = fs::create_dir_all(&days);
    let _ = fs::write(days.join(format!("{}.issues", s.date)), lines.join("\n"));
    if fin && let Some(f) = day_file(s.date) {
        let _ = fs::write(f, "");
    }
    // One line per date: a preview from earlier today is replaced
    let old = fs::read_to_string(&history).unwrap_or_default();
    let mut rows: Vec<&str> = old.lines().skip(1).filter(|l| !l.starts_with(&s.date.to_string())).collect();
    let line = s.csv();
    rows.push(&line);
    let _ = fs::write(&history, format!("{CSV_HEADER}\n{}\n", rows.join("\n")));
    // Snapshot of the character for "what changed" on the dashboard (Win rate tab)
    let _ = fs::write(d.join("days").join(format!("{}.json", s.date)), snapshot(gs).to_string());
    write_shared(s.date);
    format!("level {}, Hall of Fame rank {}", s.level, s.rank)
}

const ATTRS: [(AttributeType, &str); 5] = [
    (AttributeType::Strength, "STR"),
    (AttributeType::Dexterity, "DEX"),
    (AttributeType::Intelligence, "INT"),
    (AttributeType::Constitution, "CON"),
    (AttributeType::Luck, "LCK"),
];

const SLOTS: [(EquipmentSlot, &str); 10] = [
    (EquipmentSlot::Weapon, "Weapon"),
    (EquipmentSlot::Shield, "Shield"),
    (EquipmentSlot::Hat, "Helmet"),
    (EquipmentSlot::BreastPlate, "Chest plate"),
    (EquipmentSlot::Gloves, "Gloves"),
    (EquipmentSlot::FootWear, "Boots"),
    (EquipmentSlot::Amulet, "Amulet"),
    (EquipmentSlot::Belt, "Belt"),
    (EquipmentSlot::Ring, "Ring"),
    (EquipmentSlot::Talisman, "Talisman"),
];

/// Short item description: "15–41 dmg, STR +9".
fn item_desc(i: &Item) -> String {
    let mut parts = Vec::new();
    if let ItemType::Weapon { min_dmg, max_dmg } = i.typ {
        parts.push(format!("{min_dmg}–{max_dmg} dmg"));
    }
    for (a, n) in ATTRS {
        if i.attributes[a] > 0 {
            parts.push(format!("{n} +{}", i.attributes[a]));
        }
    }
    parts.join(", ")
}

/// Filename of this item's sprite under `roster/assets/items/` (vendored, local-only, see
/// `docs/precedents.md`): `{type}_{model}_{color}_{class}.png`, the same naming the game's own web client
/// uses for its item sprites, reverse-engineered from the (independently open-source) `sf-tools` project.
/// `color` comes straight from `Item.color` (`sf-api` already derives it with the same 1-indexed
/// convention); the class variant comes from `Item.class` (only set for class-restricted slots) + 1, since
/// `Class::Warrior == 0` but the sprite naming is 1-indexed. Non-class items and items of raw type ≥ 10
/// (talismans and beyond) always use variant/class 1.
fn item_icon(i: &Item) -> String {
    let typ = i.typ.raw_id();
    if typ >= 10 {
        format!("assets/items/{typ}_{}_1_1.png", i.model_id)
    } else {
        let class = match i.class {
            Some(Class::Warrior) => 1,
            Some(Class::Mage) => 2,
            Some(Class::Scout) => 3,
            _ => 1,
        };
        format!("assets/items/{typ}_{}_{}_{class}.png", i.model_id, i.color)
    }
}

/// A potion for the character card: its sprite (`12_<id>_1_1.png`, where the id is the inverse of sf-api's
/// `PotionType::parse`/`PotionSize::parse`: size 0/5/10 + attribute 1 STR, 2 DEX, 3 INT, 4 CON, 5 LCK; Eternal Life 16),
/// a label and how long it still lasts (`until_ts` for a live countdown, `left_sec` as of this write for snapshots).
fn potion_json(p: &sf_api::gamestate::items::Potion) -> serde_json::Value {
    use sf_api::gamestate::items::{PotionSize, PotionType};
    let id = match p.typ {
        PotionType::EternalLife => 16,
        t => {
            let base = match p.size {
                PotionSize::Small => 0,
                PotionSize::Medium => 5,
                PotionSize::Large => 10,
            };
            base + match t {
                PotionType::Strength => 1,
                PotionType::Dexterity => 2,
                PotionType::Intelligence => 3,
                PotionType::Constitution => 4,
                _ => 5,
            }
        }
    };
    serde_json::json!({
        "t": format!("{:?} {:.0} %", p.typ, p.size.effect() * 100.0),
        "icon": format!("assets/items/12_{id}_1_1.png"),
        "until_ts": p.expires.map(|e| e.to_rfc3339()),
        "left_sec": p.expires.map(|e| (e - Local::now()).num_seconds().max(0)),
    })
}

/// The derived values the game's character screen shows (damage, hit points, crit chance, armor), computed with
/// sf-api's own simulator formulas (`UpgradeableFighter::hit_points`, `damage.rs`); crit chance against an enemy
/// of our own level, like the game's tooltip. Damage is the average weapon hit × the main attribute bonus (the
/// game's "~" figure, before the enemy's armor) × the game's per-class factor (see below).
/// Armor cap and crit cap: armor reduction is capped per class (`max_armor_reduction`, Paladin 45 %), crit at 50 %.
fn derived(gs: &GameState) -> serde_json::Value {
    use sf_api::simulate::{Fighter, PlayerFighterSquad};
    let f = PlayerFighterSquad::new(gs).character;
    let attrs = f.attributes();
    let fighter = Fighter::from(&f);
    let level = f64::from(f.level.max(1));
    let main = f.class.main_attribute();
    // Unarmed damage, used when a hand holds no weapon or a weaker one (sf-api `get_hand_damage`)
    let hand = |second: bool| {
        let m = if f.class == Class::Assassin { if second { 1.25 } else { 0.875 } } else { 0.7 };
        let d = m * (level - 9.0) * f.class.weapon_multiplier();
        if f.level <= 10 { (1.0, 2.0) } else { ((d * 2.0 / 3.0).ceil().max(1.0), (d * 4.0 / 3.0).round().max(2.0)) }
    };
    let avg = |w: &Option<sf_api::simulate::Weapon>, second: bool| {
        let h = hand(second);
        let (min, max) = match w {
            Some(w) if !(w.damage.min < h.0 && w.damage.max < h.1) => (w.damage.min, w.damage.max),
            _ => h,
        };
        (min + max) / 2.0
    };
    // The game's own per-class factor on the character screen (user 2026-10-09, differs from sf-api's
    // `damage_multiplier`, which is about the simulator's per-hit scaling): weapon × factor × (1 + main/10);
    // the Assassin adds both hands first.
    let (weapon, factor) = match f.class {
        Class::Assassin => (avg(&fighter.first_weapon, false) + avg(&fighter.second_weapon, true), 0.625),
        c => (
            avg(&fighter.first_weapon, false),
            match c {
                Class::Berserker | Class::DemonHunter => 1.25,
                Class::Paladin | Class::Mage => 0.83,
                Class::Necromancer => 0.56,
                Class::Bard => 1.125,
                Class::Druid => 0.33,
                _ => 1.0,
            },
        ),
    };
    let damage = weapon * factor * (1.0 + f64::from(attrs[main]) / 10.0);
    let reduction = (f.class.armor_multiplier() * f64::from(fighter.armor) / level / 100.0)
        .min(f64::from(f.class.max_armor_reduction()) / 100.0);
    let crit = (f64::from(attrs[AttributeType::Luck]) * 5.0 / (level * 2.0)).min(50.0);
    serde_json::json!({
        "main": ATTRS.iter().find(|(a, _)| *a == main).map(|(_, n)| *n),
        "damage": damage.round(),
        "hp": f.hit_points(&attrs),
        "crit_pct": (crit * 100.0).round() / 100.0,
        "armor": fighter.armor,
        "reduction_pct": (reduction * 1000.0).round() / 10.0,
    })
}

/// What the dashboard compares day to day: level, bought attributes, equipment, potions, guild.
fn snapshot(gs: &GameState) -> serde_json::Value {
    let c = &gs.character;
    let attrs: serde_json::Map<String, serde_json::Value> =
        ATTRS.iter().map(|(a, n)| ((*n).to_string(), c.attribute_basis[*a].into())).collect();
    // Base + equipment/potion bonus, for the dashboard's end-of-day stat card (the bare `attrs` above stays
    // basis-only, as `tournament::fighter_from_day0` relies on that to build an unequipped Day 0 fighter).
    let attrs_total: serde_json::Map<String, serde_json::Value> = ATTRS
        .iter()
        .map(|(a, n)| {
            let (b, add) = (c.attribute_basis[*a], c.attribute_additions[*a]);
            ((*n).to_string(), serde_json::json!({ "base": b, "bonus": add, "total": b + add }))
        })
        .collect();
    let equip: serde_json::Map<String, serde_json::Value> = SLOTS
        .iter()
        .filter_map(|(slot, n)| {
            let i = c.equipment.0[*slot].as_ref()?;
            Some((
                (*n).to_string(),
                serde_json::json!({
                    "d": item_desc(i),
                    "v": (crate::inventory::value(gs, i) * 10.0).round() / 10.0,
                    "epic": i.is_epic(),
                    "legendary": i.is_legendary(),
                    "icon": item_icon(i),
                }),
            ))
        })
        .collect();
    let potions: Vec<String> = c
        .active_potions
        .iter()
        .flatten()
        .map(|p| format!("{:?} {:.0} %", p.typ, p.size.effect() * 100.0))
        .collect();
    serde_json::json!({
        "level": c.level,
        "attrs": attrs,
        "attrs_total": attrs_total,
        "equip": equip,
        "potions": potions,
        "potion_items": c.active_potions.iter().flatten().map(potion_json).collect::<Vec<_>>(),
        "derived": derived(gs),
        "guild": gs.guild.as_ref().map(|g| g.name.clone()),
    })
}

/// The character's current state for the dashboard card (user 2026-10-08).
/// Thirst for Adventure (ALU) at full: `sf-api` itself checks against this constant for "full day" (100 min).
const MAX_THIRST_SEC: u32 = 6000;

/// What the character is doing right now, for the app window's tile: a short label and, when known,
/// the time it ends (so the tile can show a countdown).
fn activity(gs: &GameState) -> serde_json::Value {
    let until = |t: chrono::DateTime<Local>| t.to_rfc3339();
    match gs.tavern.current_action {
        CurrentAction::Idle => serde_json::json!({ "label": "Idle" }),
        CurrentAction::CityGuard { busy_until, .. } => {
            serde_json::json!({ "label": "City Guard", "until": until(busy_until) })
        }
        CurrentAction::Expedition => {
            let stage = gs.tavern.expeditions.active().map(sf_api::gamestate::tavern::Expedition::current_stage);
            match stage {
                Some(ExpeditionStage::Waiting { busy_until, .. }) => {
                    serde_json::json!({ "label": "Expedition (waiting)", "until": until(busy_until) })
                }
                Some(ExpeditionStage::Encounters(_)) => serde_json::json!({ "label": "Expedition (choosing)" }),
                Some(ExpeditionStage::Boss(_)) => serde_json::json!({ "label": "Expedition (boss)" }),
                Some(ExpeditionStage::Rewards(_)) => serde_json::json!({ "label": "Expedition (reward)" }),
                _ => serde_json::json!({ "label": "Expedition" }),
            }
        }
        CurrentAction::Quest { busy_until, .. } => serde_json::json!({ "label": "Quest", "until": until(busy_until) }),
        CurrentAction::Unknown(_) => serde_json::json!({ "label": "Unknown" }),
    }
}

fn card_data(gs: &GameState) -> serde_json::Value {
    let c = &gs.character;
    let attrs: serde_json::Map<String, serde_json::Value> = ATTRS
        .iter()
        .map(|(a, n)| {
            let (b, add) = (c.attribute_basis[*a], c.attribute_additions[*a]);
            ((*n).to_string(), serde_json::json!({ "base": b, "bonus": add, "total": b + add }))
        })
        .collect();
    let potions: Vec<serde_json::Value> = c
        .active_potions
        .iter()
        .flatten()
        .map(|p| {
            let mut j = potion_json(p);
            j["until"] = p.expires.map(|e| e.format("%d.%m. %H:%M").to_string()).into();
            j
        })
        .collect();
    let dungeons: Vec<serde_json::Value> = gs
        .dungeons
        .light
        .iter()
        .map(|(d, p)| (format!("{d:?}"), p))
        .chain(gs.dungeons.shadow.iter().map(|(d, p)| (format!("Shadow {d:?}"), p)))
        .filter_map(|(name, p)| match p {
            DungeonProgress::Open { finished } => Some(serde_json::json!({ "name": name, "done": finished, "finished": false })),
            DungeonProgress::Finished => Some(serde_json::json!({ "name": name, "done": 10, "finished": true })),
            DungeonProgress::Locked => None,
        })
        .collect();
    let scrapbook = c.scrapbook.as_ref().map(|s| s.items.len() + s.monster.len());
    let ach = &gs.achievements;
    serde_json::json!({
        "name": c.name,
        "class": format!("{:?}", c.class),
        "level": c.level,
        "xp": c.experience,
        "next_xp": c.next_level_xp,
        "rank": c.rank,
        "honor": c.honor,
        "gold": c.silver as f64 / 100.0,
        "mushrooms": c.mushrooms,
        "lucky_coins": gs.specials.wheel.lucky_coins,
        "hourglasses": gs.tavern.quicksand_glasses,
        "strength": (crate::hunt::own_strength(gs)).round(),
        "attrs": attrs,
        "potions": potions,
        "derived": derived(gs),
        "scrapbook": scrapbook,
        "achievements": { "owned": ach.owned(), "total": ach.0.len() },
        "dungeons": dungeons,
        "guild": gs.guild.as_ref().map(|g| g.name.clone()),
        "equip": snapshot(gs)["equip"].clone(),
        "updated": Local::now().format("%d.%m. %H:%M").to_string(),
        "thirst_sec": gs.tavern.thirst_for_adventure_sec,
        "thirst_max_sec": MAX_THIRST_SEC,
        "activity": activity(gs),
    })
}

/// Writes the character's current state (`now.json`) and rebuilds the dashboard, at most every `NOW_EVERY`.
pub fn write_now(gs: &GameState) {
    const NOW_EVERY: std::time::Duration = std::time::Duration::from_secs(10 * 60);
    static LAST: crate::ctx::PerChar<Option<std::time::Instant>> = crate::ctx::PerChar::new();
    let Ok(mut last) = LAST.lock() else { return };
    if last.is_some_and(|t| t.elapsed() < NOW_EVERY) {
        return;
    }
    *last = Some(std::time::Instant::now());
    drop(last);
    let Some(d) = dir() else { return };
    let _ = fs::create_dir_all(&d);
    let _ = fs::write(d.join("now.json"), card_data(gs).to_string());

    // The dashboard rebuild scans every character's folder (O(all characters)); only actually do it once per
    // `NOW_EVERY` window for the whole process, not once per character on its own independent timer – with ~10
    // characters all ticking roughly every 10 min, that could otherwise fire close to once a minute (user
    // 2026-10-08: noticeably slower PC after starting ~10 characters at once).
    static LAST_DASHBOARD: Mutex<Option<std::time::Instant>> = Mutex::new(None);
    let Ok(mut last_dashboard) = LAST_DASHBOARD.lock() else { return };
    if last_dashboard.is_some_and(|t| t.elapsed() < NOW_EVERY) {
        return;
    }
    *last_dashboard = Some(std::time::Instant::now());
    drop(last_dashboard);
    write_dashboard(false);
}

/// Human-readable changes between two snapshots (the reasons a win rate could jump).
fn changes(prev: &serde_json::Value, cur: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let (l0, l1) = (prev["level"].as_u64().unwrap_or(0), cur["level"].as_u64().unwrap_or(0));
    if l1 > l0 {
        out.push(format!("Level {l0} → {l1}"));
    }
    let bought: Vec<String> = ATTRS
        .iter()
        .filter_map(|(_, n)| {
            let d = cur["attrs"][*n].as_i64().unwrap_or(0) - prev["attrs"][*n].as_i64().unwrap_or(0);
            (d > 0).then(|| format!("{n} +{d}"))
        })
        .collect();
    if !bought.is_empty() {
        out.push(format!("Attributes bought: {}", bought.join(", ")));
    }
    for (_, slot) in SLOTS {
        let (a, b) = (&prev["equip"][slot], &cur["equip"][slot]);
        if b.is_null() || a["d"] == b["d"] {
            continue;
        }
        let rarity = if b["legendary"].as_bool() == Some(true) {
            "legendary "
        } else if b["epic"].as_bool() == Some(true) {
            "epic "
        } else {
            ""
        };
        let value = match (a["v"].as_f64(), b["v"].as_f64()) {
            (Some(x), Some(y)) => format!(" (value {x} → {y})"),
            (None, Some(y)) => format!(" (value {y})"),
            _ => String::new(),
        };
        out.push(format!("New {rarity}{}: {}{value}", slot.to_lowercase(), b["d"].as_str().unwrap_or("?")));
    }
    let had: Vec<&str> = prev["potions"].as_array().map_or_else(Vec::new, |v| v.iter().filter_map(|p| p.as_str()).collect());
    for p in cur["potions"].as_array().into_iter().flatten().filter_map(|p| p.as_str()) {
        if !had.contains(&p) {
            out.push(format!("Potion {p}"));
        }
    }
    if cur["guild"] != prev["guild"]
        && let Some(g) = cur["guild"].as_str()
    {
        out.push(format!("Joined the guild {g}"));
    }
    out
}

/// All of a character's daily snapshots (`days/<date>.json`), sorted by date.
fn read_days(char_dir: &Path) -> Vec<(String, serde_json::Value)> {
    let mut snaps: Vec<(String, serde_json::Value)> = fs::read_dir(char_dir.join("days"))
        .map(|it| {
            it.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter_map(|e| {
                    let date = e.path().file_stem()?.to_string_lossy().to_string();
                    Some((date, serde_json::from_str(&fs::read_to_string(e.path()).ok()?).ok()?))
                })
                .collect()
        })
        .unwrap_or_default();
    snaps.sort_by(|a, b| a.0.cmp(&b.0));
    snaps
}

/// Changes per date for one character, from its daily snapshots (`days/<date>.json`).
fn daily_changes(days: &[(String, serde_json::Value)]) -> BTreeMap<String, Vec<String>> {
    days.windows(2).map(|w| (w[1].0.clone(), changes(&w[0].1, &w[1].1))).collect()
}

/// `roster/issues.txt` and `roster/leaderboard.md` from all characters' folders.
fn write_shared(date: NaiveDate) {
    let Ok(entries) = fs::read_dir(ROOT) else { return };
    let mut issues = format!("Issues and questions – {date}\n\n");
    let mut rows = Vec::new();
    // Folders starting with '_' are demos/tests: only included in a demo report
    let demo = dir().is_some_and(|d| d.file_name().is_some_and(|n| n.to_string_lossy().starts_with('_')));
    for e in entries
        .flatten()
        .filter(|e| e.path().is_dir() && (demo || !e.file_name().to_string_lossy().starts_with('_')))
    {
        let nick = e.file_name().to_string_lossy().to_string();
        if let Ok(text) = fs::read_to_string(e.path().join("days").join(format!("{date}.issues"))) {
            let body = if text.trim().is_empty() { "–".to_string() } else { text.trim().to_string() };
            issues += &format!("== {nick} ==\n{body}\n\n");
        }
        if let Ok(h) = fs::read_to_string(e.path().join("history.csv"))
            && let Some(last) = h.lines().skip(1).last()
        {
            rows.push((nick, last.to_string()));
        }
    }
    let _ = fs::write(Path::new(ROOT).join("issues.txt"), issues);

    // Leaderboard by level, then Hall of Fame rank
    let num = |l: &str, i: usize| l.split(',').nth(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    rows.sort_by(|a, b| num(&b.1, 1).total_cmp(&num(&a.1, 1)).then(num(&a.1, 2).total_cmp(&num(&b.1, 2))));
    let mut lb = format!("# Leaderboard – {date}\n\n| # | Character | Date | Level | HoF rank | Honor | Power | Gold |\n|---|---|---|---|---|---|---|---|\n");
    for (i, (nick, l)) in rows.iter().enumerate() {
        let f: Vec<&str> = l.split(',').collect();
        let g = |i: usize| f.get(i).copied().unwrap_or("?");
        lb += &format!("| {} | {nick} | {} | {} | {} | {} | {} | {} |\n", i + 1, g(0), g(1), g(2), g(3), g(4), g(5));
    }
    let _ = fs::write(Path::new(ROOT).join("leaderboard.md"), lb);
    write_dashboard(demo);
}

/// One character's history: date → column → value.
fn read_history(path: &Path) -> Vec<BTreeMap<String, String>> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut lines = text.lines();
    let Some(header) = lines.next() else { return Vec::new() };
    let cols: Vec<&str> = header.split(',').collect();
    lines
        .map(|l| cols.iter().zip(l.split(',')).map(|(c, v)| ((*c).to_string(), v.to_string())).collect())
        .collect()
}

/// `roster/dashboard.html`: all characters in one chart, metric tabs, day stepper (src/dashboard.html + data).
fn write_dashboard(demo: bool) {
    let Ok(entries) = fs::read_dir(ROOT) else { return };
    let mut dates = std::collections::BTreeSet::new();
    let mut chars = Vec::new();
    let mut dirs: Vec<_> = entries
        .flatten()
        .filter(|e| e.path().is_dir() && (demo || !e.file_name().to_string_lossy().starts_with('_')))
        .collect();
    dirs.sort_by_key(|e| e.file_name());
    // Daily simulated round robin: average win rate per character (src/tournament.rs)
    let win_rates = crate::tournament::daily_win_rates(demo);
    for e in dirs {
        let rows = read_history(&e.path().join("history.csv"));
        if rows.is_empty() && !e.path().join("now.json").exists() {
            continue;
        }
        let num = |r: &BTreeMap<String, String>, k: &str| r.get(k).and_then(|v| v.parse::<f64>().ok());
        let (mut gold, mut xp, mut mush, mut arena_wins) = (0.0, 0.0, 0.0, 0.0);
        let mut by_date = serde_json::Map::new();
        let mut class = String::new();
        let nick = e.file_name().to_string_lossy().trim_start_matches("_demo_").to_string();
        let days = read_days(&e.path());
        let changes = daily_changes(&days);
        let snap_by_date: BTreeMap<&str, &serde_json::Value> = days.iter().map(|(date, snap)| (date.as_str(), snap)).collect();
        for r in &rows {
            let Some(date) = r.get("date") else { continue };
            gold += num(r, "gold_gained").unwrap_or(0.0);
            xp += num(r, "xp_gained").unwrap_or(0.0);
            mush += num(r, "mushrooms_gained").unwrap_or(0.0);
            arena_wins += num(r, "arena_wins").unwrap_or(0.0);
            if let Some(c) = r.get("class") {
                class.clone_from(c);
            }
            dates.insert(date.clone());
            let snap = snap_by_date.get(date.as_str()).copied();
            let get = |field: &str| snap.map_or(serde_json::Value::Null, |s| s[field].clone());
            by_date.insert(
                date.clone(),
                serde_json::json!({
                    "gold": gold, "xp": xp, "mushrooms": mush, "arena_wins_total": arena_wins,
                    "dungeons": num(r, "dungeons"), "rank": num(r, "rank"), "strength": num(r, "strength"),
                    "winrate": win_rates.get(date).and_then(|d| d.get(&nick)).map(|w| w * 100.0),
                    "changes": changes.get(date).cloned().unwrap_or_default(),
                    "equip": get("equip"),
                    // End-of-day stat card (user 2026-10-09): everything below is what the character actually
                    // had *on this date*, not today's live state – e.g. Day 0 must show no equipment bonus.
                    "level": num(r, "level"), "honor": num(r, "honor"),
                    "gold_now": num(r, "gold"), "mushrooms_now": num(r, "mushrooms"), "lucky_coins_now": num(r, "lucky_coins"),
                    "attrs_total": get("attrs_total"), "potions_day": get("potions"), "potion_items": get("potion_items"), "derived": get("derived"), "guild_day": get("guild"),
                }),
            );
        }
        let now: serde_json::Value = fs::read_to_string(e.path().join("now.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or(serde_json::Value::Null);
        if class.is_empty()
            && let Some(c) = now["class"].as_str()
        {
            class = c.to_string();
        }
        chars.push(serde_json::json!({ "nick": nick, "cls": class, "rows": by_date, "now": now }));
    }
    let data = serde_json::json!({
        "generated": Local::now().format("%Y-%m-%d %H:%M").to_string(),
        "dates": dates.into_iter().collect::<Vec<_>>(),
        "chars": chars,
        "h2h": crate::tournament::head_to_head(demo),
    });
    let template = include_str!("dashboard.html");
    let start = "/*DATA*/";
    let end = "/*END*/";
    let (Some(a), Some(b)) = (template.find(start), template.find(end)) else { return };
    // `</` inside the data must not end the script tag
    let json = data.to_string().replace("</", "<\\/");
    let html = format!("{}{start}{json}{}", &template[..a], &template[b..]);
    let _ = fs::write(Path::new(ROOT).join("dashboard.html"), html);
}

/// Level-up detection for the "biggest success" (called every pass of the main loop).
pub fn track_level(gs: &GameState) {
    static LAST: crate::ctx::PerChar<Option<u16>> = crate::ctx::PerChar::new();
    let Ok(mut last) = LAST.lock() else { return };
    let level = gs.character.level;
    if let Some(prev) = *last
        && level > prev
    {
        win(5, &format!("Level up: {prev} → {level}"));
    }
    *last = Some(level);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Past days with no finalized report are always overdue regardless of time of day; today only once its
    /// own report time has passed – not asserted here (would be flaky depending on when the test runs).
    #[test]
    fn overdue_days_backfills_past_but_not_future() {
        let nick = "_test_overdue_roster";
        let d = Path::new(ROOT).join(nick);
        let _ = fs::remove_dir_all(&d);
        let _ = fs::create_dir_all(d.join("days"));
        set_character(nick);
        let today = Local::now().date_naive();
        let _ = fs::write(d.join("days").join(format!("{}.final", today - chrono::Duration::days(3))), "");
        let overdue = overdue_days();
        let _ = fs::remove_dir_all(&d);
        assert!(overdue.contains(&(today - chrono::Duration::days(2))));
        assert!(overdue.contains(&(today - chrono::Duration::days(1))));
        assert!(!overdue.contains(&(today - chrono::Duration::days(3))));
        assert!(!overdue.contains(&(today + chrono::Duration::days(1))));
    }

    #[test]
    fn changes_between_snapshots() {
        let prev = serde_json::json!({"level": 12, "attrs": {"STR": 28, "CON": 29},
            "equip": {"Weapon": {"d": "14–22 dmg", "v": 59.7, "epic": false}}, "potions": [], "guild": null});
        let cur = serde_json::json!({"level": 13, "attrs": {"STR": 33, "CON": 29},
            "equip": {"Weapon": {"d": "15–41 dmg, STR +9", "v": 99.0, "epic": true},
                      "Gloves": {"d": "STR +6", "v": 12.0, "epic": false}},
            "potions": ["Strength 25 %"], "guild": "Artušova Garda"});
        assert_eq!(
            changes(&prev, &cur),
            [
                "Level 12 → 13",
                "Attributes bought: STR +5",
                "New epic weapon: 15–41 dmg, STR +9 (value 59.7 → 99)",
                "New gloves: STR +6 (value 12)",
                "Potion Strength 25 %",
                "Joined the guild Artušova Garda",
            ]
        );
        assert!(changes(&cur, &cur).is_empty());
    }

    #[test]
    fn classifies_progress_messages() {
        assert_eq!(classify("[tasks] Claiming the daily chest 3 (12 points)"), None);
        assert_eq!(classify("[shops] Buying an upgrade: Belt …"), None);
        assert_eq!(classify("[inventory] equipping Weapon (epic) (value 50.0 > 30.0 of Weapon)"), Some(("WIN", 4)));
        assert_eq!(classify("[inventory] selling Hat (epic) …"), None);
        assert_eq!(classify("[arena] Error: server error"), Some(("ISSUE", 0)));
        assert_eq!(classify("[check] MISMATCH Cake"), Some(("ISSUE", 0)));
        assert_eq!(classify("[arena] Next free fight at 20:57:18"), None);
        assert_eq!(classify("[control] Bot started"), None);
    }

    /// Demo of the dashboard with made-up data for the roster (user 2026-10-07): `cargo test demo_dashboard -- --ignored`.
    #[test]
    #[ignore]
    fn demo_dashboard() {
        let roster = [
            ("Filminy", "Scout"),
            ("Mrožik", "Mage"),
            ("Sanek", "Assassin"),
            ("Wecros", "Berserker"),
            ("Květoš", "DemonHunter"),
            ("PajaRizz", "Bard"),
            ("Pjotr", "Necromancer"),
            ("Mimimimi11", "Paladin"),
            ("TestChar1", "Warrior"),
            ("Radek", "Druid"),
            ("Novotné", "PlagueDoctor"),
            ("Bacilek", "BattleMage"),
        ];
        let start = Local::now().date_naive() - chrono::Duration::days(6);
        let mut rng = fastrand::Rng::with_seed(7);
        for (i, (nick, class)) in roster.iter().enumerate() {
            let d = Path::new(ROOT).join(format!("_demo_{nick}"));
            let _ = fs::remove_dir_all(&d);
            let _ = fs::create_dir_all(&d);
            let mut text = format!("{CSV_HEADER}
");
            let (mut level, mut rank, mut strength, mut dungeons) = (1u32, 60_000u32 - i as u32 * 900, 20.0, 0u32);
            for day in 0..7 {
                let date = start + chrono::Duration::days(day);
                level += 2 + rng.u32(0..4);
                rank = rank.saturating_sub(4_000 + rng.u32(0..6_000));
                strength += 25.0 + rng.f64() * 30.0;
                dungeons += rng.u32(10..25);
                let gold = 40.0 + rng.f64() * 120.0;
                let xp = 2_000 + rng.u64(0..6_000) * (day as u64 + 1);
                let mush = rng.u64(0..4);
                text += &format!(
                    "{date},{level},{rank},0,{strength:.0},0,0,0,0,10,-,{gold:.2},{mush},{xp},{dungeons},{class}
"
                );
            }
            let _ = fs::write(d.join("history.csv"), text);
            // Daily snapshots with a few made-up upgrades
            let _ = fs::create_dir_all(d.join("days"));
            let mut weapon = (8u32, 14u32, 4u32);
            let mut snap = serde_json::json!({"level": 1, "attrs": {"STR": 10, "DEX": 10, "INT": 10, "CON": 10, "LCK": 10},
                "equip": {"Weapon": {"d": "8–14 dmg, STR +4", "v": 30.0, "epic": false, "legendary": false}},
                "potions": [], "guild": null});
            for day in 0..7 {
                let date = start + chrono::Duration::days(day);
                snap["level"] = (snap["level"].as_u64().unwrap_or(1) + 2 + rng.u64(0..3)).into();
                for a in ["STR", "CON"] {
                    let v = snap["attrs"][a].as_u64().unwrap_or(0) + rng.u64(0..6);
                    snap["attrs"][a] = v.into();
                }
                if rng.u8(0..3) == 0 {
                    weapon = (weapon.0 + 5, weapon.1 + 12, weapon.2 + 4);
                    let epic = rng.u8(0..4) == 0;
                    snap["equip"]["Weapon"] = serde_json::json!({"d": format!("{}–{} dmg, STR +{}", weapon.0, weapon.1, weapon.2),
                        "v": f64::from(weapon.0 + weapon.1) * 2.0, "epic": epic, "legendary": false});
                }
                if day == 2 {
                    snap["potions"] = serde_json::json!(["Strength 25 %"]);
                    snap["guild"] = "Artušova Garda".into();
                }
                let _ = fs::write(d.join("days").join(format!("{date}.json")), snap.to_string());
            }
            // Current state for the character card
            let now = serde_json::json!({
                "name": nick, "class": class, "level": level, "xp": 4100, "next_xp": 6900, "rank": rank, "honor": 300 + i * 40,
                "gold": 157.29 + i as f64 * 11.0, "mushrooms": 31 + i, "lucky_coins": 50, "hourglasses": 12 + i, "strength": strength.round(),
                "attrs": {"STR": {"base": 28, "bonus": 41, "total": 69}, "DEX": {"base": 10, "bonus": 6, "total": 16},
                          "INT": {"base": 9, "bonus": 3, "total": 12}, "CON": {"base": 29, "bonus": 37, "total": 66},
                          "LCK": {"base": 14, "bonus": 12, "total": 26}},
                "potions": [{"t": "Strength 25 %", "until": "10.10. 21:40"}, {"t": "Constitution 15 %", "until": "09.10. 08:10"}],
                "scrapbook": 85 + i * 7, "achievements": {"owned": 12 + i, "total": 284},
                "dungeons": [{"name": "TrainingCamp", "done": 10, "finished": true}, {"name": "DesecratedCatacombs", "done": 4, "finished": false}],
                "guild": "Artušova Garda", "equip": snap["equip"].clone(), "updated": "08.10. 00:20",
            });
            let _ = fs::write(d.join("now.json"), now.to_string());
        }
        write_dashboard(true);
        println!("demo dashboard: roster/dashboard.html");
    }
}

