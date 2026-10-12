//! ONE-OFF tool (user 2026-10-12), not part of the bot: every challenge character adds all the other challenge
//! characters (and the given extra names, e.g. the user's own character) as friends in the game (`PlayerFriendSet`).
//!
//! Usage (the bot must NOT be running – logging in here kicks its sessions):
//!   cargo run --release --bin befriend -- [--go] [--skip Name]… [--also Name]…
//! Without `--go` it only logs in, looks everybody up and prints what it would do (no friend request is sent).
//! Accounts come from `.env` the same way the bot reads them (`SF_USER`/… and `SF_ACCOUNTS`). Never prints credentials.
//! Sends only `Update`, `ViewPlayer` and `SetPlayerRelation { Friend }` – none of them costs mushrooms.

use std::time::Duration;

use sf_api::{command::Command, gamestate::social::Relationship, session::SimpleSession};

struct Account {
    user: String,
    pass: String,
    character: String,
    alt: Option<String>,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn accounts() -> Vec<Account> {
    let mut out: Vec<Account> = Vec::new();
    if let (Some(user), Some(pass), Some(character)) = (env("SF_USER"), env("SF_PASS"), env("SF_CHARACTER")) {
        out.push(Account { user, pass, character, alt: env("SF_USER_ALT") });
    }
    if let Some(list) = env("SF_ACCOUNTS") {
        for entry in list.split(['\n', '\r', ';']).map(str::trim).filter(|e| !e.is_empty()) {
            let p: Vec<&str> = entry.split('|').map(str::trim).collect();
            if p.len() < 3 || out.iter().any(|a| a.character.eq_ignore_ascii_case(p[2])) {
                continue;
            }
            out.push(Account { user: p[0].into(), pass: p[1].into(), character: p[2].into(), alt: p.get(3).map(|s| (*s).into()) });
        }
    }
    out
}

async fn pause() {
    tokio::time::sleep(Duration::from_millis(1500 + fastrand::u64(0..2500))).await;
}

async fn login(a: &Account) -> Result<SimpleSession, String> {
    let sessions = match SimpleSession::login_sf_account(&a.user, &a.pass).await {
        Ok(s) => s,
        Err(e) => match &a.alt {
            Some(alt) => SimpleSession::login_sf_account(alt, &a.pass).await.map_err(|e| format!("login failed: {e}"))?,
            None => return Err(format!("login failed: {e}")),
        },
    };
    let mut found: Vec<SimpleSession> =
        sessions.into_iter().filter(|s| s.username().eq_ignore_ascii_case(&a.character)).collect();
    if found.len() != 1 {
        return Err(format!("{} matching characters (expected 1)", found.len()));
    }
    let mut s = found.remove(0);
    s.send_command(Command::Update).await.map_err(|e| format!("Update failed: {e}"))?;
    Ok(s)
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let go = args.iter().any(|a| a == "--go");
    let list = |flag: &str| -> Vec<String> {
        args.windows(2).filter(|w| w[0] == flag).map(|w| w[1].clone()).collect()
    };
    let skip = list("--skip");
    let also = list("--also");

    let accounts: Vec<Account> =
        accounts().into_iter().filter(|a| !skip.iter().any(|s| s.eq_ignore_ascii_case(&a.character))).collect();
    println!("{} characters, extra names: {also:?}, mode: {}", accounts.len(), if go { "SEND" } else { "dry run" });

    // Phase 1: log everybody in, remember the player ids
    let mut sessions: Vec<(String, u32, SimpleSession)> = Vec::new();
    for a in &accounts {
        tokio::time::sleep(Duration::from_secs(4)).await;
        match login(a).await {
            Ok(s) => {
                let id = s.game_state().map_or(0, |gs| gs.character.player_id);
                println!("logged in: {} (id {id})", a.character);
                sessions.push((a.character.clone(), id, s));
            }
            Err(e) => println!("SKIPPED {}: {e}", a.character),
        }
    }
    if sessions.is_empty() {
        return;
    }

    // Ids of the extra names (looked up by the first character)
    let mut extra: Vec<(String, u32)> = Vec::new();
    for name in &also {
        pause().await;
        let s = &mut sessions[0].2;
        match s.send_command(Command::ViewPlayer { ident: name.clone() }).await {
            Ok(gs) => match gs.lookup.lookup_name(name) {
                Some(p) => {
                    println!("found {name}: id {}, level {}", p.player_id, p.level);
                    extra.push((name.clone(), p.player_id));
                }
                None => println!("NOT FOUND: {name}"),
            },
            Err(e) => println!("lookup of {name} failed: {e}"),
        }
    }

    // Phase 2: everybody adds everybody else
    let everyone: Vec<(String, u32)> =
        sessions.iter().map(|(n, id, _)| (n.clone(), *id)).chain(extra).collect();
    let (mut added, mut already, mut failed) = (0, 0, 0);
    for (me, my_id, session) in &mut sessions {
        for (name, id) in &everyone {
            if id == my_id || *id == 0 {
                continue;
            }
            let known = session
                .game_state()
                .is_some_and(|gs| gs.character.relations.iter().any(|r| r.id == *id && r.relation == Relationship::Friend));
            if known {
                already += 1;
                continue;
            }
            if !go {
                println!("[dry] {me} would add {name} (id {id})");
                continue;
            }
            pause().await;
            let cmd = Command::SetPlayerRelation { player_id: *id, relation: Relationship::Friend };
            match session.send_command(cmd).await {
                Ok(gs) => {
                    let ok = gs.character.relations.iter().any(|r| r.id == *id && r.relation == Relationship::Friend);
                    println!("{me} -> {name}: {}", if ok { "added" } else { "sent, but not in the friend list (check)" });
                    if ok { added += 1 } else { failed += 1 }
                }
                Err(e) => {
                    println!("{me} -> {name}: FAILED {e}");
                    failed += 1;
                }
            }
        }
    }
    println!("done: {added} added, {already} already friends, {failed} failed");
}
