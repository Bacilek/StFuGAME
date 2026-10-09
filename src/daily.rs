//! Daily rewards: the daily login bonus (calendar) and one free Wheel of Fortune spin per day (user rule: never
//! for mushrooms or lucky coins) – plus the one-time free "new customer" pack at the Mushroom Dealer, below.

use std::time::{Duration, Instant};

use chrono::Local;
use serde_json::Value;
use sf_api::command::{Command, FortunePayment};

use crate::session::SimpleSession;

use crate::{safe, tavern::Outcome};

fn custom(cmd_name: &str, arguments: &[&str]) -> Command {
    Command::Custom { cmd_name: cmd_name.to_string(), arguments: arguments.iter().map(|a| (*a).to_string()).collect() }
}

/// When an action fails (state unchanged), retry it at the earliest after this long.
const RETRY: Duration = Duration::from_secs(30 * 60);

static TRIED: crate::ctx::PerChar<Vec<(&'static str, Instant)>> = crate::ctx::PerChar::new();

/// May the action be tried? Records the attempt (for retrying after a failure).
fn may_try(what: &'static str) -> bool {
    let Ok(mut tried) = TRIED.lock() else { return false };
    if tried.iter().any(|(w, t)| *w == what && t.elapsed() < RETRY) {
        return false;
    }
    tried.retain(|(w, _)| *w != what);
    tried.push((what, Instant::now()));
    true
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[rewards] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Collects everything available today. Sends nothing when there is nothing to collect.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let now = Local::now();
    let specials = &gs.specials;

    // Calendar: one reward per day
    let calendar_ready = specials.calendar.next_possible.is_some_and(|t| t <= now);
    if calendar_ready && may_try("calendar") {
        let reward = specials.calendar.rewards.get(specials.calendar.collected).map(|r| crate::report::reward(&format!("{:?}", r.typ), r.amount));
        report!("[rewards] Collecting the daily login bonus: {}", reward.unwrap_or_else(|| "?".into()));
        if let Err(e) = safe::send(session, Command::CollectCalendar).await {
            return fail(&e);
        }
    }

    // Wheel of Fortune: free spin only (never for mushrooms or lucky coins)
    if session.game_state().is_some_and(safe::wheel_is_free) && may_try("wheel") {
        report!("[rewards] Spinning the Wheel of Fortune (free)");
        match safe::send(session, Command::SpinWheelOfFortune { payment: FortunePayment::FreeTurn }).await {
            Ok(gs) => match &gs.specials.wheel.result {
                Some(w) => report!("[rewards] Wheel of Fortune: {}", crate::report::reward(&format!("{:?}", w.typ), w.amount)),
                None => report!("[rewards] Wheel of Fortune: no result from the server"),
            },
            Err(e) => return fail(&e),
        }
    }

    if let Outcome::SessionLost = claim_welcome_pack(session).await {
        return Outcome::SessionLost;
    }

    // Coupon codes from roster/coupons.txt (user 2026-10-09), at most every 30 min while something is pending
    if may_try("coupons")
        && let Outcome::SessionLost = crate::coupons::run(session).await
    {
        return Outcome::SessionLost;
    }

    Outcome::Done
}

/// Marks this character as done with the welcome-pack check, forever – a plain file (not `PerChar` state) so it
/// survives bot restarts: this is genuinely one-off (user 2026-10-08: "bude to jednorázová akce a už nikdy nebude
/// potřeba"), unlike the calendar/wheel above, which are real daily resets. Living in `roster/<nick>/`, not in a
/// `logs/` subfolder, so it is not mistaken for a log file.
fn welcome_pack_marker() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("welcome_pack_claimed")
}

/// One-time free "new customer" pack at the Mushroom Dealer (user 2026-10-07: for new accounts a free deal
/// appears after some time; claimed manually once on TestChar1). Captured live 2026-10-08 from a browser Network
/// tab: not one of sf-api's typed commands, same `Command::Custom` + base64-params mechanism as the guild list
/// (`guild.rs`). Catalog item as of 2026-10-08: identifier `starterpacks_item_2` (internal `welcomepack_1`),
/// `sku: "FREE"`, price 0 – gold, mushrooms, hourglasses and lucky coins. **Never** checks out anything whose
/// catalog price is not exactly 0 – the rest of this shop (`starterpacks_item_1`, mushroom packs, VIP status)
/// costs real money. Checked at most every 30 min (`may_try`, shared with calendar/wheel) until either claimed or
/// the marker file says to stop – not yet verified live (pending: does the pack actually show up and get claimed
/// correctly?).
async fn claim_welcome_pack(session: &mut SimpleSession) -> Outcome {
    if welcome_pack_marker().exists() {
        return Outcome::Done;
    }
    if !may_try("welcome_pack") {
        return Outcome::Done;
    }
    let raw = match safe::send_raw_only(session, custom("ShopCatalog", &["1", "", "1"])).await {
        Ok(raw) => raw,
        Err(e) => return fail(&e),
    };
    let Ok(v) = serde_json::from_str::<Value>(&raw) else {
        report!("[rewards] Could not parse the shop catalog, skipping");
        return Outcome::Done;
    };
    let Some(articles) = v["catalog"]["articles"].as_array() else { return Outcome::Done };
    let free: Vec<String> = articles
        .iter()
        .filter(|a| a["price"]["amount"].as_u64() == Some(0))
        .filter_map(|a| a["identifier"].as_str().map(str::to_string))
        .collect();
    for identifier in free {
        report!("[rewards] Free shop item: {identifier}, claiming");
        if let Err(e) = safe::send_raw_only(session, custom("ShopCheckout", &["1", &identifier, ""])).await {
            return fail(&e);
        }
        // ShopCheckout's response is not parsed into the game state (see `SimpleSession::send_raw_only`) – a
        // normal Update refreshes it and re-triggers the mushroom watchdog against the state from before this.
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
        report!("[rewards] Claimed {identifier}");
        if let Some(parent) = welcome_pack_marker().parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(welcome_pack_marker(), identifier);
    }
    Outcome::Done
}

/// In how many seconds something can be collected (calendar or free spin), if known.
pub fn secs_until_ready(gs: &sf_api::gamestate::GameState) -> Option<u64> {
    let now = Local::now();
    [gs.specials.calendar.next_possible, gs.specials.wheel.next_free_spin]
        .into_iter()
        .flatten()
        .filter(|t| *t > now)
        .map(|t| u64::try_from((t - now).num_seconds()).unwrap_or(0))
        .min()
}
