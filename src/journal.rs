//! Deník expedic: každá dokončená expedice se zapíše jako jeden řádek JSON do logs/expedice.jsonl.
//! Slouží k vyhodnocení strategie (cíl: přesně ~40 hrdinství a co nejvíc klíčů/truhel).

use std::{fs::OpenOptions, io::Write};

use chrono::Local;
use serde_json::json;

const PATH: &str = "logs/expedice.jsonl";
/// Kolik nad 40 ještě bereme jako úspěch. Víc znamená, že jsme hnali hrdinství místo truhel.
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
    /// Setkání, která ještě nemáme v docs/expedice.md
    pub unmapped: Vec<String>,
    /// Výsledky ověření dat misí (OK / NESEDÍ)
    pub checks: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Journal {
    pub current: Option<Entry>,
}

impl Journal {
    /// Vrátí rozpracovaný záznam, případně založí nový.
    pub fn entry(&mut self, mission: &str) -> &mut Entry {
        self.current.get_or_insert_with(|| Entry {
            started: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            mission: mission.to_string(),
            ..Entry::default()
        })
    }

    /// Uzavře expedici, vypíše shrnutí a zapíše ho do deníku.
    pub fn finish(&mut self) {
        let Some(e) = self.current.take() else { return };
        let verdict = if e.projected < 40 {
            "neúspěch: pod 40"
        } else if e.projected > 40 + OVERSHOOT_TOLERANCE {
            "nejspíš přehnané hrdinství"
        } else {
            "úspěch"
        };
        report!(
            "[deník] {}: hrdinství {} (odhad na konci {}), úkol {}/{}, klíče {}, truhly {} → {verdict}",
            e.mission, e.heroism, e.projected, e.target_current, e.target_amount, e.keys, e.chests
        );
        let bad = e.checks.iter().filter(|c| c.starts_with("NESEDÍ")).count();
        report!("[deník] Kontroly dat: {} OK, {bad} nesedí", e.checks.len() - bad);
        if !e.unmapped.is_empty() {
            report!("[deník] Nezmapovaná setkání: {}", e.unmapped.join(", "));
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
        });
        let res = std::fs::create_dir_all("logs").and_then(|()| {
            let mut f = OpenOptions::new().create(true).append(true).open(PATH)?;
            writeln!(f, "{line}")
        });
        if let Err(err) = res {
            report!("[deník] Zápis do {PATH} selhal: {err}");
        }
    }
}
