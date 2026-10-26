//! Notifications only when something is wrong (user 2026-10-16: the popup for every epic drop was annoying; since 2026-10-26 no
//! popups at all, everything goes to the bell in the app, `alerts.rs`). Three situations of a
//! character that does not know what to do, each raised at most once per `REPEAT` per kind:
//!  1. nothing but read-only requests for `IDLE_HOURS` (the character does nothing useful any more),
//!  2. the same command fails with the same error `SAME_ERROR_LIMIT` times in a row (no success of that command in between),
//!  3. an expedition encounter the bot does not know (`unknown`, called from `tavern.rs`; the bot carries on, the user fills in
//!     `src/missions.rs`).
//! Fatal things (crash, mushroom watchdog) have their own popups in `main.rs` / `safe.rs`.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Local};
use sf_api::command::Command;

/// No real action for this long = stuck. Generous on purpose: a character that has finished everything for the day is idle by
/// design, the Arena/Dungeons/hunts normally keep something going every hour or two.
const IDLE_HOURS: i64 = 8;
/// The same command failing with the same error this many times in a row.
const SAME_ERROR_LIMIT: u32 = 10;
/// The same kind of alert is not repeated before this.
const REPEAT: Duration = Duration::hours(6);

#[derive(Default)]
struct State {
    last_action: Option<DateTime<Local>>,
    /// Any request at all; a long gap means the character was switched off (or the PC slept), so idleness starts counting anew.
    last_call: Option<DateTime<Local>>,
    /// "<command> | <error>" → failures in a row
    errors: HashMap<String, u32>,
    raised: Vec<(String, DateTime<Local>)>,
}

static STATE: crate::ctx::PerChar<State> = crate::ctx::PerChar::new();

/// "FightDungeon { .. }" → "FightDungeon".
fn variant(cmd: &Command) -> String {
    format!("{cmd:?}").split([' ', '{', '(']).next().unwrap_or_default().to_string()
}

/// Requests that only read; they do not count as the character doing something.
fn read_only(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Update | Command::UpdateDungeons | Command::HallOfFamePage { .. } | Command::ViewScrapbook | Command::Custom { .. }
    )
}

/// Raises one notification (bell in the app, `alerts.rs`; no popup since 2026-10-26), rate-limited per `kind` and character:
/// `title` is plain Czech for the user, `detail` the technical background.
fn raise(kind: &str, title: &str, detail: &str, repeat: Duration) {
    let now = Local::now();
    let fresh = STATE.lock().is_ok_and(|mut s| {
        if s.raised.iter().any(|(k, t)| k == kind && now - *t < repeat) {
            return false;
        }
        s.raised.retain(|(k, _)| k != kind);
        s.raised.push((kind.to_string(), now));
        true
    });
    if fresh {
        let category = if kind.starts_with("error") { "error" } else if kind == "idle" { "stuck" } else { "uncertain" };
        crate::alerts::raise(category, kind, title, detail);
    }
}

/// The fortress build has been waiting for GOLD alone for hours (the shops and spinning get the gold first, user 2026-10-18):
/// a warning so the user can reconsider the priorities. Once per 12 h per character.
pub fn fortress_gold_short(text: &str) {
    raise(
        "fortress-gold",
        "Stavba v pevnosti už hodiny čeká jen na zlato, protože ho dřív spotřebují obchody a točení; dřevo a kámen jsou připravené.",
        text,
        Duration::hours(12),
    );
}

/// What `after` needs to know about a request (taken before it is sent, the command is moved by then): (variant name, read-only).
pub fn describe(cmd: &Command) -> (String, bool) {
    (variant(cmd), read_only(cmd))
}

/// After every request `safe.rs` sent: bookkeeping for cases 1 and 2.
pub fn after(desc: &(String, bool), result_ok: bool, error: Option<&str>) {
    let now = Local::now();
    let (name, read_only) = (desc.0.clone(), desc.1);
    let mut stuck_error = None;
    let mut stuck_n = None;
    if let Ok(mut s) = STATE.lock() {
        if s.last_call.is_none_or(|t| now - t > Duration::hours(1)) {
            s.last_action = Some(now);
        }
        s.last_call = Some(now);
        if result_ok {
            if !read_only {
                s.last_action = Some(now);
            }
            let prefix = format!("{name} |");
            s.errors.retain(|k, _| !k.starts_with(&prefix));
        } else if let Some(e) = error {
            let n = {
                let n = s.errors.entry(format!("{name} | {e}")).or_insert(0);
                *n += 1;
                *n
            };
            if n >= SAME_ERROR_LIMIT {
                stuck_error = Some(format!("{name} failed {n} times in a row with: {e}"));
                stuck_n = Some(n);
            }
        }
    }
    if let Some(t) = stuck_error {
        raise(
            &format!("error {name}"),
            &format!("Akce „{name}“ opakovaně selhává ({} × po sobě se stejnou chybou serveru), bot to nedokáže obejít.", stuck_n.unwrap_or(0)),
            &t,
            REPEAT,
        );
    }
    let idle = STATE.lock().ok().and_then(|s| s.last_action).is_some_and(|t| now - t >= Duration::hours(IDLE_HOURS));
    if idle {
        raise(
            "idle",
            &format!("Postava už {IDLE_HOURS} hodin neudělala nic užitečného (jen čte data ze serveru), asi se zasekla a neví, co dál."),
            "only read-only requests for the idle limit",
            REPEAT,
        );
    }
}

/// An encounter/situation the bot does not know (one notification per day for all of them: an expedition shows several at once,
/// 2026-10-17). The bot carries on.
pub fn unknown(what: &str) {
    raise(
        "unknown encounter",
        "V expedici se objevila situace, kterou bot nezná. Pokračuje a vybírá podle odhadu, ale nemusí to být nejlepší volba.",
        what,
        Duration::hours(24),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_and_read_only_requests() {
        assert_eq!(variant(&Command::Update), "Update");
        assert_eq!(variant(&Command::FightDungeon { dungeon: sf_api::gamestate::dungeons::Dungeon::Light(sf_api::gamestate::dungeons::LightDungeon::DesecratedCatacombs), use_mushroom: false }), "FightDungeon");
        assert!(read_only(&Command::Update));
        assert!(!read_only(&Command::BuyBeer));
    }
}
