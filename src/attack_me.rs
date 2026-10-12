//! "Attack me" mode (user 2026-10-12): the user's main character `Bacilek31` is the preferred Arena / Hall of Fame
//! opponent of every bot character while the switch in the app window is on. Goal: the achievement for 25 / 50 / 100 Arena
//! fights won in a row – the user keeps Bacilek31's honor low and takes his weapon off so he loses on purpose.
//! The switch is remembered across restarts in `roster/attack_me.txt` (the file exists = on). Quests and events keep their
//! priority: a "win against <class>" task picks its own opponent, hunts for fight tasks run before the Arena.

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
