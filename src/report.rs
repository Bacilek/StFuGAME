//! Progress output: to the terminal and, with a timestamp, to logs/progress.log,
//! so the bot can be followed even when it runs in the background.
//! The log keeps only the last `MAX_LINES` messages (user request).

use std::sync::Mutex;

use chrono::Local;

const PATH: &str = "logs/progress.log";
/// How many latest messages a character's progress log keeps (the combined log keeps 3×).
const MAX_LINES: usize = 100;
/// Serialises writes (the icon thread and the bot both log).
static LOCK: Mutex<()> = Mutex::new(());

/// Like `println!`, but also writes to the log. Never put credentials in here.
macro_rules! report {
    ($($arg:tt)*) => { $crate::report::write(&format!($($arg)*)) };
}

pub fn write(msg: &str) {
    // Successes and issues of the day for the character challenge report (roster/)
    crate::roster::observe(msg);
    let time = Local::now().format("%Y-%m-%d %H:%M:%S");
    let who = crate::ctx::name();
    // Combined log of all characters (the character in brackets) + the character's own log
    let combined = if who.is_empty() { format!("{time} {msg}") } else { format!("{time} [{who}] {msg}") };
    println!("{combined}");
    let _guard = LOCK.lock();
    append_keep(PATH, &combined, MAX_LINES * 3);
    if !who.is_empty() {
        append_keep(&crate::ctx::log_path("progress.log"), &format!("{time} {msg}"), MAX_LINES);
    }
}

/// Appends a line and keeps only the last `max` lines of the file.
fn append_keep(path: &str, line: &str, max: usize) {
    let dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("logs"));
    let res = std::fs::create_dir_all(dir).and_then(|()| {
        let old = std::fs::read_to_string(path).unwrap_or_default();
        let mut lines: Vec<&str> = old.lines().collect();
        lines.push(line);
        let keep = &lines[lines.len().saturating_sub(max)..];
        std::fs::write(path, keep.join("\n") + "\n")
    });
    if let Err(err) = res {
        eprintln!("Writing to {path} failed: {err}");
    }
}

/// Money for the log: always gold with two decimals (100 silver = 1 gold, 50 silver = 0.50 g).
pub fn gold(silver: u64) -> String {
    format!("{:.2} g", silver as f64 / 100.0)
}

/// Money change for the log, with a sign (e.g. "+1.90 g").
pub fn gold_change(silver: i64) -> String {
    format!("{:+.2} g", silver as f64 / 100.0)
}

/// A reward amount for the log; silver is shown as gold.
pub fn reward(typ: &str, amount: i64) -> String {
    if typ == "Silver" {
        format!("Gold {:.2}", amount as f64 / 100.0)
    } else {
        format!("{typ} x{amount}")
    }
}
