//! Progress output: to the terminal and, with a timestamp, to logs/progress.log,
//! so the bot can be followed even when it runs in the background.
//! The log keeps only the last `MAX_LINES` messages (user request).

use std::sync::Mutex;

use chrono::Local;

const PATH: &str = "logs/progress.log";
/// How many latest messages the progress log keeps.
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
    let line = format!("{} {msg}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    println!("{line}");
    let _guard = LOCK.lock();
    let res = std::fs::create_dir_all("logs").and_then(|()| {
        let old = std::fs::read_to_string(PATH).unwrap_or_default();
        let mut lines: Vec<&str> = old.lines().collect();
        lines.push(&line);
        let keep = &lines[lines.len().saturating_sub(MAX_LINES)..];
        std::fs::write(PATH, keep.join("\n") + "\n")
    });
    if let Err(err) = res {
        eprintln!("Writing to {PATH} failed: {err}");
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
