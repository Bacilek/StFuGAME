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
pub fn start(rt: &Handle, name: &str, delay_secs: u64) {
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

/// Stops a character (aborts its task at the next await point).
pub fn stop(name: &str) {
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
                serde_json::json!({ "name": name, "state": state.as_str(), "now": now })
            })
            .collect(),
    )
}
