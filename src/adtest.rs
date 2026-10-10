//! Shop ad refresh (user 2026-10-10, docs/shops.md "Shop refresh for an ad"): the daily "watch an ad → new Weapon/Magic
//! Shop offer". Verified live on TestChar1: free (mushrooms unchanged), the whole offer is replaced.
//! ONLY for `TestChar1` (user: observe it for a few days before any other character gets it):
//! - `daily_refresh` (called by `shops::shop` once spinning has run dry): at most ONE ad per shop per day (user
//!   2026-10-10, resets at midnight), tracked in `roster/TestChar1/ad_refresh.json`, marked BEFORE sending so a
//!   failure never leads to a repeat. Weapon Shop first, then Magic Shop.
//! - Manual experiment: `roster/TestChar1/ad_test` = `probe`/`weapon`/`magic`/`lucky` (one shot per process, file removed
//!   after). `lucky` = the flying TV (Tavern / Dr. Abawuwu): 3 lucky coins for an ad, captured by the user 2026-10-10 as
//!   a single `AdvertisementsCompleted:1` (base64 `MQ==`), no follow-up command.
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
        Some("lucky") => return cmd_name == "AdvertisementsCompleted" && arguments == ["1"],
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

/// The flying TV: pretend to watch the ad, claim it with `AdvertisementsCompleted:1` and log what changed (lucky
/// coins before/after, the raw response). One shot, no follow-up command (user's capture 2026-10-10).
async fn lucky_ad(session: &mut SimpleSession) -> Outcome {
    let coins = |s: &SimpleSession| s.game_state().map_or(-1, |gs| i64::from(gs.specials.wheel.lucky_coins));
    log(&format!("lucky coin ad: ad-related keys of the login response: [{}]", session.login_ad()));
    log(&format!("before: lucky coins {}, {}", coins(session), snapshot(session)));
    let watch = fastrand::u64(11_000..16_000);
    log(&format!("pretending to watch the ad for {} s", watch / 1000));
    tokio::time::sleep(Duration::from_millis(watch)).await;
    match safe::send_raw_only(session, custom("AdvertisementsCompleted", &["1"])).await {
        Ok(raw) => {
            log(&format!("AdvertisementsCompleted:1 → {}", redact(&raw)));
            record_lucky(&raw, coins(session));
        }
        Err(e) => {
            log(&format!("AdvertisementsCompleted:1 failed: {e}"));
            return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
    }
    tokio::time::sleep(Duration::from_millis(fastrand::u64(1500..3500))).await;
    if let Err(e) = safe::send(session, Command::Update).await {
        log(&format!("Update after the ad failed: {e}"));
    }
    log(&format!("after: lucky coins {}, {}", coins(session), snapshot(session)));
    if session.game_state().is_none() { Outcome::SessionLost } else { Outcome::Done }
}

/// One line per lucky coin ad in `roster/TestChar1/logs/lucky_ads.csv` for the week of observation (user 2026-10-10):
/// `date,time,trust_counter,coins_before,coins_in_response`. The coins in the raw response are the 4th field of
/// `resources:` (the state after an `Update` stays stale, see docs/shops.md).
fn record_lucky(raw: &str, coins_before: i64) {
    let field = |key: &str| raw.split('&').find_map(|kv| kv.strip_prefix(key)).unwrap_or("?").to_string();
    let coins_now = field("resources:").split('/').nth(3).unwrap_or("?").to_string();
    let path = crate::ctx::log_path("lucky_ads.csv");
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let now = Local::now();
        let _ = writeln!(f, "{},{},{},{coins_before},{coins_now}", now.format("%Y-%m-%d"), now.format("%H:%M:%S"), field("trust_counter:"));
    }
}

