//! Pets (unlock: level 75 + one Tavern expedition, `docs/pets.md`), phase 1: READ ONLY (user 2026-10-21). sf-api drops the whole
//! pets state while `rank` (index 233) is 0, which is the case right after the unlock, and its later indexes may be shifted on this
//! server (the first pasted `ownpets.petsSave` had its non-zero tail at 242..275 instead of 231..258). So the raw numbers are kept
//! (`SimpleSession::pets_save`), written to `roster/<character>/pets_raw.txt` and summarised in the log whenever they change.

use crate::session::SimpleSession;

const HABITATS: [&str; 5] = ["Shadow", "Light", "Earth", "Fire", "Water"];

/// Level of pet `n` (1..=100) is at index `n + 1`, the number of collected pets at 103 (matches sf-api and the first capture).
fn summary(nums: &[i64]) -> String {
    let collected = nums.get(103).copied().unwrap_or(-1);
    let mut parts = Vec::new();
    for (h, name) in HABITATS.iter().enumerate() {
        let owned: Vec<String> = (0..20)
            .filter_map(|p| {
                let level = nums.get(h * 20 + p + 2).copied().unwrap_or(0);
                (level > 0).then(|| format!("#{}={level}", p + 1))
            })
            .collect();
        if !owned.is_empty() {
            parts.push(format!("{name} [{}]", owned.join(" ")));
        }
    }
    format!("collected {collected}: {}", if parts.is_empty() { "none".to_string() } else { parts.join(", ") })
}

/// Non-zero numbers from index 200 on as `index:value`, to learn the layout of the tail (timers, opponent, fruits per day).
fn tail(nums: &[i64]) -> String {
    nums.iter().enumerate().skip(200).filter(|(_, v)| **v != 0).map(|(i, v)| format!("{i}:{v}")).collect::<Vec<_>>().join(" ")
}

pub fn observe(session: &SimpleSession) {
    let Some(nums) = session.pets_save() else { return };
    static LAST: crate::ctx::PerChar<Vec<i64>> = crate::ctx::PerChar::new();
    let Ok(mut last) = LAST.lock() else { return };
    if *last == nums {
        return;
    }
    *last = nums.to_vec();
    report!("[pets] {} | tail {}", summary(nums), tail(nums));
    let path = format!("roster/{}/pets_raw.txt", crate::ctx::name());
    let line = nums.iter().map(ToString::to_string).collect::<Vec<_>>().join("/");
    let _ = std::fs::write(path, line);
}
