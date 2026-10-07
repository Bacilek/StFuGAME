//! Character challenge report (user 2026-10-07). Everything goes to `roster/` (local only, gitignored):
//! - during the day `roster/<nick>/notes.log`: successes and issues, picked from the progress messages (`observe`)
//!   and explicit notes (`win`),
//! - every day at ~23:50 (`due`): `roster/<nick>/days/<date>.md` (stats, Hall of Fame rank, level, the biggest success,
//!   issues), `history.csv` (one line a day), `card.html` (character card instead of a screenshot),
//!   `roster/issues.txt` (issues and questions of all characters) and `roster/leaderboard.md` (all characters).
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
        dungeons::DungeonProgress,
        items::{EquipmentSlot, Item, ItemType},
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
    let nick = NICK.lock().ok()?.clone()?;
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
    Some(dir()?.join("days").join(format!("{date}.html")))
}

/// Is today's report due (after 23:50 and not written yet)?
pub fn due() -> bool {
    let now = Local::now();
    now >= report_time(now.date_naive()) && day_file(now.date_naive()).is_some_and(|f| !f.exists())
}

/// Seconds until today's report (for the main loop's wait). None when already written / past.
pub fn secs_until_due() -> Option<u64> {
    let now = Local::now();
    let at = report_time(now.date_naive());
    (now < at).then(|| u64::try_from((at - now).num_seconds()).unwrap_or(0) + 5)
}

/// Notes since the last report (and empties the notes file).
fn take_notes() -> Vec<(String, String, u8, String)> {
    let Some(d) = dir() else { return Vec::new() };
    let path = d.join("notes.log");
    let text = fs::read_to_string(&path).unwrap_or_default();
    let _ = fs::write(&path, "");
    text.lines()
        .filter_map(|l| {
            let mut p = l.splitn(4, '\t');
            Some((p.next()?.to_string(), p.next()?.to_string(), p.next()?.parse().ok()?, p.next()?.to_string()))
        })
        .collect()
}

const ATTRS: [AttributeType; 5] = [
    AttributeType::Strength,
    AttributeType::Dexterity,
    AttributeType::Intelligence,
    AttributeType::Constitution,
    AttributeType::Luck,
];

/// Today's numbers (also one line of history.csv).
struct Snapshot {
    date: NaiveDate,
    name: String,
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
            name: c.name.clone(),
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

/// What the earlier days in history.csv say.
#[derive(Default)]
struct Past {
    /// Yesterday's (last earlier line) level and rank
    last: Option<(u16, u32)>,
    /// Best (lowest) Hall of Fame rank so far
    best_rank: Option<u32>,
    /// Gold / mushrooms gained over all earlier days
    gold_total: f64,
    mushrooms_total: u64,
}

fn past(history: &Path, today: NaiveDate) -> Past {
    let text = fs::read_to_string(history).unwrap_or_default();
    let mut p = Past::default();
    for line in text.lines().skip(1).filter(|l| !l.starts_with(&today.to_string())) {
        let f: Vec<&str> = line.split(',').collect();
        let num = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok());
        if let (Some(l), Some(r)) = (num(1), num(2)) {
            p.last = Some((l as u16, r as u32));
            if r > 0.0 {
                p.best_rank = Some(p.best_rank.map_or(r as u32, |b| b.min(r as u32)));
            }
        }
        p.gold_total += num(11).unwrap_or(0.0);
        p.mushrooms_total += num(12).unwrap_or(0.0) as u64;
    }
    p
}

