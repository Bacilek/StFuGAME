//! Round-robin of the challenge characters: every day at 23:40, 10 min before the 23:50 report (user 2026-10-07,
//! time moved from 23:20 user 2026-10-08), every character "fights" every other one;
//! the average win rate and the head-to-head go to the dashboard (no separate tournament page any more). Simulated with sf-api's fight simulator (the server's rules) `ITERATIONS` times
//! per pair, so the result is a fair win rate instead of one random fight – no Arena cooldown, no honor lost.
//! Participants = the characters in `roster/roster.md`, loaded via `ViewPlayer` (the bot's own character from its
//! own state). Output: `roster/tournament/<date>.json` (every day).

use std::{fs, path::Path};

use chrono::{Local, NaiveDate};
use enum_map::EnumMap;
use sf_api::{
    command::{AttributeType, Command},
    gamestate::character::Class,
    simulate::{DamageRange, Fighter, PlayerFighterSquad, UpgradeableFighter, Weapon, simulate_battle},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

const ROOT: &str = "roster";
/// Simulated fights per pair.
const ITERATIONS: u32 = 1000;

/// Character nicks from the roster table (`| Class | Character (nick) | Friend | … |`).
pub fn participants() -> Vec<(String, String)> {
    let text = fs::read_to_string(Path::new(ROOT).join("roster.md")).unwrap_or_default();
    text.lines()
        .filter(|l| l.starts_with('|') && !l.contains("---") && !l.contains("Character (nick)"))
        .filter_map(|l| {
            let cells: Vec<&str> = l.split('|').map(str::trim).collect();
            let (class, nick) = (cells.get(1)?, cells.get(2)?);
            (!nick.is_empty() && *nick != "–" && *nick != "-").then(|| (nick.to_string(), class.to_string()))
        })
        .collect()
}

/// Day 1 of the challenge: `roster/start.txt` (YYYY-MM-DD), otherwise the first report of any character.
pub fn start_date() -> Option<NaiveDate> {
    if let Ok(s) = fs::read_to_string(Path::new(ROOT).join("start.txt"))
        && let Ok(d) = s.trim().parse()
    {
        return Some(d);
    }
    let mut first: Option<NaiveDate> = None;
    for e in fs::read_dir(ROOT).ok()?.flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('_')) {
        let text = fs::read_to_string(e.path().join("history.csv")).unwrap_or_default();
        if let Some(d) = text.lines().nth(1).and_then(|l| l.split(',').next()).and_then(|d| d.parse().ok()) {
            first = Some(first.map_or(d, |f: NaiveDate| f.min(d)));
        }
    }
    first
}

/// Challenge day number (1-based) for a given date, per `start_date()`.
pub fn day_number(date: NaiveDate) -> i64 {
    (date - start_date().unwrap_or(date)).num_days() + 1
}

/// The most recent date this round-robin already has a finalized round for, if any.
fn last_final_date(dir: &Path) -> Option<NaiveDate> {
    let mut best: Option<NaiveDate> = None;
    for e in fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if let Some(date) = name.strip_suffix(".final").and_then(|s| s.parse::<NaiveDate>().ok()) {
            best = Some(best.map_or(date, |b| b.max(date)));
        }
    }
    best
}

/// Dates (oldest first) whose round has not been simulated yet: every date between the day after the last
/// finalized one and yesterday (always overdue), plus today once 23:40 has passed. Mirrors
/// `roster::overdue_days` – see there for why backdating a day the bot was off for is safe (nothing happens to
/// any character while the whole process isn't running, so today's live `ViewPlayer` data IS that missed day's
/// end-of-day state too).
pub fn overdue_days() -> Vec<NaiveDate> {
    let dir = Path::new(ROOT).join("tournament");
    let today = Local::now().date_naive();
    let mut day = last_final_date(&dir).map_or_else(|| start_date().unwrap_or(today), |d| d + chrono::Duration::days(1));
    let mut out = Vec::new();
    while day < today {
        out.push(day);
        day += chrono::Duration::days(1);
    }
    if Local::now() >= run_time(today) && !dir.join(format!("{today}.final")).exists() {
        out.push(today);
    }
    out
}

fn run_time(date: NaiveDate) -> chrono::DateTime<Local> {
    let t = chrono::NaiveTime::from_hms_opt(23, 40, 0).unwrap_or_default();
    date.and_time(t).and_local_timezone(Local).earliest().unwrap_or_else(Local::now)
}

