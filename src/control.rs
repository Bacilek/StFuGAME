//! Starting, stopping and the status of individual characters from the app window (user 2026-10-08):
//! the tray icon's Start/Stop toggles everyone, the app window's tiles toggle one character at a time.
//! Stopping aborts that character's task at its next `.await` (safe: next login re-reads a fresh state,
//! nothing stays half-done on the server).

use std::sync::Mutex;

use tokio::{runtime::Handle, task::AbortHandle, time::Duration};

use crate::Credentials;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CharState {
    Stopped,
    Running,
    Error,
}

impl CharState {
    fn as_str(self) -> &'static str {
        match self {
            CharState::Stopped => "stopped",
            CharState::Running => "running",
            CharState::Error => "error",
        }
    }
}

struct Entry {
    name: String,
    creds: Credentials,
    state: CharState,
    abort: Option<AbortHandle>,
}

static REGISTRY: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

/// Remembers the user's last on/off choice per character, across app restarts (user 2026-10-08: opening the app
/// to just look at stats should never force-start a character the user left switched off).
const STATE_PATH: &str = "roster/switches.json";

fn load_switches() -> std::collections::HashMap<String, bool> {
    std::fs::read_to_string(STATE_PATH).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_switch(name: &str, on: bool) {
    let mut map = load_switches();
    map.insert(name.to_string(), on);
    let _ = std::fs::create_dir_all("roster");
    if let Ok(json) = serde_json::to_string_pretty(&map) {
        let _ = std::fs::write(STATE_PATH, json);
    }
}

/// Should this character autostart when the app launches? Only if the user previously, explicitly switched it
/// on (saved by every start/stop, individual or bulk) – unknown/never-touched characters default to off.
pub fn should_autostart(name: &str) -> bool {
    load_switches().get(name).copied().unwrap_or(false)
}

fn lock() -> std::sync::MutexGuard<'static, Vec<Entry>> {
    REGISTRY.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Registers every known account (stopped). Safe to call again; keeps existing entries and their state.
pub fn register(accounts: &[Credentials]) {
    let mut r = lock();
    for c in accounts {
        if !r.iter().any(|e| e.name == c.character) {
            r.push(Entry { name: c.character.clone(), creds: c.clone(), state: CharState::Stopped, abort: None });
        }
    }
}

/// Characters in registration order.
/// How many characters are currently running (for `ctx::request_end_of_day`'s progress counter).
pub fn running_names() -> Vec<String> {
    lock().iter().filter(|e| e.state == CharState::Running).map(|e| e.name.clone()).collect()
}

pub fn names() -> Vec<String> {
    lock().iter().map(|e| e.name.clone()).collect()
}

fn set_state(name: &str, s: CharState) {
    if let Some(e) = lock().iter_mut().find(|e| e.name == name) {
        e.state = s;
    }
}

/// Is this character currently spawned (regardless of whether its login succeeded yet)?
pub fn is_running(name: &str) -> bool {
    lock().iter().any(|e| e.name == name && e.abort.is_some())
}

/// Starts a character (does nothing if already running): spawns `run_character` on `rt`, scoped to `name`
/// via `ctx::CHARACTER`, optionally after `delay_secs` (used for not logging everyone in at once).
/// Remembers this as the character's switch position for the next app launch.
pub fn start(rt: &Handle, name: &str, delay_secs: u64) {
    save_switch(name, true);
    if is_running(name) {
        return;
    }
    let Some(creds) = lock().iter().find(|e| e.name == name).map(|e| e.creds.clone()) else { return };
    let n = name.to_string();
    let handle = rt.spawn(crate::ctx::CHARACTER.scope(n.clone(), async move {
        if delay_secs > 0 {
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        }
        crate::run_character(creds).await;
        crate::alerts::raise(
            "crash",
            "character-ended",
            "Postava se sama vypnula (přihlášení selhalo nebo se příliš často ztrácela relace). Bot ji nespustí znovu, dokud ji ručně nezapnete.",
            "run_character returned",
        );
        // Ended on its own (too many relogins, bad credentials, …): free the slot so Start can retry
        set_state(&n, CharState::Error);
        if let Some(e) = lock().iter_mut().find(|e| e.name == n) {
            e.abort = None;
        }
    }));
    if let Some(e) = lock().iter_mut().find(|e| e.name == name) {
        e.abort = Some(handle.abort_handle());
        e.state = CharState::Running;
    }
}

/// Stops a character (aborts its task at the next await point). Remembers this for the next app launch.
pub fn stop(name: &str) {
    save_switch(name, false);
    if let Some(e) = lock().iter_mut().find(|e| e.name == name) {
        if let Some(h) = e.abort.take() {
            h.abort();
        }
        e.state = CharState::Stopped;
    }
}

pub fn stop_all() {
    for n in names() {
        stop(&n);
    }
}

pub fn start_all(rt: &Handle) {
    for (i, n) in names().into_iter().enumerate() {
        // Do not log in all at once (human-like, server-friendly)
        let delay = if i == 0 { 0 } else { i as u64 * 20 + fastrand::u64(0..20) };
        start(rt, &n, delay);
    }
}

/// Overall state for the tray icon.
pub fn overall() -> crate::tray::State {
    let states: Vec<CharState> = lock().iter().map(|e| e.state).collect();
    if states.contains(&CharState::Running) {
        crate::tray::State::Running
    } else if !states.is_empty() && states.iter().all(|s| *s == CharState::Error) {
        crate::tray::State::Failed
    } else {
        crate::tray::State::Stopped
    }
}

/// "A slot is free in <guild>" button (user 2026-10-26): pins the running character with the lowest level that is not in that
/// guild (and not already pinned) to it via `roster/<name>/guild_pin.txt` and wakes the main loops, so `guild::run` joins at once.
/// Returns the chosen character (name, level).
pub fn pin_lowest_level(guild: &str) -> Option<(String, u64)> {
    let (name, level) = running_names()
        .into_iter()
        .filter(|n| !std::path::Path::new(&format!("roster/{n}/guild_pin.txt")).exists())
        .filter_map(|n| {
            let now = std::fs::read_to_string(format!("roster/{n}/now.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())?;
            if now["guild"].as_str() == Some(guild) {
                return None;
            }
            Some((n, now["level"].as_u64()?))
        })
        .min_by_key(|(_, l)| *l)?;
    std::fs::write(format!("roster/{name}/guild_pin.txt"), guild).ok()?;
    crate::ctx::WAKE.notify_waiters();
    Some((name, level))
}

/// Status of every registered character + its latest snapshot (`roster/<name>/now.json`), for the app window.
pub fn status_json() -> serde_json::Value {
    let entries: Vec<(String, CharState)> = lock().iter().map(|e| (e.name.clone(), e.state)).collect();
    serde_json::Value::Array(
        entries
            .into_iter()
            .map(|(name, state)| {
                let now = std::fs::read_to_string(format!("roster/{name}/now.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .unwrap_or(serde_json::Value::Null);
                let fortress = std::fs::read_to_string(format!("roster/{name}/fortress.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .unwrap_or(serde_json::Value::Null);
                let pets = std::fs::read_to_string(format!("roster/{name}/pets_raw.txt"))
                    .ok()
                    .map(|t| t.trim().split('/').filter_map(|s| s.parse::<i64>().ok()).collect::<Vec<i64>>())
                    .filter(|n| n.len() > 250)
                    .map_or(serde_json::Value::Null, |n| crate::pets::overview(&n));
                let quests = std::fs::read_to_string(format!("roster/{name}/quests.json"))
                    .ok()
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .unwrap_or(serde_json::Value::Null);
                serde_json::json!({ "name": name, "state": state.as_str(), "now": now, "quests": quests, "fortress": fortress, "pets": pets })
            })
            .collect(),
    )
}

/// A short message for the app window's toast (a button click needs visible feedback, user 2026-10-26): numbered, so the page shows each once.
static NOTICE: Mutex<(u64, String)> = Mutex::new((0, String::new()));

pub fn notice(msg: &str) {
    let mut n = NOTICE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *n = (n.0 + 1, msg.to_string());
}

pub fn notice_json() -> serde_json::Value {
    let n = NOTICE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    serde_json::json!({ "seq": n.0, "msg": n.1 })
}
