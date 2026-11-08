//! Once-a-day free battles (user 2026-10-31, `docs/battles.md`): the personal Demon Portal (`PlayerPortalBattle`, does not trigger the
//! Dungeons cooldown), the guild Portal (`GroupPortalBattle`) and the guild pet against the hydra (`GroupPetBattle`, never with a
//! mushroom). Level 99+ is assumed for the portals; the pet battle needs the guild leader to have set a pet, so a refusal is normal and
//! is only logged. Every attempt is remembered per day in `roster/<character>/battles_day.txt` so a guild that does not allow it is not
//! asked again before the next day.

use chrono::Local;
use sf_api::{command::Command, gamestate::GameState};

use crate::{safe, session::SimpleSession, tavern::Outcome};

const PORTAL_LEVEL: u16 = 99;

fn marker_file() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("battles_day.txt")
}

fn tried_today(kind: &str) -> bool {
    let today = Local::now().date_naive().to_string();
    std::fs::read_to_string(marker_file()).is_ok_and(|t| t.lines().any(|l| l == format!("{kind} {today}")))
}

fn mark(kind: &str) {
    let today = Local::now().date_naive();
    let keep: Vec<String> = std::fs::read_to_string(marker_file())
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with(kind))
        .map(str::to_string)
        .collect();
    let mut text = keep.join("\n");
    text.push_str(&format!("\n{kind} {today}\n"));
    let _ = std::fs::write(marker_file(), text.trim_start());
}

fn is_today(t: chrono::DateTime<Local>) -> bool {
    t.date_naive() == Local::now().date_naive()
}

/// Personal portal: the server's `can_fight` (portal day differs from today).
pub fn personal_portal_ok(gs: &GameState) -> bool {
    gs.character.level >= PORTAL_LEVEL && gs.dungeons.portal.as_ref().is_some_and(|p| p.can_fight)
}

/// Guild portal: in a guild, and our own member entry has not fought the portal today.
pub fn guild_portal_ok(gs: &GameState) -> bool {
    gs.character.level >= PORTAL_LEVEL
        && gs.guild.as_ref().is_some_and(|g| {
            g.members.iter().find(|m| m.name == gs.character.name).is_none_or(|m| !m.portal_fought.is_some_and(is_today))
        })
}

/// Guild pet vs. the hydra: in a guild, and the hydra was not fought today.
pub fn pet_battle_ok(gs: &GameState) -> bool {
    gs.guild.as_ref().is_some_and(|g| !g.hydra.last_battle.is_some_and(is_today))
}

/// Attempts per kind and day: a failure (midnight data not ready yet, a refusal, a fight that did not register) is retried after an hour.
const MAX_TRIES: u32 = 4;
const RETRY_SECS: i64 = 3600;

/// Failed attempts today and the time of the last one (`kind-fail <date> <unix time> <count>`).
fn failures(kind: &str) -> (u32, i64) {
    let today = Local::now().date_naive().to_string();
    let key = format!("{kind}-fail");
    std::fs::read_to_string(marker_file())
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            let mut it = l.split_whitespace();
            (it.next() == Some(key.as_str()) && it.next() == Some(today.as_str())).then(|| {
                let t = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                (it.next().and_then(|v| v.parse().ok()).unwrap_or(0), t)
            })
        })
        .unwrap_or((0, 0))
}

fn mark_failure(kind: &str) {
    let (n, _) = failures(kind);
    let key = format!("{kind}-fail");
    let mut lines: Vec<String> = std::fs::read_to_string(marker_file()).unwrap_or_default().lines().filter(|l| !l.starts_with(&key)).map(str::to_string).collect();
    lines.push(format!("{key} {} {} {}", Local::now().date_naive(), Local::now().timestamp(), n + 1));
    let _ = std::fs::write(marker_file(), lines.join("\n") + "\n");
    if n + 1 >= MAX_TRIES {
        mark(kind);
    }
}

fn retry_due(kind: &str) -> bool {
    let (n, t) = failures(kind);
    n == 0 || Local::now().timestamp() - t >= RETRY_SECS
}

/// Fights whatever is still open today, once per kind; a failure is retried hourly, at most `MAX_TRIES` times (user 2026-11-08: Květoš
/// never fought the guild Portal although it was open, the single midnight attempt had been spent).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    for (kind, label) in [("portal", "Demon Portal"), ("guild-portal", "guild Portal"), ("guild-pet", "guild pet vs. hydra")] {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let (ok, cmd) = match kind {
            "portal" => (personal_portal_ok(gs), Command::FightPortal),
            "guild-portal" => (guild_portal_ok(gs), Command::GuildPortalBattle),
            _ => (pet_battle_ok(gs), Command::GuildPetBattle { use_mushroom: false }),
        };
        if !ok || tried_today(kind) || !retry_due(kind) {
            continue;
        }
        report!("[battles] {label}: fighting (try {})", failures(kind).0 + 1);
        let sent = safe::send(session, cmd).await.map(|_| ());
        match &sent {
            Ok(_) => {}
            Err(e) if crate::tavern::is_session_error(e) => return Outcome::SessionLost,
            Err(e) => report!("[battles] {label}: refused or failed ({e})"),
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            if crate::tavern::is_session_error(&e) {
                return Outcome::SessionLost;
            }
        }
        // Done only when the game state no longer offers the fight
        let still_open = session.game_state().is_some_and(|gs| match kind {
            "portal" => personal_portal_ok(gs),
            "guild-portal" => guild_portal_ok(gs),
            _ => pet_battle_ok(gs),
        });
        if sent.is_ok() && !still_open {
            mark(kind);
            report!("[battles] {label}: done");
        } else {
            report!("[battles] {label}: not done yet, trying again in an hour");
            mark_failure(kind);
        }
        safe::human_pause().await;
    }
    Outcome::Done
}
