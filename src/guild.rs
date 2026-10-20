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
const SWITCH_MARGIN: u32 = 20;
/// How often the quick-join list is checked (a free guild with better bonuses can appear any time).
const CHECK_EVERY: chrono::Duration = chrono::Duration::hours(6);
/// Without a guild the list is re-checked this often (a failed join, a kick): every hour without Instructor/Treasure
/// bonuses is wasted (user 2026-10-12, PajaRizz), the list is only a few requests.
/// While the Exceptional XP or Glorious Gold event runs the guild bonuses count double / five-fold (user 2026-10-16): look more often.
const EVENT_CHECK_EVERY: chrono::Duration = chrono::Duration::hours(2);
/// While Exceptional XP runs, levels are what counts (user 2026-10-16: the characters in good guilds pulled ahead): look every 15 min,
/// a slot in a better guild opens whenever a member leaves.
const XP_EVENT_CHECK_EVERY: chrono::Duration = chrono::Duration::minutes(15);
/// Without a guild the list is re-checked this often (see below)
pub const NO_GUILD_RETRY: chrono::Duration = chrono::Duration::minutes(5);
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

/// Free slots a guild must keep for us to LEAVE another guild for it (see `claimed_slots`).
const LEAVE_SPARE_SLOTS: u32 = 3;
/// How long a slot claimed by one of our characters counts against the guild's free places (the quick-join list is cached).
const CLAIM_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);
/// Guilds our characters (all in this process) just decided to join: (guild name, when).
static CLAIMS: std::sync::Mutex<Vec<(String, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());

fn claim_slot(guild: &str) {
    if let Ok(mut c) = CLAIMS.lock() {
        c.retain(|(_, t)| t.elapsed() < CLAIM_FOR);
        c.push((guild.to_string(), std::time::Instant::now()));
    }
}

fn claimed_slots(guild: &str) -> u32 {
    CLAIMS.lock().map_or(0, |c| c.iter().filter(|(g, t)| g == guild && t.elapsed() < CLAIM_FOR).count() as u32)
}

static LAST_CHECK: crate::ctx::PerChar<Option<DateTime<Local>>> = crate::ctx::PerChar::new();

/// Every join/leave/kick goes to `roster/<character>/logs/guild.log` as well: `progress.log` keeps only 100 messages, so a
/// join-leave loop (Pagan 2026-10-17/18, a guild's leader complained) could not be reconstructed afterwards.
fn glog(msg: String) {
    use std::io::Write;
    report!("{msg}");
    let path = crate::ctx::log_path("guild.log");
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {msg}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// Guilds we do not join again for a while: ones we left ourselves (`LEFT_AVOID`) and ones that kicked us (`KICKED_AVOID`).
/// Kept per character in `roster/<character>/guild_avoid.txt` (`name<TAB>until-rfc3339`) and the last guild we saw us in in
/// `guild_last.txt`, so a kick is noticed even across a restart (user 2026-10-18: Pagan kept coming back to a guild whose
/// members found it annoying; the bonuses had not changed, so it was not the switch margin).
const LEFT_AVOID: chrono::Duration = chrono::Duration::hours(24);
/// A guild that answered `group has no matching join slots` (it has join conditions we do not meet).
const JOIN_SLOTS_AVOID: chrono::Duration = chrono::Duration::hours(12);
const KICKED_AVOID: chrono::Duration = chrono::Duration::days(7);

fn char_file(name: &str) -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join(name)
}

fn avoided() -> Vec<String> {
    let now = Local::now();
    std::fs::read_to_string(char_file("guild_avoid.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (name, until) = l.split_once('\t')?;
            (until.parse::<DateTime<Local>>().ok()? > now).then(|| name.to_string())
        })
        .collect()
}

