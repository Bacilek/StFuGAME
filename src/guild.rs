//! Guild: once a day load the quick-join list (`GroupJoinList`) and join the best guild (`GroupJoin`).
//! Best = highest Instructor, then Treasure, then strength (members × average level) – user 2026-10-07.
//! Already in a guild → switch only to a clearly better one (Instructor at least `SWITCH_MARGIN` higher), and only
//! after `MIN_TENURE` in the current one (user 2026-10-08: otherwise the character could lose eligibility for
//! guild battles – 24 h after joining – again and again before ever taking part).
//! sf-api does not know these commands, they are sent as `Command::Custom` (captured from the browser, docs/guild.md).
//! `battles`: signs up for every planned guild attack/defense (free, user 2026-10-07).


use chrono::{DateTime, Local, NaiveDate};
use sf_api::{
    command::Command,
    gamestate::guild::{BattlesJoined, Guild},
    misc::from_sf_string,
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// How much higher the Instructor of another guild must be before we leave ours.
const SWITCH_MARGIN: u32 = 10;
/// Minimum time in the current guild before another switch is even considered.
const MIN_TENURE: chrono::Duration = chrono::Duration::days(3);
/// A guild has at most 50 members.
const MAX_MEMBERS: u32 = 50;
/// How many candidates to try when joining fails (requirements, full guild, …).
const MAX_ATTEMPTS: usize = 3;

/// One guild from the quick-join list.
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    /// Name exactly as the server sent it (sf escapes), used in `GroupJoin`.
    raw_name: String,
    pub name: String,
    pub rank: u32,
    pub members: u32,
    pub knights: u32,
    pub treasure: u32,
    pub instructor: u32,
    pub raids: u32,
    pub avg_level: u32,
}

impl Offer {
    fn strength(&self) -> u32 {
        self.members * self.avg_level
    }

    /// Sort key: Instructor, then Treasure, then strength.
    fn key(&self) -> (u32, u32, u32) {
        (self.instructor, self.treasure, self.strength())
    }
}

/// Parses `joinablegrouplist` from the raw response. 14 fields per guild separated by `/`
/// (a `/` inside texts is escaped as `$s`): rank/id/name/members/knights/treasure/instructor/raids/emblem/
/// min level/max level/average level/description/language.
pub fn parse_list(raw: &str) -> Vec<Offer> {
    let Some(val) = raw.split('&').find_map(|kv| {
        let (k, v) = kv.split_once(':')?;
        (k.split('.').next() == Some("joinablegrouplist")).then_some(v)
    }) else {
        return Vec::new();
    };
    let fields: Vec<&str> = val.split('/').collect();
    let num = |s: &str| s.trim().parse::<u32>().ok();
    let mut offers = Vec::new();
    for c in fields.as_chunks::<14>().0 {
        let (Some(rank), Some(members), Some(knights), Some(treasure), Some(instructor), Some(raids), Some(avg_level)) =
            (num(c[0]), num(c[3]), num(c[4]), num(c[5]), num(c[6]), num(c[7]), num(c[11]))
        else {
            break; // misaligned data, stop rather than read garbage
        };
        offers.push(Offer {
            raw_name: c[2].to_string(),
            name: from_sf_string(c[2]),
            rank,
            members,
            knights,
            treasure,
            instructor,
            raids,
            avg_level,
        });
    }
    offers
}

/// Candidates to join, best first: not full and (when we already have a guild) clearly better than it.
fn candidates(mut offers: Vec<Offer>, current: Option<(&str, u32)>) -> Vec<Offer> {
    offers.retain(|o| o.members < MAX_MEMBERS);
    if let Some((name, instructor)) = current {
        offers.retain(|o| o.name != name && o.instructor >= instructor + SWITCH_MARGIN);
    }
    offers.sort_by_key(|o| std::cmp::Reverse(o.key()));
    offers
}

fn describe(o: &Offer) -> String {
    format!(
        "{} (rank {}, {} members, avg lvl {}, Hall of Knights {}, Treasure {}, Instructor {}, raids {})",
        o.name, o.rank, o.members, o.avg_level, o.knights, o.treasure, o.instructor, o.raids
    )
}

