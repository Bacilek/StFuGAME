//! Notifications for the user (bell in the app window, user 2026-10-26): things the bot was unsure about, got stuck on, or that
//! went wrong, in plain Czech, each with a free-text box where the user writes what should ideally have happened (read by Claude
//! when the notifications are gone through later: `roster/notifications.json`).
//! The same alert of the same character (same `key`) is merged into one entry with a counter until the user marks it resolved.

use std::sync::Mutex;

use chrono::Local;
use serde::{Deserialize, Serialize};

const PATH: &str = "roster/notifications.json";
/// Resolved alerts beyond this many are dropped (oldest first); unresolved ones are never dropped.
const KEEP_RESOLVED: usize = 200;

#[derive(Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: u64,
    pub key: String,
    pub character: String,
    /// stuck / error / unknown / uncertain / safety / crash / inventory
    pub kind: String,
    /// What happened, in plain Czech
    pub title: String,
    /// Technical detail (collapsed in the app)
    pub detail: String,
    pub first: String,
    pub last: String,
    pub count: u32,
    /// "new" or "resolved"
    pub status: String,
    /// The user's answer: what should ideally have happened
    pub note: String,
}

static STORE: Mutex<Option<Vec<Alert>>> = Mutex::new(None);

fn locked<R>(save: bool, f: impl FnOnce(&mut Vec<Alert>) -> R) -> R {
    let mut guard = STORE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let list = guard.get_or_insert_with(|| std::fs::read_to_string(PATH).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default());
    let r = f(list);
    if save && let Ok(json) = serde_json::to_string_pretty(&*list) {
        let _ = std::fs::create_dir_all("roster");
        let _ = std::fs::write(PATH, json);
    }
    r
}

/// Run `f` on the list and save it afterwards.
fn with<R>(f: impl FnOnce(&mut Vec<Alert>) -> R) -> R {
    locked(true, f)
}

/// Raises an alert of the current character. `key` identifies the situation (the same one is merged and counted),
/// `title` is what the user reads (Czech, plain), `detail` the technical background.
pub fn raise(kind: &str, key: &str, title: &str, detail: &str) {
    raise_limited(kind, key, title, detail, 0);
}

/// Like `raise`, but an open alert with the same key that was touched less than `cooldown_min` minutes ago is left alone
/// (for situations that are checked again and again, e.g. every loop pass).
pub fn raise_limited(kind: &str, key: &str, title: &str, detail: &str, cooldown_min: i64) {
    let character = crate::ctx::name();
    let now_t = Local::now();
    let now = now_t.format("%Y-%m-%d %H:%M:%S").to_string();
    let recent = |last: &str| {
        chrono::NaiveDateTime::parse_from_str(last, "%Y-%m-%d %H:%M:%S")
            .is_ok_and(|t| now_t.naive_local() - t < chrono::Duration::minutes(cooldown_min))
    };
    if locked(false, |l| l.iter().any(|a| a.character == character && a.key == key && a.status != "resolved" && recent(&a.last))) {
        return;
    }
    report!("[ALERT] {character}: {title} ({detail})");
    with(|list| {
        if let Some(a) = list.iter_mut().find(|a| a.character == character && a.key == key && a.status != "resolved") {
            a.count += 1;
            a.last = now;
            a.title = title.to_string();
            a.detail = detail.to_string();
            return;
        }
        let id = list.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        list.push(Alert {
            id,
            key: key.to_string(),
            character,
            kind: kind.to_string(),
            title: title.to_string(),
            detail: detail.to_string(),
            first: now.clone(),
            last: now,
            count: 1,
            status: "new".to_string(),
            note: String::new(),
        });
        let resolved = list.iter().filter(|a| a.status == "resolved").count();
        if resolved > KEEP_RESOLVED {
            let mut drop = resolved - KEEP_RESOLVED;
            list.retain(|a| {
                let gone = drop > 0 && a.status == "resolved";
                if gone {
                    drop -= 1;
                }
                !gone
            });
        }
    });
}

/// All alerts for the app window, newest activity first.
pub fn json() -> serde_json::Value {
    let mut list = locked(false, |l| l.clone());
    list.sort_by(|a, b| b.last.cmp(&a.last));
    serde_json::to_value(list).unwrap_or_default()
}

pub fn set_note(id: u64, note: &str) {
    with(|l| {
        if let Some(a) = l.iter_mut().find(|a| a.id == id) {
            a.note = note.to_string();
        }
    });
}

pub fn set_status(id: u64, status: &str) {
    if status != "new" && status != "resolved" {
        return;
    }
    with(|l| {
        if let Some(a) = l.iter_mut().find(|a| a.id == id) {
            a.status = status.to_string();
        }
    });
}
