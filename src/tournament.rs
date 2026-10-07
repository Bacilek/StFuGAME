//! Round-robin tournament of the challenge characters (user 2026-10-07): every day at 23:50 every character "fights"
//! every other one (the daily win rate goes to the dashboard); the rounds of day 1, 3, 7 and 14 make the tournament page. Simulated with sf-api's fight simulator (the server's rules) `ITERATIONS` times
//! per pair, so the result is a fair win rate instead of one random fight – no Arena cooldown, no honor lost.
//! Participants = the characters in `roster/roster.md`, loaded via `ViewPlayer` (the bot's own character from its
//! own state). Output: `roster/tournament/<date>.json` (every day) + `roster/tournament.html` (rounds of DAYS).

use std::{fs, path::Path};

use chrono::{Local, NaiveDate};
use sf_api::{
    command::Command,
    simulate::{Fighter, PlayerFighterSquad, UpgradeableFighter, simulate_battle},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

const ROOT: &str = "roster";
/// The days of the challenge after which the tournament runs.
pub const DAYS: [i64; 4] = [1, 3, 7, 14];
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
pub fn due_today() -> Option<i64> {
    let today = Local::now().date_naive();
    let day = (today - start_date().unwrap_or(today)).num_days() + 1;
    let done = Path::new(ROOT).join("tournament").join(format!("{today}.json")).exists();
    (!done).then_some(day)
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
pub async fn run(session: &mut SimpleSession, day: i64) -> Outcome {
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
    let dir = Path::new(ROOT).join("tournament");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(format!("{}.json", Local::now().date_naive())), round.to_string());
    write_page(false);
    report!("[tournament] Day {day} done ({n} characters, {} missing)", missing.len());
    Outcome::Done
}

/// `roster/tournament.html` from the rounds of day 1, 3, 7 and 14 (template src/tournament.html).
pub fn write_page(demo: bool) {
    let rounds: Vec<serde_json::Value> =
        rounds(demo).into_iter().filter(|r| DAYS.contains(&r["day"].as_i64().unwrap_or(0))).collect();
    let template = include_str!("tournament.html");
    let (Some(a), Some(b)) = (template.find("/*DATA*/"), template.find("/*END*/")) else { return };
    let json = serde_json::Value::Array(rounds).to_string().replace("</", "<\\/");
    let html = format!("{}/*DATA*/{json}{}", &template[..a], &template[b..]);
    let _ = fs::write(Path::new(ROOT).join("tournament.html"), html);
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
        write_page(true);
    }

    #[test]
    fn days_are_the_agreed_ones() {
        assert_eq!(DAYS, [1, 3, 7, 14]);
    }
}
