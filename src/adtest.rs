//! EXPERIMENT (user 2026-10-10, docs/shops.md "Shop refresh for an ad"): the daily "watch an ad → new Weapon/Magic Shop
//! offer". Runs ONLY on `TestChar1` and ONLY when `roster/TestChar1/ad_test` exists; its content picks the step:
//! - `probe`  – logs the ad-related keys of the login response (`skipvideo` …), sends nothing,
//! - `weapon` / `magic` – simulates watching the ad (waits ~10–15 s like the real one), then sends
//!   `AdvertisementsCompleted:<4|5>` and `PlayerNewWares:<1|2>/2` (the ad refresh) and logs mushrooms, gold and the
//!   shop offer before/after, so it is visible whether the refresh was free (the user accepts losing at most ONE
//!   mushroom here). The flag file is removed afterwards, the whole thing runs at most once per process.
//! Everything goes to `roster/TestChar1/logs/ad_test.log` (session keys are never written).

use std::{io::Write, time::Duration};

use sf_api::command::Command;

use crate::{inventory, safe, session::SimpleSession, tavern::Outcome};

const CHARACTER: &str = "TestChar1";

static TRIED: crate::ctx::PerChar<bool> = crate::ctx::PerChar::new();

fn flag_path() -> std::path::PathBuf {
    std::path::Path::new("roster").join(CHARACTER).join("ad_test")
}

/// Content of the flag file (`probe`/`weapon`/`magic`), only for the test character.
fn flag() -> Option<String> {
    if crate::ctx::name() != CHARACTER {
        return None;
    }
    std::fs::read_to_string(flag_path()).ok().map(|s| s.trim().to_string())
}

/// Whitelist for `safe::custom_allowed`: exactly the two commands of the chosen shop, nothing else.
pub fn custom_allowed(cmd_name: &str, arguments: &[String]) -> bool {
    let (ad, shop) = match flag().as_deref() {
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

/// Mushrooms the ad refresh may cost (the user accepts one for this experiment).
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
        let _ = writeln!(f, "{} {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
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

pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(mode) = flag() else { return Outcome::Done };
    if TRIED.lock().map_or(true, |mut t| std::mem::replace(&mut *t, true)) {
        return Outcome::Done;
    }
    log(&format!("mode {mode}, ad-related keys of the login response: [{}]", session.login_ad()));
    let (shop_name, ad, shop) = match mode.as_str() {
        "weapon" => ("Weapon", "4", "1"),
        "magic" => ("Magic", "5", "2"),
        _ => {
            let _ = std::fs::remove_file(flag_path());
            return Outcome::Done;
        }
    };
    log(&format!("before: {} | {shop_name} Shop: {}", snapshot(session), offer(session, shop_name)));

    // Pretend to watch the ad like a human would (they last about 10 s), then claim it
    let watch = fastrand::u64(11_000..16_000);
    log(&format!("pretending to watch the ad for {} s", watch / 1000));
    tokio::time::sleep(Duration::from_millis(watch)).await;

    match safe::send_raw_only(session, custom("AdvertisementsCompleted", &[ad])).await {
        Ok(raw) => log(&format!("AdvertisementsCompleted:{ad} → {}", redact(&raw))),
        Err(e) => {
            log(&format!("AdvertisementsCompleted:{ad} failed: {e}"));
            let _ = std::fs::remove_file(flag_path());
            return Outcome::Done;
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
    let _ = std::fs::remove_file(flag_path());
    if session.game_state().is_none() { Outcome::SessionLost } else { Outcome::Done }
}
