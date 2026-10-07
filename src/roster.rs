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
const WINS: &[(u8, &str)] = &[
    (4, "(epic)"),
    (3, "[tasks] Claiming"),
    (3, "[guild] Joined"),
    (2, "[dungeons] Win"),
    (2, "[hunt] Win"),
    (2, "[shops] Buying an upgrade"),
    (1, "[potions] Drinking"),
];

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
    Some(dir()?.join("days").join(format!("{date}.md")))
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

fn total(gs: &GameState, a: AttributeType) -> u32 {
    gs.character.attribute_basis[a] + gs.character.attribute_additions[a]
}

const ATTRS: [AttributeType; 5] = [
    AttributeType::Strength,
    AttributeType::Dexterity,
    AttributeType::Intelligence,
    AttributeType::Constitution,
    AttributeType::Luck,
];

fn item_line(item: &Item) -> String {
    let attrs: Vec<String> = ATTRS
        .iter()
        .filter(|a| item.attributes[**a] > 0)
        .map(|a| format!("{:?} +{}", a, item.attributes[*a]))
        .collect();
    let dmg = match item.typ {
        ItemType::Weapon { min_dmg, max_dmg } => format!("{min_dmg}–{max_dmg} dmg, "),
        _ => String::new(),
    };
    format!("{dmg}{}{}", attrs.join(", "), if item.is_epic() { " (epic)" } else { "" })
}

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
}

const CSV_HEADER: &str = "date,level,rank,honor,strength,gold,mushrooms,lucky_coins,hourglasses,arena_wins,guild";

impl Snapshot {
    fn of(gs: &GameState) -> Self {
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
        }
    }

    fn csv(&self) -> String {
        format!(
            "{},{},{},{},{:.0},{:.2},{},{},{},{},{}",
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
            self.guild
        )
    }
}

/// Last line of a history.csv before today: (level, rank).
fn previous(history: &Path, today: NaiveDate) -> Option<(u16, u32)> {
    let text = fs::read_to_string(history).ok()?;
    let line = text.lines().skip(1).filter(|l| !l.starts_with(&today.to_string())).last()?;
    let f: Vec<&str> = line.split(',').collect();
    Some((f.get(1)?.parse().ok()?, f.get(2)?.parse().ok()?))
}