/// Lucky coin ads per day (user 2026-10-10: three a day on TestChar1 only, spread over the day with random jitter,
/// a week of observation of `trust_counter` and the account before anything more).
const LUCKY_ADS_PER_DAY: usize = 3;
/// The day is split into three equal windows from 00:05 to 23:55 (user 2026-10-10); one ad at a random minute of each.
const LUCKY_FROM_MIN: u32 = 5;
const LUCKY_WINDOW_MIN: u32 = 476;
/// End of the allowed day (23:55) and how late a planned ad may be before the rest is re-planned.
const LUCKY_TO_MIN: u32 = 23 * 60 + 55;
const LUCKY_LATE_MIN: u32 = 30;

fn lucky_plan_path() -> std::path::PathBuf {
    dir().join("lucky_ads.json")
}

/// Today's `(planned minutes of the day, ads already claimed)`; a new random plan on a new day.
fn lucky_plan(today: NaiveDate) -> (Vec<u32>, usize) {
    let v: serde_json::Value =
        std::fs::read_to_string(lucky_plan_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    if v["date"].as_str() == Some(&today.to_string())
        && let Some(times) = v["times"].as_array()
    {
        let times: Vec<u32> = times.iter().filter_map(|t| t.as_u64().map(|t| t as u32)).collect();
        if times.len() == LUCKY_ADS_PER_DAY {
            return (times, v["done"].as_u64().unwrap_or(0) as usize);
        }
    }
    let times: Vec<u32> = (0..LUCKY_ADS_PER_DAY as u32)
        .map(|k| fastrand::u32(LUCKY_FROM_MIN + k * LUCKY_WINDOW_MIN..LUCKY_FROM_MIN + (k + 1) * LUCKY_WINDOW_MIN))
        .collect();
    save_lucky_plan(today, &times, 0);
    (times, 0)
}

fn save_lucky_plan(today: NaiveDate, times: &[u32], done: usize) {
    let _ = std::fs::create_dir_all(dir());
    let v = serde_json::json!({ "date": today.to_string(), "times": times, "done": done });
    let _ = std::fs::write(lucky_plan_path(), v.to_string());
}

/// The daily lucky coin ads of `TestChar1`: claims the next planned one when its time has come (at most one per
/// call). Marked as done BEFORE sending, so a failure never leads to a repeat.
async fn daily_lucky(session: &mut SimpleSession) -> Outcome {
    if crate::ctx::name() != CHARACTER {
        return Outcome::Done;
    }
    let now = Local::now();
    let today = now.date_naive();
    let (mut times, done) = lucky_plan(today);
    let minute = chrono::Timelike::hour(&now) * 60 + chrono::Timelike::minute(&now);
    // The bot was off (e.g. started in the evening): spread the ads still missing over the rest of the day instead of
    // claiming them back to back
    if done < times.len() && minute > times[done] + LUCKY_LATE_MIN {
        let left = (times.len() - done) as u32;
        let (from, to) = (minute + 3, LUCKY_TO_MIN.max(minute + 3 + left * 4));
        let window = (to - from) / left;
        for k in 0..left {
            times[done + k as usize] = fastrand::u32(from + k * window..from + (k + 1) * window);
        }
        save_lucky_plan(today, &times, done);
        log(&format!("late start: the {left} remaining ad(s) re-planned for minutes {:?} of the day", &times[done..]));
        return Outcome::Done;
    }
    if done >= times.len() || minute < times[done] {
        return Outcome::Done;
    }
    save_lucky_plan(today, &times, done + 1);
    log(&format!("daily lucky coin ad {}/{LUCKY_ADS_PER_DAY} (planned for minute {} of the day)", done + 1, times[done]));
    if let Ok(mut a) = ACTIVE.lock() {
        *a = Some("lucky");
    }
    let outcome = lucky_ad(session).await;
    if let Ok(mut a) = ACTIVE.lock() {
        *a = None;
    }
    outcome
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
    if let Outcome::SessionLost = daily_lucky(session).await {
        return Outcome::SessionLost;
    }
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
        "lucky" => lucky_ad(session).await,
        _ => {
            log(&format!("probe: ad-related keys of the login response: [{}]", session.login_ad()));
            Outcome::Done
        }
    };
    let _ = std::fs::remove_file(flag_path());
    outcome
}