/// Writes today's report for the logged-in character and refreshes the shared files. Returns a summary line.
pub fn write_day(gs: &GameState) -> String {
    let Some(d) = dir() else { return "no character set".to_string() };
    let notes = take_notes();
    let sum = |kind: &str| notes.iter().filter(|n| n.1 == kind).filter_map(|n| n.3.parse::<u64>().ok()).sum::<u64>();
    let s = Snapshot::of(gs, sum("GOLD") as f64 / 100.0, sum("MUSH"), sum("XP"));
    let history = d.join("history.csv");
    let past = past(&history, s.date);

    let best = notes.iter().filter(|n| n.1 == "WIN").max_by_key(|n| n.2).map(|n| n.3.clone());
    let wins: Vec<String> = notes.iter().filter(|n| n.1 == "WIN").map(|n| format!("{} {}", &n.0[11..16], n.3)).collect();
    // Issues grouped by message (with a count and the first time)
    let mut issues: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for n in notes.iter().filter(|n| n.1 == "ISSUE") {
        issues.entry(n.3.clone()).or_insert((0, n.0[11..16].to_string())).0 += 1;
    }
    let issue_lines: Vec<String> = issues
        .iter()
        .map(|(m, (c, t))| if *c > 1 { format!("{t} {m} ({c}×)") } else { format!("{t} {m}") })
        .collect();

    let html = page(gs, &s, &past, best.as_deref(), &wins, &issue_lines);
    if let Some(f) = day_file(s.date) {
        let _ = fs::create_dir_all(f.parent().unwrap_or(Path::new(ROOT)));
        let _ = fs::write(&f, &html);
        let _ = fs::write(f.with_extension("issues"), issue_lines.join("\n"));
    }
    let _ = fs::write(d.join("card.html"), &html);
    if !history.exists() {
        append(&history, CSV_HEADER);
    }
    append(&history, &s.csv());
    write_shared(s.date);
    format!("level {}, Hall of Fame rank {}", s.level, s.rank)
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
    let mut lb = format!("# Leaderboard – {date}\n\n| # | Character | Date | Level | HoF rank | Honor | Strength | Gold |\n|---|---|---|---|---|---|---|---|\n");
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
    for e in dirs {
        let rows = read_history(&e.path().join("history.csv"));
        if rows.is_empty() {
            continue;
        }
        let num = |r: &BTreeMap<String, String>, k: &str| r.get(k).and_then(|v| v.parse::<f64>().ok());
        let (mut gold, mut xp, mut mush) = (0.0, 0.0, 0.0);
        let mut by_date = serde_json::Map::new();
        let mut class = String::new();
        for r in &rows {
            let Some(date) = r.get("date") else { continue };
            gold += num(r, "gold_gained").unwrap_or(0.0);
            xp += num(r, "xp_gained").unwrap_or(0.0);
            mush += num(r, "mushrooms_gained").unwrap_or(0.0);
            if let Some(c) = r.get("class") {
                class.clone_from(c);
            }
            dates.insert(date.clone());
            by_date.insert(
                date.clone(),
                serde_json::json!({
                    "gold": gold, "xp": xp, "mushrooms": mush,
                    "dungeons": num(r, "dungeons"), "rank": num(r, "rank"), "strength": num(r, "strength"),
                }),
            );
        }
        chars.push(serde_json::json!({ "nick": e.file_name().to_string_lossy().trim_start_matches("_demo_"), "cls": class, "rows": by_date }));
    }
    let data = serde_json::json!({
        "generated": Local::now().format("%Y-%m-%d %H:%M").to_string(),
        "dates": dates.into_iter().collect::<Vec<_>>(),
        "chars": chars,
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

/// Minimal base64 (for embedding the portrait into the HTML, so it works from any folder).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A portrait made by hand (the game renders it in a WebGL canvas, the bot cannot): `roster/<nick>/portrait.png|jpg`.
fn portrait_html() -> String {
    let Some(d) = dir() else { return String::new() };
    for (file, mime) in [("portrait.png", "image/png"), ("portrait.jpg", "image/jpeg"), ("portrait.jpeg", "image/jpeg")] {
        if let Ok(data) = fs::read(d.join(file)) {
            return format!(r#"<img class="portrait" src="data:{mime};base64,{}" alt="portrait">"#, base64(&data));
        }
    }
    String::new()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn slot_name(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::Hat => "Helmet",
        EquipmentSlot::BreastPlate => "Chest plate",
        EquipmentSlot::Gloves => "Gloves",
        EquipmentSlot::FootWear => "Boots",
        EquipmentSlot::Amulet => "Amulet",
        EquipmentSlot::Belt => "Belt",
        EquipmentSlot::Ring => "Ring",
        EquipmentSlot::Talisman => "Talisman",
        EquipmentSlot::Weapon => "Weapon",
        EquipmentSlot::Shield => "Shield",
    }
}

fn attr_short(a: AttributeType) -> &'static str {
    match a {
        AttributeType::Strength => "STR",
        AttributeType::Dexterity => "DEX",
        AttributeType::Intelligence => "INT",
        AttributeType::Constitution => "CON",
        AttributeType::Luck => "LCK",
    }
}

/// One equipment slot as an "item tooltip" box.
fn item_box(gs: &GameState, slot: EquipmentSlot, item: Option<&Item>) -> String {
    let Some(i) = item else {
        return format!(r#"<div class="item empty"><div class="slot">{}</div><div class="none">empty</div></div>"#, slot_name(slot));
    };
    let rarity = if i.is_legendary() {
        "legendary"
    } else if i.is_epic() {
        "epic"
    } else {
        "normal"
    };
    let mut lines = Vec::new();
    if let ItemType::Weapon { min_dmg, max_dmg } = i.typ {
        lines.push(format!("<b>{min_dmg}–{max_dmg}</b> damage"));
    }
    if i.armor() > 0 {
        lines.push(format!("<b>{}</b> armor", i.armor()));
    }
    let attrs: Vec<String> = ATTRS
        .iter()
        .filter(|a| i.attributes[**a] > 0)
        .map(|a| format!(r#"<span class="attr">{} +{}</span>"#, attr_short(*a), i.attributes[*a]))
        .collect();
    if !attrs.is_empty() {
        lines.push(attrs.join(" "));
    }
    if let Some(e) = i.enchantment {
        lines.push(format!("Enchantment: {e:?}"));
    }
    match &i.gem_slot {
        Some(sf_api::gamestate::items::GemSlot::Filled(g)) => lines.push(format!("Gem: {:?} {}", g.typ, g.value)),
        Some(sf_api::gamestate::items::GemSlot::Empty) => lines.push("Empty gem socket".to_string()),
        None => {}
    }
    if let Some(r) = &i.rune {
        lines.push(format!("Rune: {:?} {}", r.typ, r.value));
    }
    if i.upgrade_count > 0 {
        lines.push(format!("Upgraded ×{}", i.upgrade_count));
    }
    format!(
        r#"<div class="item {rarity}"><div class="slot">{}{}</div>{}<div class="score">value {:.1}</div></div>"#,
        slot_name(slot),
        if rarity == "normal" { String::new() } else { format!(r#" <span class="badge">{rarity}</span>"#) },
        lines.iter().map(|l| format!("<div>{l}</div>")).collect::<String>(),
        crate::inventory::value(gs, i)
    )
}

fn list(items: &[String], empty: &str) -> String {
    if items.is_empty() {
        format!(r#"<p class="muted">{empty}</p>"#)
    } else {
        format!("<ul>{}</ul>", items.iter().map(|i| format!("<li>{}</li>", esc(i))).collect::<String>())
    }
}

/// The whole daily report as one HTML page (also `card.html` = the latest one).
fn page(
    gs: &GameState,
    s: &Snapshot,
    past: &Past,
    best: Option<&str>,
    wins: &[String],
    issues: &[String],
) -> String {
    let c = &gs.character;
    let level_delta = past.last.map_or(String::new(), |(l, _)| match i32::from(s.level) - i32::from(l) {
        0 => String::new(),
        d => format!(r#" <span class="up">{d:+}</span>"#),
    });
    // Arrow before the rank: green up / red down, a little bigger for a bigger move
    let rank_arrow = past.last.map_or(String::new(), |(_, r)| {
        let d = i64::from(r) - i64::from(s.rank); // positive = better
        if d == 0 {
            return String::new();
        }
        let size = 14.0 + ((d.unsigned_abs() as f64).log10() * 4.0).min(12.0);
        let (class, arrow) = if d > 0 { ("up", "▲") } else { ("down", "▼") };
        format!(r#"<span class="{class}" style="font-size:{size:.0}px" title="{d:+}">{arrow}</span> "#)
    });
    let best_rank = past.best_rank.map_or(s.rank, |b| b.min(s.rank));
    let gold_total = past.gold_total + s.gold_gained;
    let mushrooms_total = past.mushrooms_total + s.mushrooms_gained;
    let attrs: String = ATTRS
        .iter()
        .map(|a| {
            let (b, add) = (c.attribute_basis[*a], c.attribute_additions[*a]);
            let main = *a == c.class.main_attribute();
            format!(
                r#"<tr{}><td>{}</td><td>{b}</td><td>+{add}</td><td><b>{}</b></td></tr>"#,
                if main { r#" class="main""# } else { "" },
                attr_short(*a),
                b + add
            )
        })
        .collect();
    let slots = [
        EquipmentSlot::Hat,
        EquipmentSlot::Amulet,
        EquipmentSlot::BreastPlate,
        EquipmentSlot::Gloves,
        EquipmentSlot::Belt,
        EquipmentSlot::Ring,
        EquipmentSlot::FootWear,
        EquipmentSlot::Talisman,
        EquipmentSlot::Weapon,
        EquipmentSlot::Shield,
    ];
    let equip: String = slots
        .iter()
        .filter(|slot| **slot != EquipmentSlot::Shield || c.equipment.0[EquipmentSlot::Shield].is_some())
        .map(|slot| item_box(gs, *slot, c.equipment.0[*slot].as_ref()))
        .collect();
    let potions: Vec<String> = c
        .active_potions
        .iter()
        .flatten()
        .map(|p| {
            let until = p.expires.map_or("?".to_string(), |e| e.format("%d.%m. %H:%M").to_string());
            format!("{:?} {:.0} % until {until}", p.typ, p.size.effect() * 100.0)
        })
        .collect();
    let dungeons: Vec<String> = gs
        .dungeons
        .light
        .iter()
        .filter_map(|(d, p)| match p {
            DungeonProgress::Open { finished } => Some(format!("{d:?} {finished}/10")),
            DungeonProgress::Finished => Some(format!("{d:?} ✓")),
            DungeonProgress::Locked => None,
        })
        .collect();
    let xp_pct = if c.next_level_xp > 0 { c.experience as f64 * 100.0 / c.next_level_xp as f64 } else { 0.0 };
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{name} – {date}</title>
<style>
:root{{--bg:#efe7d8;--fg:#2a2118;--card:#fbf6ec;--muted:#7d705f;--accent:#8a4b12;--line:#d9ccb5;--epic:#7b3fc4;--leg:#c46a00;--up:#2e7d32;--down:#c62828}}
@media (prefers-color-scheme:dark){{:root{{--bg:#16130f;--fg:#efe6d6;--card:#221d17;--muted:#a99a84;--accent:#e2a44f;--line:#3a3128;--epic:#b48cf0;--leg:#ffae42;--up:#81c784;--down:#ef9a9a}}}}
*{{box-sizing:border-box}} body{{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif;margin:0;padding:16px}}
.wrap{{max-width:900px;margin:auto}} .card{{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:16px;margin-bottom:14px}}
h1{{margin:0;color:var(--accent);font-size:26px}} h2{{margin:0 0 10px;font-size:16px;color:var(--accent)}} .muted,.sub{{color:var(--muted)}}
.stats{{display:grid;grid-template-columns:repeat(auto-fit,minmax(130px,1fr));gap:10px;margin-top:12px}}
.k{{color:var(--muted);font-size:12px}} .small{{font-size:13px;font-weight:400;color:var(--muted)}} .v{{font-size:20px;font-weight:700}} .up{{color:var(--up);font-size:14px}} .down{{color:var(--down);font-size:14px}}
.bar{{height:6px;background:var(--line);border-radius:3px;overflow:hidden;margin-top:4px}} .bar>i{{display:block;height:100%;background:var(--accent)}}
.best{{font-size:16px;font-weight:600}} table{{border-collapse:collapse}} td{{padding:3px 12px 3px 0}} tr.main td{{color:var(--accent);font-weight:600}}
.items{{display:grid;grid-template-columns:repeat(auto-fill,minmax(190px,1fr));gap:10px}}
.item{{border:1px solid var(--line);border-left:4px solid var(--muted);border-radius:8px;padding:8px 10px;background:var(--bg)}}
.item.epic{{border-left-color:var(--epic)}} .item.legendary{{border-left-color:var(--leg)}} .item.empty{{opacity:.5}}
.slot{{font-weight:700;margin-bottom:4px}} .badge{{font-size:11px;color:var(--epic);text-transform:uppercase}} .legendary .badge{{color:var(--leg)}}
.attr{{display:inline-block;background:var(--card);border:1px solid var(--line);border-radius:4px;padding:0 5px;margin:2px 2px 0 0;font-size:12px}}
.score{{color:var(--muted);font-size:12px;margin-top:4px}} ul{{margin:0;padding-left:18px}} li{{margin:2px 0}}
.portrait{{float:right;max-height:160px;max-width:40%;border-radius:10px;margin-left:12px;border:1px solid var(--line)}}
.cols{{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:14px}}
</style></head><body><div class="wrap">
<div class="card">{portrait}<h1>{name}</h1><div class="sub">{class} · level {level} · {date}</div>
<div class="stats">
<div><div class="k">Level</div><div class="v">{level}{level_delta}</div><div class="bar"><i style="width:{xp_pct:.0}%"></i></div><div class="k">XP {xp} / {next}</div></div>
<div><div class="k">Hall of Fame</div><div class="v">{rank_arrow}#{rank} <span class="small">(#{best_rank})</span></div></div>
<div><div class="k">Honor</div><div class="v">{honor}</div></div>
<div><div class="k">Strength</div><div class="v">{strength:.0}</div></div>
<div><div class="k">Gold</div><div class="v">+{gold_gained:.0} <span class="small">({gold_total:.0})</span></div></div>
<div><div class="k">Mushrooms</div><div class="v">+{mushrooms_gained} <span class="small">({mushrooms_total})</span></div></div>
</div></div>
<div class="card"><h2>Biggest success of the day</h2><div class="best">{best}</div></div>
<div class="cols">
<div class="card"><h2>Successes</h2>{wins}</div>
<div class="card"><h2>Issues / questions</h2>{issues}</div>
</div>
<div class="cols">
<div class="card"><h2>Attributes</h2><table><tr class="k"><td></td><td>base</td><td>bonus</td><td>total</td></tr>{attrs}</table></div>
<div class="card"><h2>Other</h2><table>
<tr><td class="k">Guild</td><td>{guild}</td></tr>
<tr><td class="k">Arena wins (XP)</td><td>{arena}/10</td></tr><tr><td class="k">Lucky coins</td><td>{coins}</td></tr>
<tr><td class="k">Potions</td><td>{potions}</td></tr>
<tr><td class="k">Dungeons</td><td>{dungeons}</td></tr></table></div>
</div>
<div class="card"><h2>Equipment</h2><div class="items">{equip}</div></div>
</div></body></html>"#,
        portrait = portrait_html(),
        name = esc(&s.name),
        class = s.class,
        level = s.level,
        date = s.date,
        rank = s.rank,
        honor = s.honor,
        strength = s.strength,
        gold_gained = s.gold_gained,
        mushrooms_gained = s.mushrooms_gained,
        xp = c.experience,
        next = c.next_level_xp,
        best = esc(best.unwrap_or("–")),
        wins = list(wins, "–"),
        issues = list(issues, "nothing, a quiet day"),
        guild = esc(&s.guild),
        arena = s.arena_wins,
        coins = s.lucky_coins,
        potions = if potions.is_empty() { "–".to_string() } else { esc(&potions.join(", ")) },
        dungeons = if dungeons.is_empty() { "–".to_string() } else { esc(&dungeons.join(", ")) },
    )
}

/// Level-up detection for the "biggest success" (called every pass of the main loop).
pub fn track_level(gs: &GameState) {
    static LAST: Mutex<Option<u16>> = Mutex::new(None);
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

    /// Demo of one daily report (user 2026-10-07): `cargo test demo_day -- --ignored`.
    /// Notes come from the real `logs/progress.log`, the character values are illustrative (no server access).
    #[test]
    #[ignore]
    fn demo_day() {
        use sf_api::gamestate::{
            character::Class,
            items::{Potion, PotionSize, PotionType},
        };
        set_character("_demo_TestChar1");
        if let Some(d) = dir() {
            let _ = fs::remove_dir_all(&d);
        }
        for line in fs::read_to_string("logs/progress.log").unwrap_or_default().lines() {
            if let Some((_, msg)) = line.split_at_checked(20) {
                observe(msg);
            }
        }
        // An earlier day in the history, so the changes and totals show
        if let Some(d) = dir() {
            let yesterday = Local::now().date_naive() - chrono::Duration::days(1);
            append(&d.join("history.csv"), CSV_HEADER);
            append(&d.join("history.csv"), &format!("{yesterday},12,10230,90,120,98.10,29,50,7,10,-,184.00,2"));
        }
        ledger(7_150, 2, 1_800);
        ledger(900, 0, 0);
        win(5, "Level up: 12 → 13 (demo)");
        observe("[dungeons] Win: xp +578, gold +3.00 g, item no (demo)");
        observe("[inventory] equipping Weapon (epic) (value 99.0 > 59.7 of Weapon) (demo)");

        let mut gs = GameState::default();
        let c = &mut gs.character;
        c.name = "TestChar1".into();
        c.class = Class::Warrior;
        c.level = 13;
        c.experience = 4100;
        c.next_level_xp = 6900;
        c.rank = 9480;
        c.honor = 112;
        c.silver = 15_729;
        c.mushrooms = 31;
        for (a, base, bonus) in [
            (AttributeType::Strength, 28, 41),
            (AttributeType::Dexterity, 10, 6),
            (AttributeType::Intelligence, 9, 3),
            (AttributeType::Constitution, 29, 37),
            (AttributeType::Luck, 14, 12),
        ] {
            c.attribute_basis[a] = base;
            c.attribute_additions[a] = bonus;
        }
        let item = |typ: ItemType, attrs: &[(AttributeType, u32)]| {
            let mut i = Item {
                typ,
                price: 0,
                mushroom_price: 0,
                full_model_id: 0,
                model_id: 1,
                class: None,
                type_specific_val: 0,
                attributes: Default::default(),
                gem_slot: None,
                rune: None,
                enchantment: None,
                color: 0,
                upgrade_count: 0,
                item_quality: 0,
                is_washed: false,
            };
            for (a, v) in attrs {
                i.attributes[*a] = *v;
            }
            if !matches!(i.typ, ItemType::Weapon { .. }) {
                i.type_specific_val = 12;
            }
            i
        };
        use AttributeType::*;
        c.equipment.0[EquipmentSlot::Weapon] = Some(item(ItemType::Weapon { min_dmg: 15, max_dmg: 41 }, &[(Strength, 9)]));
        c.equipment.0[EquipmentSlot::Gloves] = Some(item(ItemType::Gloves, &[(Strength, 6), (Constitution, 8)]));
        c.equipment.0[EquipmentSlot::Belt] = Some(item(ItemType::Belt, &[(Constitution, 5), (Luck, 2)]));
        c.equipment.0[EquipmentSlot::Amulet] = Some(item(ItemType::Amulet, &[(Strength, 7), (Constitution, 6)]));
        c.equipment.0[EquipmentSlot::Hat] = Some(item(ItemType::Hat, &[(Constitution, 4)]));
        c.active_potions[0] = Some(Potion {
            typ: PotionType::Strength,
            size: PotionSize::Medium,
            expires: Some(Local::now() + chrono::Duration::days(2)),
        });
        gs.specials.wheel.lucky_coins = 50;
        gs.tavern.quicksand_glasses = 7;
        gs.arena.fights_for_xp = 5;
        let summary = write_day(&gs);
        println!("demo report written: {summary}");
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
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
            ("MimiMimi11", "Paladin"),
            ("TestChar1", "Warrior"),
            ("Radek", "Druid"),
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
        }
        write_dashboard(true);
        println!("demo dashboard: roster/dashboard.html");
    }
}

