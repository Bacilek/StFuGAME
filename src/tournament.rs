//! Round-robin tournament of the challenge characters (user 2026-10-07): at the end of day 1, 3, 7 and 14 every
//! character "fights" every other one. Simulated with sf-api's fight simulator (the server's rules) `ITERATIONS` times
//! per pair, so the result is a fair win rate instead of one random fight – no Arena cooldown, no honor lost.
//! Participants = the characters in `roster/roster.md`, loaded via `ViewPlayer` (the bot's own character from its
//! own state). Output: `roster/tournament/day-<n>.json` + `roster/tournament.html` (all rounds).

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

/// Today's challenge day if a tournament is due today and not done yet.
pub fn due_today() -> Option<i64> {
    let today = Local::now().date_naive();
    let day = (today - start_date().unwrap_or(today)).num_days() + 1;
    let done = Path::new(ROOT).join("tournament").join(format!("day-{day}.json")).exists();
    (DAYS.contains(&day) && !done).then_some(day)
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
    let _ = fs::write(dir.join(format!("day-{day}.json")), round.to_string());
    write_page();
    report!("[tournament] Day {day} done ({n} characters, {} missing)", missing.len());
    Outcome::Done
}

/// `roster/tournament.html` from all rounds (template src/tournament.html).
pub fn write_page() {
    let dir = Path::new(ROOT).join("tournament");
    let mut rounds: Vec<serde_json::Value> = fs::read_dir(&dir)
        .map(|it| {
            it.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter_map(|e| serde_json::from_str(&fs::read_to_string(e.path()).ok()?).ok())
                .collect()
        })
        .unwrap_or_default();
    rounds.sort_by_key(|r| r["day"].as_i64().unwrap_or(0));
    let template = include_str!("tournament.html");
    let (Some(a), Some(b)) = (template.find("/*DATA*/"), template.find("/*END*/")) else { return };
    let json = serde_json::Value::Array(rounds).to_string().replace("</", "<\\/");
    let html = format!("{}/*DATA*/{json}{}", &template[..a], &template[b..]);
    let _ = fs::write(Path::new(ROOT).join("tournament.html"), html);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Demo page with made-up win rates: `cargo test demo_tournament -- --ignored`.
    #[test]
    #[ignore]
    fn demo_tournament() {
        let players = participants();
        let n = players.len();
        let dir = Path::new(ROOT).join("tournament");
        let _ = fs::create_dir_all(&dir);
        let mut rng = fastrand::Rng::with_seed(3);
        let skill: Vec<f64> = (0..n).map(|_| rng.f64()).collect();
        for day in DAYS {
            let mut win = vec![vec![None; n]; n];
            for i in 0..n {
                for j in (i + 1)..n {
                    let d = (skill[i] - skill[j]) * (1.0 + day as f64 / 7.0) + (rng.f64() - 0.5) * 0.3;
                    let p = (0.5 + d).clamp(0.02, 0.98);
                    win[i][j] = Some((p * 1000.0).round() / 1000.0);
                    win[j][i] = Some(((1.0 - p) * 1000.0).round() / 1000.0);
                }
            }
            let round = serde_json::json!({
                "day": day, "date": format!("demo day {day}"), "iterations": ITERATIONS,
                "players": players.iter().map(|(n, c)| serde_json::json!({"nick": n, "cls": c})).collect::<Vec<_>>(),
                "win": win, "missing": [],
            });
            let _ = fs::write(dir.join(format!("_demo-day-{day}.json")), round.to_string());
        }
        write_page();
        // the demo files would mix with the real rounds: remove them again, the page stays
        for day in DAYS {
            let _ = fs::remove_file(dir.join(format!("_demo-day-{day}.json")));
        }
    }

    #[test]
    fn days_are_the_agreed_ones() {
        assert_eq!(DAYS, [1, 3, 7, 14]);
    }
}
