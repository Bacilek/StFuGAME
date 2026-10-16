//! Raw ids of encounters sf-api cannot name. sf-api turns an unknown encounter id into `ExpeditionThing::Unknown`
//! and only logs "Unknown encounter: <id>" (via the `log` crate). This logger keeps those ids per character so
//! the Tavern can write them to the journal; with them a new (e.g. seasonal) mission can be mapped.

use std::{collections::HashMap, sync::Mutex};

/// Most recent unknown ids per character (oldest first), capped so it never grows.
static IDS: Mutex<Option<HashMap<String, Vec<i64>>>> = Mutex::new(None);
const KEEP: usize = 16;

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() == log::Level::Warn
    }

    fn log(&self, r: &log::Record) {
        let text = r.args().to_string();
        let Some(id) = text.strip_prefix("Unknown encounter: ").and_then(|s| s.trim().parse::<i64>().ok()) else {
            return;
        };
        if let Ok(mut g) = IDS.lock() {
            let v = g.get_or_insert_with(HashMap::new).entry(crate::ctx::name()).or_default();
            v.push(id);
            if v.len() > KEEP {
                v.remove(0);
            }
        }
    }

    fn flush(&self) {}
}

/// Installs the logger once (a second call is harmless).
pub fn init() {
    if log::set_logger(&Capture).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
}

/// The last `n` unknown ids seen by the current character (in the order the server sent them), and forgets them all.
pub fn take_last(n: usize) -> Vec<i64> {
    let Ok(mut g) = IDS.lock() else { return Vec::new() };
    let Some(v) = g.as_mut().and_then(|m| m.get_mut(&crate::ctx::name())) else { return Vec::new() };
    let start = v.len().saturating_sub(n);
    let out = v[start..].to_vec();
    v.clear();
    out
}