/// Writes today's report for the logged-in character and refreshes the shared files. Returns a summary line.
pub fn write_day(gs: &GameState) -> String {
    let Some(d) = dir() else { return "no character set".to_string() };
    let s = Snapshot::of(gs);
    let notes = take_notes();
    let history = d.join("history.csv");
    let prev = previous(&history, s.date);

    // Biggest success: highest priority, the latest of them
    let best = notes.iter().filter(|n| n.1 == "WIN").max_by_key(|n| n.2).map(|n| n.3.clone());
    // Issues grouped by message (with a count)
    let mut issues: BTreeMap<String, usize> = BTreeMap::new();
    for n in notes.iter().filter(|n| n.1 == "ISSUE") {
        *issues.entry(n.3.clone()).or_default() += 1;
    }
    let wins: Vec<&String> = notes.iter().filter(|n| n.1 == "WIN" && n.2 >= 2).map(|n| &n.3).collect();

    let mut md = format!("# {} – {} ({})\n\n", s.name, s.date, s.class);
    md += "## End of day\n";
    md += &format!("- Level: **{}**{}\n", s.level, prev.map_or(String::new(), |(l, _)| format!(" ({:+})", i32::from(s.level) - i32::from(l))));
    md += &format!(
        "- Hall of Fame rank: **{}**{}\n",
        s.rank,
        prev.map_or(String::new(), |(_, r)| format!(" ({:+}, minus = better)", i64::from(s.rank) - i64::from(r)))
    );
    md += &format!("- Honor {}, strength {:.0} (Arena formula), Arena wins for XP today {}/10\n", s.honor, s.strength, s.arena_wins);
    md += &format!(
        "- Gold {:.2} g, mushrooms {}, lucky coins {}, hourglasses {}, guild {}\n\n",
        s.gold, s.mushrooms, s.lucky_coins, s.hourglasses, s.guild
    );
    md += &format!("## Biggest success\n{}\n\n", best.as_deref().unwrap_or("–"));
    md += "## Other successes\n";
    md += &if wins.is_empty() { "–\n".to_string() } else { wins.iter().map(|w| format!("- {w}\n")).collect::<String>() };
    md += "\n## Issues / questions\n";
    md += &if issues.is_empty() {
        "–\n".to_string()
    } else {
        issues.iter().map(|(m, c)| if *c > 1 { format!("- {m} ({c}×)\n") } else { format!("- {m}\n") }).collect::<String>()
    };

    if let Some(f) = day_file(s.date) {
        let _ = fs::create_dir_all(f.parent().unwrap_or(Path::new(ROOT)));
        let _ = fs::write(&f, &md);
    }
    if !history.exists() {
        append(&history, CSV_HEADER);
    }
    append(&history, &s.csv());
    let _ = fs::write(d.join("card.html"), card(gs, &s));
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
        if let Ok(day) = fs::read_to_string(e.path().join("days").join(format!("{date}.md")))
            && let Some(i) = day.find("## Issues / questions")
        {
            issues += &format!("== {nick} ==\n{}\n", day[i + "## Issues / questions".len()..].trim());
            issues += "\n";
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
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Character card (HTML) – what the character screen shows, instead of a screenshot.
fn card(gs: &GameState, s: &Snapshot) -> String {
    let c = &gs.character;
    let attrs: String = ATTRS
        .iter()
        .map(|a| {
            format!(
                "<tr><td>{:?}</td><td>{}</td><td>+{}</td><td><b>{}</b></td></tr>",
                a,
                c.attribute_basis[*a],
                c.attribute_additions[*a],
                total(gs, *a)
            )
        })
        .collect();
    let slots = [
        EquipmentSlot::Hat,
        EquipmentSlot::BreastPlate,
        EquipmentSlot::Gloves,
        EquipmentSlot::FootWear,
        EquipmentSlot::Amulet,
        EquipmentSlot::Belt,
        EquipmentSlot::Ring,
        EquipmentSlot::Talisman,
        EquipmentSlot::Weapon,
        EquipmentSlot::Shield,
    ];
    let equip: String = slots
        .iter()
        .map(|slot| {
            let v = c.equipment.0[*slot].as_ref().map_or("–".to_string(), item_line);
            format!("<tr><td>{slot:?}</td><td>{}</td></tr>", esc(&v))
        })
        .collect();
    let potions: String = c
        .active_potions
        .iter()
        .flatten()
        .map(|p| {
            let until = p.expires.map_or("?".to_string(), |e| e.format("%d.%m. %H:%M").to_string());
            format!("<li>{:?} {:.0} % until {until}</li>", p.typ, p.size.effect() * 100.0)
        })
        .collect();
    let mount = format!(
        "{:?} until {}",
        c.mount,
        c.mount_end.map_or("–".to_string(), |e| e.format("%d.%m. %H:%M").to_string())
    );
    let dungeons: String = gs
        .dungeons
        .light
        .iter()
        .filter_map(|(d, p)| match p {
            DungeonProgress::Open { finished } => Some(format!("{d:?} {finished}/10")),
            DungeonProgress::Finished => Some(format!("{d:?} done")),
            DungeonProgress::Locked => None,
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{name}</title>
<style>
:root{{--bg:#f6f4ef;--fg:#222;--card:#fff;--muted:#777;--accent:#8a5a00}}
@media (prefers-color-scheme:dark){{:root{{--bg:#1b1a17;--fg:#eee;--card:#262420;--muted:#aaa;--accent:#e0b050}}}}
body{{background:var(--bg);color:var(--fg);font:14px/1.4 system-ui,sans-serif;margin:16px}}
.card{{background:var(--card);border-radius:10px;padding:16px;max-width:720px;margin:auto;box-shadow:0 1px 4px #0003}}
h1{{margin:0;color:var(--accent)}} .sub{{color:var(--muted)}} table{{border-collapse:collapse;width:100%;margin:8px 0}}
td{{padding:3px 6px;border-bottom:1px solid #8883}} .grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:8px;margin:12px 0}}
.k{{color:var(--muted);font-size:12px}} .v{{font-size:18px;font-weight:600}}
</style></head><body><div class="card">
<h1>{name}</h1><div class="sub">{class}, level {level} · {date} 23:50</div>
<div class="grid">
<div><div class="k">Hall of Fame</div><div class="v">#{rank}</div></div>
<div><div class="k">Honor</div><div class="v">{honor}</div></div>
<div><div class="k">Strength</div><div class="v">{strength:.0}</div></div>
<div><div class="k">Gold</div><div class="v">{gold:.2} g</div></div>
<div><div class="k">Mushrooms</div><div class="v">{mushrooms}</div></div>
<div><div class="k">XP</div><div class="v">{xp} / {next}</div></div>
</div>
<h3>Attributes</h3><table><tr class="k"><td></td><td>base</td><td>bonus</td><td>total</td></tr>{attrs}</table>
<h3>Equipment</h3><table>{equip}</table>
<h3>Potions</h3><ul>{potions}</ul>
<p><span class="k">Mount</span> {mount} · <span class="k">Guild</span> {guild} · <span class="k">Lucky coins</span> {coins} · <span class="k">Hourglasses</span> {glasses}</p>
<p><span class="k">Dungeons</span> {dungeons}</p>
</div></body></html>"#,
        name = esc(&s.name),
        class = s.class,
        level = s.level,
        date = s.date,
        rank = s.rank,
        honor = s.honor,
        strength = s.strength,
        gold = s.gold,
        mushrooms = s.mushrooms,
        xp = c.experience,
        next = c.next_level_xp,
        attrs = attrs,
        equip = equip,
        potions = if potions.is_empty() { "<li>–</li>".to_string() } else { potions },
        mount = esc(&mount),
        guild = esc(&s.guild),
        coins = s.lucky_coins,
        glasses = s.hourglasses,
        dungeons = esc(&dungeons),
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
        assert_eq!(classify("[tasks] Claiming the daily chest 3 (12 points)"), Some(("WIN", 3)));
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
        win(5, "Level up: 12 → 13 (demo)");

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
}

