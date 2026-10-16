//! Guild: every `CHECK_EVERY` load the quick-join list (`GroupJoinList`, pages until nothing new) and join the best guild
//! (`GroupJoin`). Best = weighted score, the Instructor slightly ahead of the Treasure (3:2, user 2026-10-11: "more weight,
//! but not that much"), then strength (members × average level). Already in a guild → switch as soon as another one has
//! BOTH Instructor and Treasure at least `SWITCH_MARGIN` higher (no minimum tenure since 2026-10-11) – except while an open
//! daily/event task wants a guild fight (`GuildReadyFight`, hydra, portal): after it is done we leave. Guilds whose
//! description asks for "N/M" skill levels we do not have are skipped (`unmet_requirement`).
//! sf-api does not know these commands, they are sent as `Command::Custom` (captured from the browser, docs/guild.md).
//! `battles`: signs up for every planned guild attack/defense (free, user 2026-10-07).


use chrono::{DateTime, Local};
use sf_api::{
    command::Command,
    gamestate::{
        guild::{BattlesJoined, Guild},
        rewards::TaskType,
    },
    misc::from_sf_string,
};

use sf_api::gamestate::rewards::Event;

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// How much higher BOTH the Instructor and the Treasure of another guild must be before we leave ours (user 2026-10-11).
const SWITCH_MARGIN: u32 = 10;
/// How often the quick-join list is checked (a free guild with better bonuses can appear any time).
const CHECK_EVERY: chrono::Duration = chrono::Duration::hours(6);
/// Without a guild the list is re-checked this often (a failed join, a kick): every hour without Instructor/Treasure
/// bonuses is wasted (user 2026-10-12, PajaRizz), the list is only a few requests.
/// While the Exceptional XP or Glorious Gold event runs the guild bonuses count double / five-fold (user 2026-10-16): look more often.
const EVENT_CHECK_EVERY: chrono::Duration = chrono::Duration::hours(2);
/// Without a guild the list is re-checked this often (see below)
const NO_GUILD_RETRY: chrono::Duration = chrono::Duration::minutes(5);
/// Most list pages loaded per check (the page parameter of `GroupJoinList` is unverified: we stop at the first page that
/// brings no new guild).
const MAX_PAGES: u32 = 5;
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
    pub description: String,
}

impl Offer {
    fn strength(&self) -> u32 {
        self.members * self.avg_level
    }

    /// Sort key: Instructor weighted 3, Treasure 2 (levels are similar everywhere, so Instructor leads only slightly),
    /// then strength.
    fn key(&self) -> (u32, u32) {
        (self.instructor * 3 + self.treasure * 2, self.strength())
    }
}

/// Our own guild skill levels (what a description may demand).
#[derive(Clone, Copy, Debug)]
struct Own {
    instructor: u32,
    treasure: u32,
}

/// Does the description demand "N/M" skill levels (e.g. "5/5 Instructor/Treasure") that we do not reach? The order of the
/// two numbers is unknown, so the larger one must be reached by both skills. Free text: a false hit only skips a guild.
fn unmet_requirement(description: &str, own: Own) -> bool {
    let b = description.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let a = description[start..i].parse::<u32>().unwrap_or(0);
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            if j < b.len() && b[j] == b'/' {
                j += 1;
                while j < b.len() && b[j] == b' ' {
                    j += 1;
                }
                let s = j;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                if let Ok(m) = description[s..j].parse::<u32>() {
                    let need = a.max(m);
                    if (1..=100).contains(&need) && own.instructor.min(own.treasure) < need {
                        return true;
                    }
                }
                i = j.max(i);
            }
        } else {
            i += 1;
        }
    }
    false
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
            description: from_sf_string(c[12]),
        });
    }
    offers
}

/// Candidates to join, best first: not full, no skill requirement we miss, and (when we already have a guild) clearly
/// better than it: Instructor AND Treasure both at least `SWITCH_MARGIN` higher. `current` = (name, instructor, treasure).
fn candidates(mut offers: Vec<Offer>, current: Option<(&str, u32, u32)>, own: Own) -> Vec<Offer> {
    offers.retain(|o| o.members < MAX_MEMBERS && !unmet_requirement(&o.description, own));
    if let Some((name, instructor, treasure)) = current {
        offers.retain(|o| {
            o.name != name && o.instructor >= instructor + SWITCH_MARGIN && o.treasure >= treasure + SWITCH_MARGIN
        });
    }
    offers.sort_by_key(|o| std::cmp::Reverse(o.key()));
    offers
}

/// An open daily/event task that wants a guild fight: leaving now would lose it.
fn guild_fight_task_open(gs: &sf_api::gamestate::GameState) -> bool {
    crate::tasks::open_tasks(gs)
        .any(|t| matches!(t.typ, TaskType::GuildReadyFight | TaskType::FightGuildHydra | TaskType::FightGuildPortal))
}

fn describe(o: &Offer) -> String {
    format!(
        "{} (rank {}, {} members, avg lvl {}, Hall of Knights {}, Treasure {}, Instructor {}, raids {})",
        o.name, o.rank, o.members, o.avg_level, o.knights, o.treasure, o.instructor, o.raids
    )
}

static LAST_CHECK: crate::ctx::PerChar<Option<DateTime<Local>>> = crate::ctx::PerChar::new();

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[guild] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

fn custom(cmd_name: &str, arguments: &[&str]) -> Command {
    Command::Custom { cmd_name: cmd_name.to_string(), arguments: arguments.iter().map(|a| (*a).to_string()).collect() }
}

