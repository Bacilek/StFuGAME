//! Round-robin of the challenge characters: every day at 23:40, 10 min before the 23:50 report (user 2026-10-07,
//! time moved from 23:20 user 2026-10-08), every character "fights" every other one;
//! the average win rate and the head-to-head go to the dashboard (no separate tournament page any more). Simulated with sf-api's fight simulator (the server's rules) `ITERATIONS` times
//! per pair, so the result is a fair win rate instead of one random fight – no Arena cooldown, no honor lost.
//! Participants = the characters in `roster/roster.md`, loaded via `ViewPlayer` (the bot's own character from its
//! own state). Output: `roster/tournament/<date>.json` (every day).

use std::{fs, path::Path};

use chrono::{Local, NaiveDate};
use sf_api::{
    command::Command,
    simulate::{Fighter, PlayerFighterSquad, UpgradeableFighter, simulate_battle},
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

/// Today's challenge day if today's round has not been simulated yet (every day).
/// The daily duels run at 23:40 (user 2026-10-08, was 23:20), before the 23:50 report.
pub fn due_today() -> Option<i64> {
    let now = Local::now();
    let today = now.date_naive();
    let day = (today - start_date().unwrap_or(today)).num_days() + 1;
    let done = Path::new(ROOT).join("tournament").join(format!("{today}.final")).exists();
    (now >= run_time(today) && !done).then_some(day)
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

pub async fn run(session: &mut SimpleSession, day: i64) -> Outcome {
    // With several characters in one process only the first one to get here runs the duels (lock file)
    let dir = Path::new(ROOT).join("tournament");
    let _ = fs::create_dir_all(&dir);
    let lock = dir.join(format!("{}.lock", Local::now().date_naive()));
    if fs::OpenOptions::new().write(true).create_new(true).open(&lock).is_err() {
        return Outcome::Done;
    }
    let preview = Local::now() < run_time(Local::now().date_naive());
    let players = participants();
    report!("[tournament] Day {day}: {} characters, {ITERATIONS} simulated fights per pair", players.len());
    let own = session.game_state().map(|gs| gs.character.name.clone()).unwrap_or_default();
    let mut fighters: Vec<(String, String, Fighter)> = Vec::new();
    let mut missing = Vec::new();
    for (nick, class) in &players {
        let fighter = if nick.eq_ignore_ascii_case(&own) {
            session.game_state().map(|gs| Fighter::from(&PlayerFighterSquad::new(gs).character))
        } else {
            let gs = match safe::send(session, Command::ViewPlayer { ident: nick.clone() }).await {
                Ok(gs) => gs,
                Err(e) if crate::tavern::is_session_error(&e) => return fail(&e),
                Err(e) => {
                    report!("[tournament] {nick} could not be loaded: {e}");
                    missing.push(nick.clone());
                    continue;
                }
            };
            gs.lookup.lookup_name(nick).map(|p| Fighter::from(&UpgradeableFighter::from_other(p)))
        };
        match fighter {
            Some(f) => fighters.push((nick.clone(), class.clone(), f)),
            None => missing.push(nick.clone()),
        }
    }

    // win[i][j] = share of fights i won against j
    let n = fighters.len();
    if n < 2 {
        report!("[tournament] Fewer than 2 characters could be loaded ({} missing), no round today", missing.len());
        return Outcome::Done;
    }
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
    let round = serde_json::json!({
        "day": day,
        "date": Local::now().date_naive().to_string(),
        "iterations": ITERATIONS,
        "players": fighters.iter().map(|(n, c, _)| serde_json::json!({"nick": n, "cls": c})).collect::<Vec<_>>(),
        "win": win,
        "missing": missing,
    });
    let date = Local::now().date_naive();
    let _ = fs::write(dir.join(format!("{date}.json")), round.to_string());
    // After 23:40 this is the day's real round (a manual run earlier is only a preview)
    if Local::now() >= run_time(date) {
        let _ = fs::write(dir.join(format!("{date}.final")), "");
    }
    report!("[tournament] Day {day} done ({n} characters, {} missing){}", missing.len(), if preview { ", preview" } else { "" });
    if preview {
        // the real 23:40 run must still be able to take the lock
        let _ = fs::remove_file(&lock);
    }
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
