//! Shop ad refresh (user 2026-10-10, docs/shops.md "Shop refresh for an ad"): the daily "watch an ad → new Weapon/Magic
//! Shop offer". Verified live on TestChar1: free (mushrooms unchanged), the whole offer is replaced.
//! ONLY for `TestChar1` (user: observe it for a few days before any other character gets it):
//! - `daily_refresh` (called by `shops::shop` once spinning has run dry): at most ONE ad per shop per day (user
//!   2026-10-10, resets at midnight), tracked in `roster/TestChar1/ad_refresh.json`, marked BEFORE sending so a
//!   failure never leads to a repeat. Weapon Shop first, then Magic Shop.
//! - Manual experiment: `roster/TestChar1/ad_test` = `probe`/`weapon`/`magic` (one shot per process, file removed after).
//! Each refresh pretends to watch the ad (11–16 s), then sends `AdvertisementsCompleted:<4|5>` and
//! `PlayerNewWares:<1|2>/2`. Everything goes to `roster/TestChar1/logs/ad_test.log` (session keys redacted).

use std::{io::Write, time::Duration};

use chrono::{Local, NaiveDate};
use sf_api::command::Command;

use crate::{inventory, safe, session::SimpleSession, tavern::Outcome};

const CHARACTER: &str = "TestChar1";

static TRIED: crate::ctx::PerChar<bool> = crate::ctx::PerChar::new();
/// The shop whose ad refresh is in progress right now (`weapon`/`magic`), set only inside `refresh`.
static ACTIVE: crate::ctx::PerChar<Option<&'static str>> = crate::ctx::PerChar::new();

/// What `shops::shop` should do after `daily_refresh`.
pub enum Refresh {
    /// Nothing happened (not this character, or both shops already refreshed today)
    NotApplicable,
    /// A shop was refreshed: new offer, look at it again
    Refreshed,
    SessionLost,
}

fn dir() -> std::path::PathBuf {
    std::path::Path::new("roster").join(CHARACTER)
}

fn flag_path() -> std::path::PathBuf {
    dir().join("ad_test")
}

fn marker_path() -> std::path::PathBuf {
    dir().join("ad_refresh.json")
}

/// Content of the manual flag file (`probe`/`weapon`/`magic`), only for the test character.
fn flag() -> Option<String> {
    if crate::ctx::name() != CHARACTER {
        return None;
    }
    std::fs::read_to_string(flag_path()).ok().map(|s| s.trim().to_string())
}

/// Which shop is allowed to send ad commands right now.
fn current_mode() -> Option<String> {
    if crate::ctx::name() != CHARACTER {
        return None;
    }
    let active = ACTIVE.lock().ok().and_then(|a| *a);
    active.map(str::to_string).or_else(flag)
}

/// Whitelist for `safe::custom_allowed`: exactly the two commands of the shop being refreshed, nothing else.
pub fn custom_allowed(cmd_name: &str, arguments: &[String]) -> bool {
    let (ad, shop) = match current_mode().as_deref() {
        Some("weapon") => ("4", "1"),
        Some("magic") => ("5", "2"),
        _ => return false,
    };
    match (cmd_name, arguments) {
        ("AdvertisementsCompleted", [a]) => a == ad,
        ("PlayerNewWares", [s, paid]) => s == shop && paid == "2",
        _ => false,
    }
}

/// Mushrooms the ad refresh may cost (the user accepts one, expected to be free).
pub fn allowed_spend(cmd: &Command) -> u32 {
    match cmd {
        Command::Custom { cmd_name, arguments } if cmd_name == "PlayerNewWares" && custom_allowed(cmd_name, arguments) => 1,
        _ => 0,
    }
}

fn custom(cmd_name: &str, arguments: &[&str]) -> Command {
    Command::Custom { cmd_name: cmd_name.to_string(), arguments: arguments.iter().map(|a| (*a).to_string()).collect() }
}