/// Loads the quick-join list: page 0, then further pages while they bring guilds we have not seen.
async fn load_offers(session: &mut SimpleSession) -> Result<Vec<Offer>, sf_api::error::SFError> {
    let mut all: Vec<Offer> = Vec::new();
    for page in 0..MAX_PAGES {
        let raw = safe::send_raw(session, custom("GroupJoinList", &[&page.to_string()])).await?;
        let fresh: Vec<Offer> = parse_list(&raw).into_iter().filter(|o| all.iter().all(|a| a.raw_name != o.raw_name)).collect();
        if fresh.is_empty() {
            break;
        }
        all.extend(fresh);
    }
    Ok(all)
}

/// Every `CHECK_EVERY`: join the best guild, or switch to a clearly better one.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let now = Local::now();
    {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        // A task wants a guild fight: stay (checked before the timer, so we leave right after the task is done)
        if gs.guild.is_some() && guild_fight_task_open(gs) {
            return Outcome::Done;
        }
        let Ok(mut last) = LAST_CHECK.lock() else { return Outcome::Done };
        let event = [Event::ExceptionalXPEvent, Event::GloriousGoldGalore].into_iter().any(|e| crate::tasks::event_active(gs, e));
        let every = if gs.guild.is_none() {
            NO_GUILD_RETRY
        } else if event {
            EVENT_CHECK_EVERY
        } else {
            CHECK_EVERY
        };
        if last.is_some_and(|t| now - t < every) {
            return Outcome::Done;
        }
        *last = Some(now);
    }

    let offers = match load_offers(session).await {
        Ok(o) => o,
        Err(e) => return fail(&e),
    };
    if offers.is_empty() {
        report!("[guild] The quick-join list is empty or could not be read");
        return Outcome::Done;
    }
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let current = gs.guild.as_ref().map(|g| {
        (g.name.clone(), u32::from(g.total_instructor_skill), u32::from(g.total_treasure_skill))
    });
    let own = gs.guild.as_ref().map_or(Own { instructor: 0, treasure: 0 }, |g| Own {
        instructor: u32::from(g.own_instructor_skill),
        treasure: u32::from(g.own_treasure_skill),
    });
    let player_id = gs.character.player_id;
    let skipped: Vec<&str> = offers.iter().filter(|o| o.members < MAX_MEMBERS && unmet_requirement(&o.description, own)).map(|o| o.name.as_str()).collect();
    if !skipped.is_empty() {
        report!("[guild] Skipping {} guild(s) whose description asks for skill levels we lack: {}", skipped.len(), skipped.join(", "));
    }
    let list = candidates(offers.clone(), current.as_ref().map(|(n, i, t)| (n.as_str(), *i, *t)), own);

    match &current {
        Some((name, instructor, treasure)) if list.is_empty() => {
            report!(
                "[guild] Staying in {name} (Instructor {instructor}, Treasure {treasure}), no joinable guild is {SWITCH_MARGIN}/{SWITCH_MARGIN} better"
            );
            return Outcome::Done;
        }
        Some((name, instructor, treasure)) => {
            report!(
                "[guild] Leaving {name} (Instructor {instructor}, Treasure {treasure}) for a better guild: {}",
                describe(&list[0])
            );
            if let Err(e) = safe::send(session, custom("GroupRemoveMember", &[&player_id.to_string()])).await {
                return fail(&e);
            }
        }
        None if list.is_empty() => {
            report!("[guild] No guild in the list can be joined ({} offers, all full or with requirements)", offers.len());
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
    report!("[guild] Could not join any guild this time");
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
    const OWN: Own = Own { instructor: 10, treasure: 10 };
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
        let list = candidates(parse_list(RAW), None, OWN);
        let names: Vec<_> = list.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Artušova Garda", "Venom", "Autogrill", "EZM"]);
    }

    #[test]
    fn switches_only_for_a_clearly_better_guild() {
        // Venom is 86/82: from a guild with 80/80 not 10/10 better, Artušova Garda (106/106) is; Treasure must rise too
        assert!(candidates(parse_list(RAW), Some(("Artušova Garda", 100, 100)), OWN).is_empty());
        let list = candidates(parse_list(RAW), Some(("X", 80, 80)), OWN);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Artušova Garda");
    }

    #[test]
    fn skips_full_guilds() {
        let raw = RAW.replace("Venom/45/", "Venom/50/");
        assert!(candidates(parse_list(&raw), None, OWN).iter().all(|o| o.name != "Venom"));
    }
}

#[cfg(test)]
mod requirement_tests {
    use super::*;

    #[test]
    fn descriptions_with_skill_requirements() {
        let own = Own { instructor: 5, treasure: 7 };
        assert!(unmet_requirement("Need 10/10 Instructor/Treasure", own));
        assert!(unmet_requirement("min 6 / 5", own));
        assert!(!unmet_requirement("min 5/5 please", own));
        assert!(!unmet_requirement("- 5 5 Be active, 1 2 3", own));
        assert!(!unmet_requirement("", own));
    }

    #[test]
    fn instructor_leads_only_slightly() {
        let o = |i, t| Offer {
            raw_name: String::new(), name: String::new(), rank: 1, members: 10, knights: 0, treasure: t, instructor: i,
            raids: 0, avg_level: 10, description: String::new(),
        };
        assert!(o(60, 50).key() > o(55, 55).key());
        assert!(o(55, 60).key() > o(60, 50).key()); // 5 Instructor is outweighed by 10 Treasure
        assert!(o(50, 80).key() > o(55, 50).key());
    }
}
