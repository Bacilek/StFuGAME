//! Coupon codes from other players (user 2026-10-09, code `N3UTR4L-EU31`: a free mount, Life potion and lucky coins).
//! Two steps, both outside the game's own commands:
//! 1. **Redeem** – a plain HTTP POST to the Playa Games coupon service (what the "Redeem Coupon" screen at the
//!    Mushroom Dealer does, captured from a browser Network tab): form fields `coupon`, `paymentstring`, `lang`.
//!    `paymentstring` = `<player id>_<first number of ownplayersavecharacter>_<second-to-last number of it>_1`
//!    (reverse-engineered from ONE sample, the trailing `1` is a guess). The answer is `{"status":"ok"}` on success.
//! 2. **Claim** – the code shows up as a pending reward (`pendingrewards`, type Coupon) in the Mail; viewed and claimed with
//!    the game's own `PendingRewardView`/`PendingRewardClaim` (sf-api `ClaimablePreview`/`ClaimableClaim`).
//!
//! Codes live in `roster/coupons.txt` (one per line, `#` comments), the per-character progress in
//! `roster/<character>/coupons.txt` (`code<TAB>redeemed|claimed|rejected:<status>`). Every code is redeemed at most
//! once per character (a rejection is final, delete the line to retry). Nothing here spends mushrooms.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use sf_api::{
    command::Command,
    gamestate::social::{ClaimableMailType, ClaimableStatus},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

const REDEEM_URL: &str = "https://coupon.playa-games.com/redeem";
const CODES_FILE: &str = "roster/coupons.txt";

fn state_path() -> PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("coupons.txt")
}

fn read_codes() -> Vec<String> {
    std::fs::read_to_string(CODES_FILE)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn load_state() -> BTreeMap<String, String> {
    std::fs::read_to_string(state_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(c, s)| (c.to_string(), s.to_string()))
        .collect()
}

fn save_state(state: &BTreeMap<String, String>) {
    let path = state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text: String = state.iter().map(|(c, s)| format!("{c}\t{s}\n")).collect();
    let _ = std::fs::write(path, text);
}

/// The name a coupon has in the Mail: the code without dashes (`N3UTR4L-EU31` → `N3UTR4LEU31`).
fn mail_name(code: &str) -> String {
    code.chars().filter(|c| *c != '-').collect::<String>().to_uppercase()
}

/// `<player id>_<first save number>_<second-to-last save number>_1`, see the module docs.
fn payment_string(save: &[i64]) -> Option<String> {
    let (pid, first, world) = (*save.get(1)?, *save.first()?, *save.get(save.len().checked_sub(2)?)?);
    Some(format!("{pid}_{first}_{world}_1"))
}

/// Posts the code to the coupon service, returns its `status` (`ok` = redeemed).
async fn redeem(code: &str, payment: &str) -> Result<String, String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let resp = client
        .post(REDEEM_URL)
        .form(&[("coupon", code), ("paymentstring", payment), ("lang", "en")])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let http = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|_| format!("HTTP {http}, unexpected answer"))?;
    v["status"].as_str().map(str::to_string).ok_or_else(|| format!("HTTP {http}, no status in the answer"))
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[coupons] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Redeems the codes this character has not used yet and claims their rewards from the Mail.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let codes = read_codes();
    if codes.is_empty() {
        return Outcome::Done;
    }
    let mut state = load_state();

    // 1. Redeem
    let todo: Vec<String> = codes.iter().filter(|c| !state.contains_key(*c)).cloned().collect();
    let mut redeemed_now = false;
    for code in &todo {
        let Some(payment) = session.char_save().and_then(payment_string) else {
            report!("[coupons] No character data for the payment string yet, trying again later");
            return Outcome::Done;
        };
        match redeem(code, &payment).await {
            Ok(status) if status == "ok" => {
                report!("[coupons] Redeemed {code}, the reward is in the Mail");
                state.insert(code.clone(), "redeemed".into());
                redeemed_now = true;
            }
            Ok(status) => {
                report!("[coupons] {code} rejected by the service: {status}");
                state.insert(code.clone(), format!("rejected:{status}"));
            }
            Err(e) => {
                report!("[coupons] Could not redeem {code}: {e}");
                return Outcome::Done;
            }
        }
        save_state(&state);
        tokio::time::sleep(Duration::from_millis(fastrand::u64(1500..3500))).await;
    }

    // 2. Claim whatever waits in the Mail for one of our codes (also a code redeemed by hand earlier: it is "rejected"
    // by the service now but its reward may still be unclaimed)
    if redeemed_now && let Err(e) = safe::send(session, Command::Update).await {
        return fail(&e);
    }
    let wanted: Vec<(String, i64)> = session
        .game_state()
        .map(|gs| {
            gs.mail
                .claimables
                .iter()
                .filter(|m| m.typ == ClaimableMailType::Coupon && m.status != ClaimableStatus::Claimed)
                .filter_map(|m| {
                    let code = codes.iter().find(|c| mail_name(c) == mail_name(&m.name))?;
                    (state.get(code).map(String::as_str) != Some("claimed")).then(|| (code.clone(), m.msg_id))
                })
                .collect()
        })
        .unwrap_or_default();
    for (code, msg_id) in &wanted {
        if let Err(e) = safe::send(session, Command::ClaimablePreview { msg_id: *msg_id }).await {
            return fail(&e);
        }
        if let Err(e) = safe::send(session, Command::ClaimableClaim { msg_id: *msg_id }).await {
            return fail(&e);
        }
        report!("[coupons] Claimed the reward of {code} from the Mail");
        state.insert(code.clone(), "claimed".into());
        save_state(&state);
    }
    if !wanted.is_empty()
        && let Err(e) = safe::send(session, Command::Update).await
    {
        return fail(&e);
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_string_matches_the_captured_sample() {
        // ownplayersavecharacter of Mimimimi11 (2026-10-09), paymentstring seen in the browser: 27375_339294320_562_1
        let save: Vec<i64> = "339294320/27375/0/16/10000/15730/528/10963/8/301/302/2/302/3/1/8/0/0/2/1/11/4/65536/547/15/45/0/8772/0/1792629758/99/32/27/86/59/89/21/0/56/40/75/12/12/67/44/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/1/0/307/10021/0/562/0"
            .split('/')
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(payment_string(&save).as_deref(), Some("27375_339294320_562_1"));
    }

    #[test]
    fn payment_string_second_sample_confirms_the_trailing_one() {
        // Pagan (2026-10-09): ownplayersavecharacter 7646100/27812/…/562/0 → paymentstring 27812_7646100_562_1
        let save: Vec<i64> = "7646100/27812/0/16/8610/15730/539/10886/7/404/401/4/401/3/1/2/1/0/5/1/5/4/32768/128/18/34/0/6290/0/1793879656/83/24/19/74/48/80/6/17/53/39/67/8/8/59/38/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/1/0/343/0/0/562/0"
            .split('/')
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(payment_string(&save).as_deref(), Some("27812_7646100_562_1"));
    }

    #[test]
    fn mail_name_drops_dashes() {
        assert_eq!(mail_name("N3UTR4L-EU31"), "N3UTR4LEU31");
    }
}