fn log(msg: &str) {
    report!("[adtest] {msg}");
    let path = crate::ctx::log_path("ad_test.log");
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {msg}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// A raw response without session keys, every value cut short.
fn redact(raw: &str) -> String {
    raw.split('&')
        .filter(|kv| !["sessionid", "cryptokey", "cryptoid"].iter().any(|k| kv.starts_with(&format!("{k}:"))))
        .map(|kv| kv.chars().take(160).collect::<String>())
        .collect::<Vec<_>>()
        .join(" & ")
}

fn offer(session: &SimpleSession, shop_name: &str) -> String {
    let Some(gs) = session.game_state() else { return "no state".into() };
    gs.shops
        .values()
        .flat_map(|s| s.iter())
        .filter(|(pos, _)| format!("{:?}", pos.typ) == shop_name)
        .map(|(_, i)| format!("{} ({}g/{}m)", inventory::detail(i), i.price, i.mushroom_price))
        .collect::<Vec<_>>()
        .join("; ")
}

fn snapshot(session: &SimpleSession) -> String {
    session
        .game_state()
        .map_or("no state".into(), |gs| format!("mushrooms {}, silver {}", gs.character.mushrooms, gs.character.silver))
}

/// Pretends to watch the ad of one shop and refreshes it. `mode` = `weapon`/`magic`; the caller made sure the
/// whitelist allows it (`current_mode`). Never retries.
async fn refresh(session: &mut SimpleSession, mode: &'static str) -> Outcome {
    let (shop_name, ad, shop) = if mode == "weapon" { ("Weapon", "4", "1") } else { ("Magic", "5", "2") };
    if let Ok(mut a) = ACTIVE.lock() {
        *a = Some(mode);
    }
    let outcome = refresh_inner(session, shop_name, ad, shop).await;
    if let Ok(mut a) = ACTIVE.lock() {
        *a = None;
    }
    outcome
}

async fn refresh_inner(session: &mut SimpleSession, shop_name: &str, ad: &str, shop: &str) -> Outcome {
    log(&format!("{shop_name} Shop: ad-related keys of the login response: [{}]", session.login_ad()));
    log(&format!("before: {} | {shop_name} Shop: {}", snapshot(session), offer(session, shop_name)));

    // Pretend to watch the ad like a human would (they last about 10 s), then claim it
    let watch = fastrand::u64(11_000..16_000);
    log(&format!("pretending to watch the ad for {} s", watch / 1000));
    tokio::time::sleep(Duration::from_millis(watch)).await;

    match safe::send_raw_only(session, custom("AdvertisementsCompleted", &[ad])).await {
        Ok(raw) => log(&format!("AdvertisementsCompleted:{ad} → {}", redact(&raw))),
        Err(e) => {
            log(&format!("AdvertisementsCompleted:{ad} failed: {e}"));
            return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
    }
    tokio::time::sleep(Duration::from_millis(fastrand::u64(1500..3500))).await;

    match safe::send_raw(session, custom("PlayerNewWares", &[shop, "2"])).await {
        Ok(raw) => log(&format!("PlayerNewWares:{shop}/2 → {}", redact(&raw))),
        Err(e) => log(&format!("PlayerNewWares:{shop}/2 failed: {e}")),
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        log(&format!("Update after the refresh failed: {e}"));
    }
    log(&format!("after: {} | {shop_name} Shop: {}", snapshot(session), offer(session, shop_name)));
    if session.game_state().is_none() { Outcome::SessionLost } else { Outcome::Done }
}

/// `{date, weapon, magic}`: which shops already used today's ad.
fn used_today(today: NaiveDate) -> (bool, bool) {
    let v: serde_json::Value =
        std::fs::read_to_string(marker_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    if v["date"].as_str() != Some(&today.to_string()) {
        return (false, false);
    }
    (v["weapon"].as_bool().unwrap_or(false), v["magic"].as_bool().unwrap_or(false))
}

/// Records that today's ad of this shop (`weapon`/`magic`) is used.
fn mark_used(mode: &str) {
    let today = Local::now().date_naive();
    let (w, m) = used_today(today);
    let (weapon, magic) = (w || mode == "weapon", m || mode == "magic");
    let _ = std::fs::create_dir_all(dir());
    let v = serde_json::json!({ "date": today.to_string(), "weapon": weapon, "magic": magic });
    let _ = std::fs::write(marker_path(), v.to_string());
}

/// Daily ad refresh for `TestChar1`, called when spinning ran dry (every item costs mushrooms or gold ran out):
/// the next shop that has not used today's ad yet (Weapon, then Magic) – one per call. The shop pass looks at the
/// new offer afterwards and may call this again for the other shop.
pub async fn daily_refresh(session: &mut SimpleSession) -> Refresh {
    if crate::ctx::name() != CHARACTER {
        return Refresh::NotApplicable;
    }
    let today = Local::now().date_naive();
    let (weapon, magic) = used_today(today);
    let mode = if !weapon {
        "weapon"
    } else if !magic {
        "magic"
    } else {
        return Refresh::NotApplicable;
    };
    // Marked BEFORE sending: whatever happens, never a second try the same day
    mark_used(mode);
    log(&format!("daily ad refresh: {mode} shop"));
    match refresh(session, mode).await {
        Outcome::SessionLost => Refresh::SessionLost,
        Outcome::Done => Refresh::Refreshed,
    }
}

/// Manual one-shot experiment behind the flag file (see the module docs).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(mode) = flag() else { return Outcome::Done };
    if TRIED.lock().map_or(true, |mut t| std::mem::replace(&mut *t, true)) {
        return Outcome::Done;
    }
    let outcome = match mode.as_str() {
        "weapon" | "magic" => {
            let mode: &'static str = if mode == "weapon" { "weapon" } else { "magic" };
            mark_used(mode);
            refresh(session, mode).await
        }
        _ => {
            log(&format!("probe: ad-related keys of the login response: [{}]", session.login_ad()));
            Outcome::Done
        }
    };
    let _ = std::fs::remove_file(flag_path());
    outcome
}
