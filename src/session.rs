//! Our own copy of sf-api's `SimpleSession` (session + game state). The only difference: `send_raw` also returns
//! the raw server response, because sf-api ignores some keys we need (e.g. `joinablegrouplist` for the Guild).
//! Same method names as the sf-api one, so the rest of the bot does not care.

use std::time::Duration;

use sf_api::{command::Command, error::SFError, gamestate::GameState, session::Session};

pub struct SimpleSession {
    session: Session,
    gamestate: Option<GameState>,
    /// The raw `ownplayersavecharacter` numbers of the latest response that carried them (sf-api drops the first
    /// one; the coupon "payment string" needs it, see `coupons.rs`).
    char_save: Option<Vec<i64>>,
    /// `key:value` pairs of the latest login response that might say whether the daily shop ad is still available
    /// (`skipvideo` etc.; sf-api ignores them), see `adtest.rs`. Never contains session keys.
    login_ad: String,
}

/// The `key:value` pairs of a raw response whose key looks ad-related (`skipvideo`, `skipallow`, …).
fn ad_keys(raw: &str) -> String {
    raw.split('&')
        .filter(|kv| {
            let key = kv.split(':').next().unwrap_or("");
            key.contains("skip") || key.contains("video") || key.contains("advert")
        })
        .map(|kv| kv.chars().take(80).collect::<String>())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Picks the `ownplayersavecharacter` numbers out of a raw response (`key:value&key:value…`), if it has them.
fn parse_char_save(raw: &str) -> Option<Vec<i64>> {
    let v = raw.split('&').find_map(|kv| kv.strip_prefix("ownplayersavecharacter:"))?;
    let nums: Vec<i64> = v.split('/').filter_map(|s| s.parse().ok()).collect();
    (nums.len() > 3).then_some(nums)
}

impl SimpleSession {
    /// Logs in to the S&F account (SSO) and returns a session for every character of the account.
    pub async fn login_sf_account(username: &str, password: &str) -> Result<Vec<Self>, SFError> {
        let acc = sf_api::sso::SFAccount::login(username.to_string(), password.to_string()).await?;
        Ok(acc
            .characters()
            .await?
            .into_iter()
            .flatten()
            .map(|session| Self { session, gamestate: None, char_save: None, login_ad: String::new() })
            .collect())
    }

    /// Server host name, e.g. s31.sfgame.eu.
    pub fn server_host(&self) -> Option<&str> {
        self.session.server_url().host_str()
    }

    pub fn username(&self) -> &str {
        self.session.username()
    }

    /// Numbers of the latest `ownplayersavecharacter` seen (login or any response), see `coupons.rs`.
    pub fn char_save(&self) -> Option<&[i64]> {
        self.char_save.as_deref()
    }

    /// Ad-related keys of the latest login response (see `adtest.rs`).
    pub fn login_ad(&self) -> &str {
        &self.login_ad
    }

    pub fn game_state(&self) -> Option<&GameState> {
        self.gamestate.as_ref()
    }

    pub fn game_state_mut(&mut self) -> Option<&mut GameState> {
        self.gamestate.as_mut()
    }

    /// Sends a command (logs in first if needed), updates the game state and returns the raw server response
    /// (`key:value&key:value…`).
    pub async fn send_raw(&mut self, cmd: Command) -> Result<String, SFError> {
        if self.gamestate.is_none() {
            let resp = self.session.login().await?;
            self.char_save = parse_char_save(resp.raw_response()).or(self.char_save.take());
            self.login_ad = ad_keys(resp.raw_response());
            self.gamestate = Some(GameState::new(resp)?);
            tokio::time::sleep(Duration::from_millis(fastrand::u64(1000..2000))).await;
        }
        let resp = match self.session.send_command(cmd).await {
            Ok(resp) => resp,
            Err(e) => {
                self.gamestate = None;
                return Err(e);
            }
        };
        let raw = resp.raw_response().to_string();
        self.char_save = parse_char_save(&raw).or(self.char_save.take());
        if let Some(gs) = &mut self.gamestate
            && let Err(e) = gs.update(resp)
        {
            self.gamestate = None;
            return Err(e);
        }
        Ok(raw)
    }

    /// Like `send_raw`, but never touches the game state – for responses `GameState::update` cannot be trusted to
    /// parse (e.g. the shop's `ShopCatalog`/`ShopCheckout`, whose raw response is a JSON blob, optionally followed
    /// by `&key:value…` pairs sf-api does understand, not a response of either shape alone). A normal `Update` on
    /// the next pass refreshes the game state as usual; this only returns what the command itself said.
    pub async fn send_raw_only(&mut self, cmd: Command) -> Result<String, SFError> {
        if self.gamestate.is_none() {
            let resp = self.session.login().await?;
            self.char_save = parse_char_save(resp.raw_response()).or(self.char_save.take());
            self.login_ad = ad_keys(resp.raw_response());
            self.gamestate = Some(GameState::new(resp)?);
            tokio::time::sleep(Duration::from_millis(fastrand::u64(1000..2000))).await;
        }
        let resp = self.session.send_command(cmd).await?;
        self.char_save = parse_char_save(resp.raw_response()).or(self.char_save.take());
        Ok(resp.raw_response().to_string())
    }
}