static LAST_CHECK: crate::ctx::PerChar<Option<NaiveDate>> = crate::ctx::PerChar::new();

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[guild] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

fn custom(cmd_name: &str, arguments: &[&str]) -> Command {
    Command::Custom { cmd_name: cmd_name.to_string(), arguments: arguments.iter().map(|a| (*a).to_string()).collect() }
}

/// Once a day: join the best guild, or switch to a clearly better one.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let today = Local::now().date_naive();
    {
        let Ok(mut last) = LAST_CHECK.lock() else { return Outcome::Done };
        if *last == Some(today) {
            return Outcome::Done;
        }
        *last = Some(today);
    }

    let raw = match safe::send_raw(session, custom("GroupJoinList", &["0"])).await {
        Ok(raw) => raw,
        Err(e) => return fail(&e),
    };
    let offers = parse_list(&raw);
    if offers.is_empty() {
        report!("[guild] The quick-join list is empty or could not be read");
        return Outcome::Done;
    }
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let current = gs.guild.as_ref().map(|g| (g.name.clone(), u32::from(g.total_instructor_skill)));
    let player_id = gs.character.player_id;
    let tenure = gs.guild.as_ref().and_then(|g| g.joined).map(|j| Local::now() - j);
    let too_new = tenure.is_some_and(|t| t < MIN_TENURE);
    let list = if too_new { Vec::new() } else { candidates(offers.clone(), current.as_ref().map(|(n, i)| (n.as_str(), *i))) };

    match &current {
        Some((name, instructor)) if too_new => {
            report!(
                "[guild] Staying in {name} (Instructor {instructor}): joined {}, waiting for the {} day minimum",
                tenure.map_or("an unknown time ago".to_string(), |t| format!("{} h ago", t.num_hours())),
                MIN_TENURE.num_days()
            );
            return Outcome::Done;
        }
        Some((name, instructor)) if list.is_empty() => {
            report!("[guild] Staying in {name} (Instructor {instructor}), no guild in the list is clearly better");
            return Outcome::Done;
        }
        Some((name, instructor)) => {
            report!("[guild] Leaving {name} (Instructor {instructor}) for a better guild: {}", describe(&list[0]));
            if let Err(e) = safe::send(session, custom("GroupRemoveMember", &[&player_id.to_string()])).await {
                return fail(&e);
            }
        }
        None if list.is_empty() => {
            report!("[guild] No guild in the list can be joined ({} offers, all full)", offers.len());
            return Outcome::Done;
        }
        None => {}
    }

    for offer in list.iter().take(MAX_ATTEMPTS) {
        report!("[guild] Joining {}", describe(offer));
        match safe::send(session, custom("GroupJoin", &[&offer.raw_name, "int"])).await {
            Ok(gs) if gs.guild.as_ref().is_some_and(|g| g.name == offer.name) => {
                report!("[guild] Joined {}", offer.name);
                return Outcome::Done;
            }
            Ok(_) => report!("[guild] Joining {} did not work, trying the next one", offer.name),
            Err(e) if crate::tavern::is_session_error(&e) => return Outcome::SessionLost,
            Err(e) => report!("[guild] Joining {} failed: {e}", offer.name),
        }
    }
    report!("[guild] Could not join any guild today");
    Outcome::Done
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Battle {
    Attack,
    Defense,
}

/// Battles we already signed up for (kind + time of the battle), so we never send the same sign-up twice.
static SIGNED_UP: crate::ctx::PerChar<Vec<(Battle, DateTime<Local>)>> = crate::ctx::PerChar::new();
/// Last failed sign-up per kind (e.g. ~12 h after joining a guild, not yet a 24 h member): retry every 4 h
/// rather than hourly, since the server-side reason doesn't change until the 24 h mark anyway. This is only
/// in-memory, so a reconnect/restart still resets it and may retry sooner than that.
static LAST_FAIL: crate::ctx::PerChar<Vec<(Battle, std::time::Instant)>> = crate::ctx::PerChar::new();
const RETRY_SEC: u64 = 4 * 3600;