fn avoid(name: &str, how_long: chrono::Duration) {
    let now = Local::now();
    let mut lines: Vec<String> = std::fs::read_to_string(char_file("guild_avoid.txt"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.split_once('\t').is_some_and(|(n, u)| n != name && u.parse::<DateTime<Local>>().is_ok_and(|u| u > now)))
        .map(str::to_string)
        .collect();
    lines.push(format!("{name}\t{}", (now + how_long).to_rfc3339()));
    let _ = std::fs::write(char_file("guild_avoid.txt"), lines.join("\n") + "\n");
}

/// Notices a kick (we had a guild last time, now none, and did not leave on purpose) and remembers the current guild.
fn track_membership(current: Option<&str>) {
    let last = std::fs::read_to_string(char_file("guild_last.txt")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    match (last, current) {
        (Some(l), None) => {
            glog(format!("[guild] No longer in {l} (kicked or left): not joining it again for {} days", KICKED_AVOID.num_days()));
            avoid(&l, KICKED_AVOID);
            let _ = std::fs::remove_file(char_file("guild_last.txt"));
        }
        (l, Some(c)) if l.as_deref() != Some(c) => {
            let _ = std::fs::write(char_file("guild_last.txt"), c);
        }
        _ => {}
    }
}

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

/// Instructor / Treasure of our own guild, summed over the members from a fresh `ViewGuild` (same measure as the quick-join
/// list). None when the guild cannot be looked up.
async fn fresh_own_totals(session: &mut SimpleSession) -> Option<(u32, u32)> {
    let name = session.game_state()?.guild.as_ref()?.name.clone();
    let gs = safe::send(session, Command::ViewGuild { guild_ident: name.clone() }).await.ok()?;
    let g = gs.lookup.guilds.get(&name)?;
    if g.members.is_empty() {
        return None;
    }
    let instructor = g.members.iter().map(|m| u32::from(m.instructor_lvl)).sum();
    let treasure = g.members.iter().map(|m| u32::from(m.treasure_lvl)).sum();
    Some((instructor, treasure))
}

/// Every `CHECK_EVERY`: join the best guild, or switch to a clearly better one.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    let now = Local::now();
    {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        track_membership(gs.guild.as_ref().map(|g| g.name.as_str()));
        // A task wants a guild fight: stay (checked before the timer, so we leave right after the task is done)
        if gs.guild.is_some() && guild_fight_task_open(gs) {
            return Outcome::Done;
        }
        let Ok(mut last) = LAST_CHECK.lock() else { return Outcome::Done };
        let event = [Event::ExceptionalXPEvent, Event::GloriousGoldGalore].into_iter().any(|e| crate::tasks::event_active(gs, e));
        let every = if gs.guild.is_none() {
            NO_GUILD_RETRY
        } else if crate::tasks::event_active(gs, Event::ExceptionalXPEvent) {
            XP_EVENT_CHECK_EVERY
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
    // Our own totals are read fresh (user 2026-10-17: Mimimimi11 left Smietanka shakes, which really had 652/680, because the
    // cached totals from the save showed 500/478; the cache is as old as the login). Cannot read them -> stay.
    let in_guild = session.game_state().is_some_and(|gs| gs.guild.is_some());
    let fresh = if in_guild { fresh_own_totals(session).await } else { None };
    if in_guild && fresh.is_none() {
        report!("[guild] Could not read the totals of our own guild, staying (no switch on stale numbers)");
        return Outcome::Done;
    }
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let current = gs.guild.as_ref().map(|g| {
        let (i, t) = fresh.unwrap_or((u32::from(g.total_instructor_skill), u32::from(g.total_treasure_skill)));
        if (i, t) != (u32::from(g.total_instructor_skill), u32::from(g.total_treasure_skill)) {
            report!("[guild] Own totals: cached {}/{}, fresh {i}/{t} (Instructor/Treasure) – using the fresh ones", g.total_instructor_skill, g.total_treasure_skill);
        }
        (g.name.clone(), i, t)
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
    // What the list holds at the top, joinable or not, so "why are we not in a better guild" can be read from the log
    let mut best: Vec<&Offer> = offers.iter().collect();
    best.sort_by_key(|o| std::cmp::Reverse(o.key()));
    let top: Vec<String> = best
        .iter()
        .take(3)
        .map(|o| {
            let why = if o.members >= MAX_MEMBERS {
                " FULL"
            } else if unmet_requirement(&o.description, own) {
                " REQUIREMENT"
            } else {
                ""
            };
            format!("{} (Instructor {}, Treasure {}, {} members{why})", o.name, o.instructor, o.treasure, o.members)
        })
        .collect();
    report!("[guild] Top of the list ({} guilds): {}", offers.len(), top.join("; "));
    let avoid_list = avoided();
    let all = candidates(offers.clone(), current.as_ref().map(|(n, i, t)| (n.as_str(), *i, *t)), own);
    let mut list: Vec<Offer> = all.iter().filter(|o| !avoid_list.contains(&o.name)).cloned().collect();
    // Never stay without a guild (user 2026-10-18): when the only joinable guilds are avoided ones, join the best of them anyway
    if list.is_empty() && current.is_none() {
        list = all;
    }

    // Leaving first and joining second is risky when our own characters head for the same guild at once (2026-10-20 22:47:
    // Pagan and Sanek left Smietanka shakes for FANCY at 49/50 members, Wecros and Pjotr took the free slots first, both
    // ended in a worse guild or none). When leaving, only go to a guild that keeps `LEAVE_SPARE_SLOTS` free slots after the
    // members it already has plus the slots our other characters just claimed.
    if current.is_some() {
        list.retain(|o| o.members + claimed_slots(&o.name) + LEAVE_SPARE_SLOTS <= MAX_MEMBERS);
    }
    match &current {
        Some((name, instructor, treasure)) if list.is_empty() => {
            report!(
                "[guild] Staying in {name} (Instructor {instructor}, Treasure {treasure}), no joinable guild is {SWITCH_MARGIN}/{SWITCH_MARGIN} better"
            );
            return Outcome::Done;
        }
        Some((name, instructor, treasure)) => {
            glog(format!(
                "[guild] Leaving {name} (Instructor {instructor}, Treasure {treasure}) for a better guild: {}",
                describe(&list[0])
            ));
            claim_slot(&list[0].name);
            avoid(name, LEFT_AVOID);
            let _ = std::fs::remove_file(char_file("guild_last.txt"));
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

    for (attempt, offer) in list.iter().take(MAX_ATTEMPTS).enumerate() {
        glog(format!("[guild] Joining {}", describe(offer)));
        match safe::send(session, custom("GroupJoin", &[&offer.raw_name, "int"])).await {
            Ok(gs) if gs.guild.as_ref().is_some_and(|g| g.name == offer.name) => {
                glog(format!("[guild] Joined {}", offer.name));
                let _ = std::fs::write(char_file("guild_last.txt"), &offer.name);
                return Outcome::Done;
            }
            Ok(_) => glog(format!("[guild] Joining {} did not work (full?), trying the next one", offer.name)),
            Err(e) if crate::tavern::is_session_error(&e) => return Outcome::SessionLost,
            Err(e) => {
                glog(format!("[guild] Joining {} failed: {e}", offer.name));
                let msg = e.to_string();
                if msg.contains("no matching join slots") {
                    // The guild's own join conditions (higher guild upgrades, level…) are not in the list: remember it, so the next
                    // round starts with a guild we have not been refused by (user 2026-10-20, Wecros: FF15 headed the list every 5 min)
                    avoid(&offer.name, JOIN_SLOTS_AVOID);
                } else if msg.contains("cannot do this right now") && attempt > 0 {
                    // Every attempt after a refused one got this answer (Wecros 2026-10-20 while STC took him manually): the server
                    // seems to block further joins for a while after a failed one, so do not burn the rest of the list
                    glog("[guild] The server refuses further joins right after a failed one, the next round starts afresh".to_string());
                    break;
                }
            }
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

/// One-time purchase of the Instructor and Treasure levels 5 → 6 (user 2026-10-16: guild descriptions ask for "6/6" and the
/// user wants us to be competitive and wanted members) and 6 → 7 (user 2026-10-19: more competitiveness on the guild "job market",
/// the Instructor's extra XP pays off in the long run). An upgrade costs 500 g + 2 mushrooms (5 → 6) or 1000 g + 4 mushrooms
/// (6 → 7), so it is a narrow, priced exception to the "never spend mushrooms" rule: `safe.rs` lets exactly this command through
/// (`six_price_ok`) and the mushroom watchdog allows exactly the price. Pet is not part of it. Done once per character, skill and
/// step (`roster/<character>/guild_six.txt`: a line `Instructor` = 6 done, `Instructor7` = 7 done).
/// (from level, max silver in copper, max mushrooms); the price the server shows must be within these caps.
const STEPS: [(u16, u64, u16); 2] = [(5, 50_000, 2), (6, 100_000, 4)];

fn step(current: u16) -> Option<(u64, u16)> {
    STEPS.iter().find(|s| s.0 == current).map(|s| (s.1, s.2))
}

/// May this exact guild upgrade (own skill `current` → `current + 1`) be paid with mushrooms? Only Instructor/Treasure at level 5
/// or 6, a price within the caps of that step that we can pay now.
pub fn six_price_ok(gs: &sf_api::gamestate::GameState, skill: sf_api::gamestate::guild::GuildSkill, current: u16) -> bool {
    use sf_api::gamestate::guild::GuildSkill;
    let Some(g) = gs.guild.as_ref() else { return false };
    let own = match skill {
        GuildSkill::Instructor => g.own_instructor_skill,
        GuildSkill::Treasure => g.own_treasure_skill,
        GuildSkill::Pet => return false,
    };
    let Some((max_silver, max_mushrooms)) = step(current) else { return false };
    let p = &g.upgrade_price[skill];
    own == current
        && p.mushrooms > 0
        && p.mushrooms <= max_mushrooms
        && p.silver <= max_silver
        && p.silver <= gs.character.silver
        && gs.character.mushrooms >= u32::from(p.mushrooms)
}

fn six_marker() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("guild_six.txt")
}

/// The marker line of the step that starts at `from` (5 → "Instructor", 6 → "Instructor7").
fn six_key(skill: sf_api::gamestate::guild::GuildSkill, from: u16) -> String {
    if from == 5 { format!("{skill:?}") } else { format!("{skill:?}{}", from + 1) }
}

fn six_done(skill: sf_api::gamestate::guild::GuildSkill, from: u16) -> bool {
    let key = six_key(skill, from);
    std::fs::read_to_string(six_marker()).is_ok_and(|t| t.lines().any(|l| l.trim() == key))
}

fn six_mark(skill: sf_api::gamestate::guild::GuildSkill, from: u16) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(six_marker()) {
        let _ = writeln!(f, "{}", six_key(skill, from));
    }
}

/// Buys the missing levels 6 and 7 of Instructor and Treasure, once the shop reserve is covered.
pub async fn six_bonuses(session: &mut SimpleSession) -> Outcome {
    use sf_api::gamestate::guild::GuildSkill;
    for skill in [GuildSkill::Instructor, GuildSkill::Treasure] {
        for from in STEPS.map(|s| s.0) {
            let Some(gs) = session.game_state() else { return Outcome::Done };
            let Some(g) = gs.guild.as_ref() else { return Outcome::Done };
            if six_done(skill, from) {
                continue;
            }
            let own = if skill == GuildSkill::Instructor { g.own_instructor_skill } else { g.own_treasure_skill };
            if own > from {
                six_mark(skill, from); // already past this step (bought for gold at some point or by hand)
                continue;
            }
            if own != from || !six_price_ok(gs, skill, own) {
                break; // a lower step is not done yet (or unaffordable): the next one must wait
            }
            let p = &g.upgrade_price[skill];
            if gs.character.silver < p.silver + crate::shops::reserve() {
                break;
            }
            report!(
                "[guild] Buying {skill:?} {own} → {} for {} and {} mushrooms (user: guild bonuses {}/{})",
                own + 1,
                crate::report::gold(p.silver),
                p.mushrooms,
                own + 1,
                own + 1
            );
            if let Err(e) = safe::send(session, Command::GuildIncreaseSkill { skill, current: own }).await {
                report!("[guild] Error: {e}");
                return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
            }
            if let Err(e) = safe::send(session, Command::Update).await {
                report!("[guild] Error: {e}");
                return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
            }
            let now = session.game_state().and_then(|gs| gs.guild.as_ref()).map(|g| {
                if skill == GuildSkill::Instructor { g.own_instructor_skill } else { g.own_treasure_skill }
            });
            if now.is_some_and(|n| n > own) {
                six_mark(skill, from);
                report!("[guild] {skill:?} is now {}", own + 1);
            } else {
                break;
            }
        }
    }
    Outcome::Done
}

#[cfg(test)]
mod six_tests {
    use super::*;

    /// Mushroom-priced guild upgrades are allowed only for 5 → 6 (500 g, 2 mushrooms) and 6 → 7 (1000 g, 4 mushrooms), never beyond.
    #[test]
    fn only_the_two_priced_steps_exist() {
        assert_eq!(step(5), Some((50_000, 2)));
        assert_eq!(step(6), Some((100_000, 4)));
        assert_eq!(step(4), None);
        assert_eq!(step(7), None);
    }

    #[test]
    fn marker_keys_keep_the_old_six_format() {
        use sf_api::gamestate::guild::GuildSkill;
        assert_eq!(six_key(GuildSkill::Instructor, 5), "Instructor");
        assert_eq!(six_key(GuildSkill::Treasure, 6), "Treasure7");
    }
}

// ---- Guild-only XP activities (user 2026-10-19) ----
// The Instructor bonus (+100 % XP at the top) makes expeditions and Dungeons worth far more inside a guild, almost a permanent XP event,
// so a guildless character pauses both and keeps trying to join. Late in the evening the Tavern override applies: Thirst for Adventure
// must not go to waste at midnight. Dungeons stay paused all day.

/// From this hour on, expeditions run even without a guild (user: "after 10 in the evening").
pub const NO_GUILD_TAVERN_FROM_HOUR: u32 = 22;

/// Dungeons only while in a guild (no evening override).
pub fn dungeons_allowed(gs: &sf_api::gamestate::GameState) -> bool {
    gs.guild.is_some()
}

/// May a NEW expedition start? In a guild always; without one only from 22:00 on. A running expedition is always finished.
pub fn expeditions_allowed(gs: &sf_api::gamestate::GameState) -> bool {
    use chrono::Timelike;
    gs.guild.is_some() || Local::now().hour() >= NO_GUILD_TAVERN_FROM_HOUR
}

static PAUSE_REPORTED: crate::ctx::PerChar<Option<DateTime<Local>>> = crate::ctx::PerChar::new();

/// Says (at most once an hour per character) that the guild-only activities wait for a guild.
pub fn report_pause(what: &str) {
    let now = Local::now();
    let Ok(mut last) = PAUSE_REPORTED.lock() else { return };
    if last.is_none_or(|t| now - t >= chrono::Duration::hours(1)) {
        *last = Some(now);
        report!("[guild] No guild: {what} paused until we join one (the guild check runs every {} min)", NO_GUILD_RETRY.num_minutes());
    }
}

#[cfg(test)]
mod guild_only_tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn guildless_character_has_no_dungeons() {
        let gs = sf_api::gamestate::GameState::default();
        assert!(gs.guild.is_none());
        assert!(!dungeons_allowed(&gs));
        // expeditions: only the evening override can open them
        assert_eq!(expeditions_allowed(&gs), Local::now().hour() >= NO_GUILD_TAVERN_FROM_HOUR);
    }
}