/// Seconds until today's duels (for the main loop's wait); None when the time has passed.
pub fn secs_until_due() -> Option<u64> {
    let now = Local::now();
    let at = run_time(now.date_naive());
    (now < at).then(|| u64::try_from((at - now).num_seconds()).unwrap_or(0) + 5)
}

/// Average win rate of every character in every daily round: date → nick → win rate (0–1).
pub fn daily_win_rates(demo: bool) -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>> {
    let mut out = std::collections::BTreeMap::new();
    for r in rounds(demo) {
        let Some(date) = r["date"].as_str() else { continue };
        let players = r["players"].as_array().cloned().unwrap_or_default();
        let win = r["win"].as_array().cloned().unwrap_or_default();
        let mut day = std::collections::BTreeMap::new();
        for (i, p) in players.iter().enumerate() {
            let row: Vec<f64> = win.get(i).and_then(|w| w.as_array()).map_or_else(Vec::new, |w| {
                w.iter().filter_map(serde_json::Value::as_f64).collect()
            });
            if let (Some(nick), false) = (p["nick"].as_str(), row.is_empty()) {
                day.insert(nick.to_string(), row.iter().sum::<f64>() / row.len() as f64);
            }
        }
        out.insert(date.to_string(), day);
    }
    out
}

/// Head-to-head of every daily round for the dashboard: date → {players, win}.
pub fn head_to_head(demo: bool) -> serde_json::Map<String, serde_json::Value> {
    rounds(demo)
        .into_iter()
        .filter_map(|r| {
            let date = r["date"].as_str()?.to_string();
            let players: Vec<serde_json::Value> = r["players"].as_array()?.iter().map(|p| p["nick"].clone()).collect();
            Some((date, serde_json::json!({ "players": players, "win": r["win"].clone() })))
        })
        .collect()
}