/// Has our character already joined this kind of battle (per the guild member data)?
fn already_joined(guild: &Guild, me: &str, kind: Battle) -> bool {
    let joined = guild.members.iter().find(|m| m.name == me).and_then(|m| m.battles_joined);
    matches!(
        (kind, joined),
        (Battle::Attack, Some(BattlesJoined::Attack | BattlesJoined::Both))
            | (Battle::Defense, Some(BattlesJoined::Defense | BattlesJoined::Both))
    )
}

/// Signs up for every planned guild attack (incl. raids) and defense we have not joined yet.
pub async fn battles(session: &mut SimpleSession) -> Outcome {
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(guild) = &gs.guild else { return Outcome::Done };
    let now = Local::now();
    let me = gs.character.name.clone();
    let mut todo = Vec::new();
    for (kind, planned) in [(Battle::Attack, &guild.attacking), (Battle::Defense, &guild.defending)] {
        let Some(b) = planned else { continue };
        let signed = SIGNED_UP.lock().is_ok_and(|v| v.contains(&(kind, b.date)));
        let failed_recently = LAST_FAIL
            .lock()
            .is_ok_and(|v| v.iter().any(|(k, t)| *k == kind && t.elapsed().as_secs() < RETRY_SEC));
        if b.date > now && !signed && !failed_recently && !already_joined(guild, &me, kind) {
            let what = if b.is_raid() { "raid".to_string() } else { format!("{kind:?}").to_lowercase() };
            todo.push((kind, b.date, what));
        }
    }

    for (kind, date, what) in todo {
        report!("[guild] Signing up for the guild {what} at {}", date.format("%d.%m. %H:%M"));
        let cmd = match kind {
            Battle::Attack => Command::GuildJoinAttack,
            Battle::Defense => Command::GuildJoinDefense,
        };
        match safe::send(session, cmd).await {
            Ok(_) => {
                if let Ok(mut v) = SIGNED_UP.lock() {
                    v.push((kind, date));
                }
            }
            Err(e) if crate::tavern::is_session_error(&e) => return Outcome::SessionLost,
            Err(e) => {
                report!("[guild] Sign-up failed: {e} (retry in {} h)", RETRY_SEC / 3600);
                if let Ok(mut v) = LAST_FAIL.lock() {
                    v.retain(|(k, _)| *k != kind);
                    v.push((kind, std::time::Instant::now()));
                }
            }
        }
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shortened real response from 2026-10-07.
    const RAW: &str = "joinablegrouplist.r:108/504/Artušova Garda/48/3/106/106/0/0000000000000000000000/10/33/17/- 5$s5 $b$b- Be active$b$bGood Luck $c)/xx/304/280/Venom/45/1/82/86/0/09030B090C030209090503/5/33/15//fr/253/307/EZM/25/3/31/26/0/03090F0610010202020203/1/37/13//xx/213/101/Autogrill/16/0/44/41/0//12/32/20//xx/";

    #[test]
    fn parses_the_list() {
        let offers = parse_list(RAW);
        assert_eq!(offers.len(), 4);
        let a = &offers[0];
        assert_eq!((a.name.as_str(), a.rank, a.members, a.knights), ("Artušova Garda", 108, 48, 3));
        assert_eq!((a.treasure, a.instructor, a.raids, a.avg_level), (106, 106, 0, 17));
        assert_eq!((offers[1].treasure, offers[1].instructor), (82, 86));
        assert_eq!(offers[3].name, "Autogrill");
    }

    #[test]
    fn picks_by_instructor_then_treasure() {
        let list = candidates(parse_list(RAW), None);
        let names: Vec<_> = list.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Artušova Garda", "Venom", "Autogrill", "EZM"]);
    }

    #[test]
    fn switches_only_for_a_clearly_better_guild() {
        // Venom has Instructor 86: from a guild with 80 not worth it, from 76 yes
        assert!(candidates(parse_list(RAW), Some(("Artušova Garda", 100))).is_empty());
        let list = candidates(parse_list(RAW), Some(("X", 80)));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Artušova Garda");
    }

    #[test]
    fn skips_full_guilds() {
        let raw = RAW.replace("Venom/45/", "Venom/50/");
        assert!(candidates(parse_list(&raw), None).iter().all(|o| o.name != "Venom"));
    }
}
