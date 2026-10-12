//! "Attack me" mode (user 2026-10-12): while the switch in the app window is on, every bot character looks up the user's
//! main character `Bacilek31` in the Hall of Fame and attacks him (`hunt::Hunt::AttackMe`, ahead of every other hunt and
//! of the Arena XP fights). Goal: his honor and rank sink, so he fights among weaker opponents and can reach the
//! achievement for 25 / 50 / 100 Arena fights won in a row (he takes his weapon off and loses on purpose). These are
//! Hall of Fame fights by name: they use the Arena cooldown but do NOT count towards the 10 XP wins a day.
//! The switch is remembered across restarts in `roster/attack_me.txt` (the file exists = on).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

/// The character the bots attack.
pub const TARGET: &str = "Bacilek31";

static ON: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

const FILE: &str = "roster/attack_me.txt";

/// Is the mode on? (read from the file once at the first call)
pub fn active() -> bool {
    INIT.call_once(|| ON.store(std::path::Path::new(FILE).exists(), Ordering::SeqCst));
    ON.load(Ordering::SeqCst)
}

pub fn set(on: bool) {
    INIT.call_once(|| {});
    ON.store(on, Ordering::SeqCst);
    if on {
        let _ = std::fs::create_dir_all("roster");
        let _ = std::fs::write(FILE, TARGET);
    } else {
        let _ = std::fs::remove_file(FILE);
    }
    report!("[attack-me] {} – the bots {} {TARGET}", if on { "ON" } else { "OFF" }, if on { "attack" } else { "no longer prefer" });
}

/// The opponent to prefer for the current character (never itself).
pub fn target() -> Option<&'static str> {
    (active() && !crate::ctx::name().eq_ignore_ascii_case(TARGET)).then_some(TARGET)
}
