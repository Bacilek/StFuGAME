//! Expedition journal: every finished expedition is written as one JSON line to logs/expeditions.jsonl.
//! Used to evaluate the strategy (goal: ~40 heroism exactly and as many keys/chests as possible).

use std::{fs::OpenOptions, io::Write};

use chrono::Local;
use serde_json::json;

const PATH: &str = "logs/expeditions.jsonl";
/// How far above 40 still counts as a success. More (together with a declined key/chest) means
/// we chased heroism instead of chests.
pub const OVERSHOOT_TOLERANCE: i32 = 5;

#[derive(Debug, Default)]
pub struct Entry {
    pub started: String,
    pub mission: String,
    pub target_current: u8,
    pub target_amount: u8,
    pub heroism: i32,
    pub projected: i32,
    pub keys: u32,
    pub chests: u32,
    pub picks: Vec<String>,
    pub rewards: Vec<String>,
    /// Encounters not yet documented in docs/expeditions.md
    pub unmapped: Vec<String>,
    /// Results of mission data checks (OK / MISMATCH)
    pub checks: Vec<String>,
    /// How many times a key/chest was offered with 40 secured and the bot did not take it
    pub declined_resources: u32,
}

#[derive(Debug, Default)]
pub struct Journal {
    pub current: Option<Entry>,
}

impl Journal {
    /// Returns the entry in progress, or starts a new one.
    pub fn entry(&mut self, mission: &str) -> &mut Entry {
        self.current.get_or_insert_with(|| Entry {
            started: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            mission: mission.to_string(),
            ..Entry::default()
        })
    }

    /// Closes the expedition, prints a summary and writes it to the journal.
    pub fn finish(&mut self) {
        let Some(e) = self.current.take() else { return };
        let verdict = if e.projected < 40 {
            "failure: below 40"
        } else if e.projected > 40 + OVERSHOOT_TOLERANCE && e.declined_resources > 0 {
            "probably too much heroism"
        } else {
            "success"
        };
        report!(
            "[journal] {}: heroism {} (projected at end {}), task {}/{}, keys {}, chests {} → {verdict}",
            e.mission, e.heroism, e.projected, e.target_current, e.target_amount, e.keys, e.chests
        );
        let bad = e.checks.iter().filter(|c| c.starts_with("MISMATCH")).count();
        report!("[journal] Data checks: {} OK, {bad} mismatched", e.checks.len() - bad);
        if !e.unmapped.is_empty() {
            report!("[journal] Unmapped encounters: {}", e.unmapped.join(", "));
        }
        let line = json!({
            "started": e.started,
            "finished": Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            "mission": e.mission,
            "target": format!("{}/{}", e.target_current, e.target_amount),
            "heroism": e.heroism,
            "projected": e.projected,
            "keys": e.keys,
            "chests": e.chests,
            "verdict": verdict,
            "picks": e.picks,
            "rewards": e.rewards,
            "unmapped": e.unmapped,
            "checks": e.checks,
            "declined_resources": e.declined_resources,
        });
        let res = std::fs::create_dir_all("logs").and_then(|()| {
            let mut f = OpenOptions::new().create(true).append(true).open(PATH)?;
            writeln!(f, "{line}")
        });
        if let Err(err) = res {
            report!("[journal] Writing to {PATH} failed: {err}");
        }
    }
}