/// All saved rounds, sorted by day. Demo rounds (`_demo-*`) only for a demo.
fn rounds(demo: bool) -> Vec<serde_json::Value> {
    let dir = Path::new(ROOT).join("tournament");
    let mut v: Vec<serde_json::Value> = fs::read_dir(&dir)
        .map(|it| {
            it.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter(|e| demo == e.file_name().to_string_lossy().starts_with("_demo"))
                .filter_map(|e| serde_json::from_str(&fs::read_to_string(e.path()).ok()?).ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|r| r["day"].as_i64().unwrap_or(0));
    // Older rounds may spell a nick with different capitals than roster.md (MimiMimi11 vs Mimimimi11); the dashboard
    // matches characters by the roster spelling, so a mismatch made the character vanish from the charts
    let canon = participants();
    for r in &mut v {
        for p in r["players"].as_array_mut().into_iter().flatten() {
            if let Some(c) = p["nick"].as_str().and_then(|n| canon.iter().find(|(c, _)| c.eq_ignore_ascii_case(n))) {
                p["nick"] = serde_json::Value::String(c.0.clone());
            }
        }
    }
    v
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[tournament] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Loads every participant and simulates all pairs. Writes the round and the tournament page.
/// Today's challenge day (for a manual run).
pub fn today() -> i64 {
    let today = Local::now().date_naive();
    (today - start_date().unwrap_or(today)).num_days() + 1
}

/// Allows the duels to run again today (manual "end of day now"; the 23:40 run then replaces the round).
pub fn unlock_today() {
    let dir = Path::new(ROOT).join("tournament");
    let date = Local::now().date_naive();
    let _ = fs::remove_file(dir.join(format!("{date}.lock")));
    let _ = fs::remove_file(dir.join(format!("{date}.json")));
}

/// Health multiplier per class (own copy: `sf_api::gamestate::character::Class::health_multiplier` is
/// crate-private). `is_companion` is always false for our fighters, so the Warrior/companion special case
/// (6.1) never applies here.
fn health_multiplier(class: Class) -> f64 {
    use Class::{Assassin, Bard, BattleMage, Berserker, DemonHunter, Druid, Mage, Necromancer, Paladin, PlagueDoctor, Scout, Warrior};
    match class {
        Warrior | BattleMage | Druid => 5.0,
        Paladin => 6.0,
        PlagueDoctor | Scout | Assassin | Berserker | DemonHunter | Necromancer => 4.0,
        Mage | Bard => 2.0,
    }
}

/// The exact `Class` Debug spelling (no spaces, e.g. "BattleMage") as stored in `history.csv`'s `class` column.
fn parse_class(s: &str) -> Option<Class> {
    use Class::{Assassin, Bard, BattleMage, Berserker, DemonHunter, Druid, Mage, Necromancer, Paladin, PlagueDoctor, Scout, Warrior};
    Some(match s {
        "Warrior" => Warrior,
        "Mage" => Mage,
        "Scout" => Scout,
        "Assassin" => Assassin,
        "BattleMage" => BattleMage,
        "Berserker" => Berserker,
        "Druid" => Druid,
        "Bard" => Bard,
        "Necromancer" => Necromancer,
        "DemonHunter" => DemonHunter,
        "Paladin" => Paladin,
        "PlagueDoctor" => PlagueDoctor,
        _ => return None,
    })
}

/// Parses an item's "d" description (built by `roster::item_desc`, e.g. "15–41 dmg, STR +9" or "CON +2") back
/// into a weapon damage range (if any) and attribute bonuses (added into `attrs`), to rebuild a `Fighter` from a
/// stored `days/<date>.json` snapshot instead of live data.
fn parse_item_desc(d: &str, attrs: &mut EnumMap<AttributeType, u32>) -> Option<(f64, f64)> {
    let mut dmg = None;
    for part in d.split(", ") {
        if let Some(rest) = part.strip_suffix(" dmg")
            && let Some((min, max)) = rest.split_once('–')
            && let (Ok(min), Ok(max)) = (min.trim().parse(), max.trim().parse())
        {
            dmg = Some((min, max));
            continue;
        }
        if let Some((abbr, n)) = part.rsplit_once(" +")
            && let Ok(n) = n.trim().parse::<u32>()
        {
            let a = match abbr {
                "STR" => Some(AttributeType::Strength),
                "DEX" => Some(AttributeType::Dexterity),
                "INT" => Some(AttributeType::Intelligence),
                "CON" => Some(AttributeType::Constitution),
                "LCK" => Some(AttributeType::Luck),
                _ => None,
            };
            if let Some(a) = a {
                attrs[a] += n;
            }
        }
    }
    dmg
}

/// Builds a `Fighter` from this character's own stored Day 0 snapshot (`roster::day0_snapshot`), not live
/// `ViewPlayer` data – by the time one character's first login triggers `run_day0`, others may well have already
/// leveled past their own Day 0 (user 2026-10-08: Sanek/Květoš still level 2 in the round, but everyone else
/// already level 4-7 by the time it ran, because the round used live data for them). Covers Chlamydie's manually
/// simulated baseline too, the same way (no special-casing needed). Armor is always 0 and there is no gem/rune/
/// potion/portal bonus – `days/<date>.json` does not record those, only level/base attributes/equipped items.
fn fighter_from_day0(nick: &str) -> Option<Fighter> {
    let (class_str, snap) = crate::roster::day0_snapshot(nick)?;
    let class = parse_class(&class_str)?;
    let level = u16::try_from(snap["level"].as_u64()?).ok()?;
    let mut attrs: EnumMap<AttributeType, u32> = EnumMap::default();
    for (key, a) in [
        ("STR", AttributeType::Strength),
        ("DEX", AttributeType::Dexterity),
        ("INT", AttributeType::Intelligence),
        ("CON", AttributeType::Constitution),
        ("LCK", AttributeType::Luck),
    ] {
        attrs[a] = u32::try_from(snap["attrs"][key].as_u64().unwrap_or(0)).unwrap_or(0);
    }
    let (mut weapon_dmg, mut second_dmg) = (None, None);
    for (slot, item) in snap["equip"].as_object().into_iter().flatten() {
        if let Some(d) = item["d"].as_str()
            && let Some(dmg) = parse_item_desc(d, &mut attrs)
        {
            // An Assassin's second weapon sits in the Shield slot of the snapshot
            if slot == "Shield" && class == Class::Assassin {
                second_dmg = Some(dmg);
            } else {
                weapon_dmg = Some(dmg);
            }
        }
    }
    let con = attrs[AttributeType::Constitution];
    let max_health = f64::from(con) * health_multiplier(class) * f64::from(level + 1);
    Some(Fighter {
        ident: Default::default(),
        name: std::sync::Arc::from(nick),
        class,
        level,
        attributes: attrs,
        max_health,
        armor: 0,
        first_weapon: weapon_dmg.map(|(min, max)| Weapon { rune_value: 0, rune_type: None, damage: DamageRange { min, max } }),
        second_weapon: second_dmg.map(|(min, max)| Weapon { rune_value: 0, rune_type: None, damage: DamageRange { min, max } }),
        has_reaction_enchant: false,
        crit_dmg_multi: 2.0,
        resistances: EnumMap::default(),
        portal_dmg_bonus: 0.0,
        is_companion: false,
        gladiator_lvl: 0,
    })
}

/// Loads every participant (the bot's own character from its own state, everyone else via `ViewPlayer`, except
/// `overrides` which are used as-is) and simulates all pairs. `win[i][j]` = share of fights `i` won against `j`.
async fn simulate(
    session: &mut SimpleSession,
    overrides: &[(&str, Fighter)],
) -> Result<(Vec<(String, String)>, Vec<Vec<Option<f64>>>, Vec<String>), Outcome> {
    let players = participants();
    let own = session.game_state().map(|gs| gs.character.name.clone()).unwrap_or_default();
    let mut fighters: Vec<(String, String, Fighter)> = Vec::new();
    let mut missing = Vec::new();
    for (nick, class) in &players {
        let fighter = if let Some((_, f)) = overrides.iter().find(|(n, _)| nick.eq_ignore_ascii_case(n)) {
            Some(f.clone())
        } else if nick.eq_ignore_ascii_case(&own) {
            session.game_state().map(|gs| Fighter::from(&PlayerFighterSquad::new(gs).character))
        } else {
            let name = crate::roster::game_name(nick);
            let gs = match safe::send(session, Command::ViewPlayer { ident: name.clone() }).await {
                Ok(gs) => gs,
                Err(e) if crate::tavern::is_session_error(&e) => return Err(fail(&e)),
                Err(e) => {
                    report!("[tournament] {nick} could not be loaded: {e}");
                    missing.push(nick.clone());
                    continue;
                }
            };
            gs.lookup.lookup_name(&name).map(|p| Fighter::from(&UpgradeableFighter::from_other(p)))
        };
        match fighter {
            Some(f) => fighters.push((nick.clone(), class.clone(), f)),
            None => missing.push(nick.clone()),
        }
    }
    let n = fighters.len();
    let mut win = vec![vec![None; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let r = simulate_battle(
                std::slice::from_ref(&fighters[i].2),
                std::slice::from_ref(&fighters[j].2),
                ITERATIONS,
                true,
            );
            win[i][j] = Some(r.win_ratio);
            win[j][i] = Some(1.0 - r.win_ratio);
        }
    }
    Ok((fighters.iter().map(|(n, c, _)| (n.clone(), c.clone())).collect(), win, missing))
}

/// Runs the round-robin for `date` (normally today, at/after 23:40). A `date` in the past is how
/// `overdue_days()` backfills a day the bot was off for entirely – always written as final straight away
/// (never a preview), using today's live `ViewPlayer` data under that backdated date (see `overdue_days`).
pub async fn run(session: &mut SimpleSession, day: i64, date: NaiveDate) -> Outcome {
    // With several characters in one process only the first one to get here runs the duels (lock file)
    let dir = Path::new(ROOT).join("tournament");
    let _ = fs::create_dir_all(&dir);
    let lock = dir.join(format!("{date}.lock"));
    if fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() {
        return Outcome::Done;
    }
    let today = Local::now().date_naive();
    let preview = date == today && Local::now() < run_time(date);
    report!("[tournament] Day {day}: {} characters, {ITERATIONS} simulated fights per pair", participants().len());
    let (players, win, missing) = match simulate(session, &[]).await {
        Ok(r) => r,
        Err(o) => return o,
    };
    if players.len() < 2 {
        report!("[tournament] Fewer than 2 characters could be loaded ({} missing), no round today", missing.len());
        return Outcome::Done;
    }
    let round = serde_json::json!({
        "day": day,
        "date": date.to_string(),
        "iterations": ITERATIONS,
        "players": players.iter().map(|(n, c)| serde_json::json!({"nick": n, "cls": c})).collect::<Vec<_>>(),
        "win": win,
        "missing": missing,
    });
    let _ = fs::write(dir.join(format!("{date}.json")), round.to_string());
    // After 23:40 (or for a backdated past day, always) this is the day's real round (a manual run earlier
    // today is only a preview)
    if date < today || Local::now() >= run_time(date) {
        let _ = fs::write(dir.join(format!("{date}.final")), "");
    }
    report!(
        "[tournament] Day {day} done ({} characters, {} missing){}",
        players.len(),
        missing.len(),
        if preview { ", preview" } else { "" }
    );
    if preview {
        // the real 23:40 run must still be able to take the lock
        let _ = fs::remove_file(&lock);
    }
    Outcome::Done
}

/// Runs (or re-runs) the "Day 0" baseline round, backdated to `date` so it lines up with every character's own
/// Day 0 snapshot (`roster::write_day0`) regardless of when each is actually handed to the bot. Re-run every time
/// a freshly added character reaches its own Day 0, to pick up newcomers while others are still fresh too.
/// Every participant with a stored Day 0 snapshot fights with THAT (`fighter_from_day0`), not live `ViewPlayer`
/// data – otherwise whoever triggers this is compared fairly, but everyone else is pulled in at whatever level
/// they already reached by that moment (user 2026-10-08: found exactly this – most participants were already
/// level 4-7 in a round meant to be "everyone at level 2"). Only participants with no stored snapshot yet fall
/// back to `ViewPlayer`.
pub async fn run_day0(session: &mut SimpleSession, date: NaiveDate) -> Outcome {
    let dir = Path::new(ROOT).join("tournament");
    let _ = fs::create_dir_all(&dir);
    let lock = dir.join(format!("{date}.lock0"));
    if fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() {
        return Outcome::Done; // another character's first login is already doing this
    }
    let day0_fighters: Vec<(String, Fighter)> =
        participants().iter().filter_map(|(nick, _)| fighter_from_day0(nick).map(|f| (nick.clone(), f))).collect();
    let overrides: Vec<(&str, Fighter)> = day0_fighters.iter().map(|(n, f)| (n.as_str(), f.clone())).collect();
    let (players, win, missing) = match simulate(session, &overrides).await {
        Ok(r) => r,
        Err(o) => {
            let _ = fs::remove_file(&lock);
            return o;
        }
    };
    if players.len() >= 2 {
        let round = serde_json::json!({
            "day": 0,
            "date": date.to_string(),
            "iterations": ITERATIONS,
            "players": players.iter().map(|(n, c)| serde_json::json!({"nick": n, "cls": c})).collect::<Vec<_>>(),
            "win": win,
            "missing": missing,
        });
        let _ = fs::write(dir.join(format!("{date}.json")), round.to_string());
        report!("[tournament] Day 0 baseline ({date}): {} characters, {} missing", players.len(), missing.len());
    }
    let _ = fs::remove_file(&lock);
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Demo with made-up win rates for 7 days (same dates as the dashboard demo):
    /// `cargo test demo_tournament -- --ignored`, then `cargo test demo_dashboard -- --ignored`.
    #[test]
    #[ignore]
    fn demo_tournament() {
        let players = participants();
        let n = players.len();
        let dir = Path::new(ROOT).join("tournament");
        let _ = fs::create_dir_all(&dir);
        let start = Local::now().date_naive() - chrono::Duration::days(6);
        let mut rng = fastrand::Rng::with_seed(3);
        let mut skill: Vec<f64> = (0..n).map(|_| rng.f64()).collect();
        for day in 1..=7i64 {
            for s in &mut skill {
                *s += (rng.f64() - 0.5) * 0.15;
            }
            let mut win = vec![vec![None; n]; n];
            for i in 0..n {
                for j in (i + 1)..n {
                    let p = (0.5 + (skill[i] - skill[j]) * 1.5).clamp(0.02, 0.98);
                    win[i][j] = Some((p * 1000.0).round() / 1000.0);
                    win[j][i] = Some(((1.0 - p) * 1000.0).round() / 1000.0);
                }
            }
            let date = start + chrono::Duration::days(day - 1);
            let round = serde_json::json!({
                "day": day, "date": date.to_string(), "iterations": ITERATIONS,
                "players": players.iter().map(|(n, c)| serde_json::json!({"nick": n, "cls": c})).collect::<Vec<_>>(),
                "win": win, "missing": [],
            });
            let _ = fs::write(dir.join(format!("_demo-{date}.json")), round.to_string());
        }
    }
}
