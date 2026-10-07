//! Our own copy of sf-api's `SimpleSession` (session + game state). The only difference: `send_raw` also returns
//! the raw server response, because sf-api ignores some keys we need (e.g. `joinablegrouplist` for the Guild).
//! Same method names as the sf-api one, so the rest of the bot does not care.

use std::time::Duration;

use sf_api::{command::Command, error::SFError, gamestate::GameState, session::Session};

pub struct SimpleSession {
    session: Session,
    gamestate: Option<GameState>,
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
            .map(|session| Self { session, gamestate: None })
            .collect())
    }

    /// Server host name, e.g. s31.sfgame.eu.
    pub fn server_host(&self) -> Option<&str> {
        self.session.server_url().host_str()
    }

    pub fn username(&self) -> &str {
        self.session.username()
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
        if let Some(gs) = &mut self.gamestate
            && let Err(e) = gs.update(resp)
        {
            self.gamestate = None;
            return Err(e);
        }
        Ok(raw)
    }
}
