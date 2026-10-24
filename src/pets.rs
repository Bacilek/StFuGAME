//! Pets (unlock: level 75 + one Tavern expedition, `docs/pets.md`). Phase 1 (user 2026-10-21): the raw `ownpets.petsSave` numbers
//! (`SimpleSession::pets_save`) are written to `roster/<character>/pets_raw.txt` and summarised in the log whenever they change.
//! Phase 2 (user 2026-10-23): feed the chain pets, then the hourly habitat dungeon fight. The layout of the raw numbers matches sf-api
//! (captured 2026-10-23); the raw numbers are still used because sf-api drops the pets state while `rank` is 0.

use chrono::{Local, NaiveDate};
use sf_api::{
    command::Command,
    gamestate::{GameState, unlockables::HabitatType},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

const HABITATS: [&str; 5] = ["Shadow", "Light", "Earth", "Fire", "Water"];

const HABITAT_TYPES: [HabitatType; 5] =
    [HabitatType::Shadow, HabitatType::Light, HabitatType::Earth, HabitatType::Fire, HabitatType::Water];

/// Level of pet `n` (1..=100) is at index `n + 1`, the number of collected pets at 103 (matches sf-api and the first capture).
fn summary(nums: &[i64]) -> String {
    let collected = nums.get(103).copied().unwrap_or(-1);
    let mut parts = Vec::new();
    for (h, name) in HABITATS.iter().enumerate() {
        let owned: Vec<String> = (0..20)
            .filter_map(|p| {
                let level = nums.get(h * 20 + p + 2).copied().unwrap_or(0);
                (level > 0).then(|| format!("#{}={level}", p + 1))
            })
            .collect();
        if !owned.is_empty() {
            parts.push(format!("{name} [{}]", owned.join(" ")));
        }
    }
    format!("collected {collected}: {}", if parts.is_empty() { "none".to_string() } else { parts.join(", ") })
}

/// Non-zero numbers from index 200 on as `index:value`, to learn the layout of the tail (timers, opponent, fruits per day).
fn tail(nums: &[i64]) -> String {
    nums.iter().enumerate().skip(200).filter(|(_, v)| **v != 0).map(|(i, v)| format!("{i}:{v}")).collect::<Vec<_>>().join(" ")
}

static LAST: crate::ctx::PerChar<Vec<i64>> = crate::ctx::PerChar::new();

/// The latest raw pets numbers of this character (for the Tavern and the scheduler, which have no session at hand); empty before Pets exist.
pub fn latest() -> Vec<i64> {
    LAST.lock().map(|l| l.clone()).unwrap_or_default()
}

pub fn observe(session: &SimpleSession) {
    let Some(nums) = session.pets_save() else { return };
    let Ok(mut last) = LAST.lock() else { return };
    if *last == nums {
        return;
    }
    *last = nums.to_vec();
    report!("[pets] {} | tail {}", summary(nums), tail(nums));
    let path = format!("roster/{}/pets_raw.txt", crate::ctx::name());
    let line = nums.iter().map(ToString::to_string).collect::<Vec<_>>().join("/");
    let _ = std::fs::write(path, line);
}

/// The pets we raise per habitat: (position 1..=20 in the habitat, level to feed to), the user's chains (`docs/pets.md`). The highest one
/// we own is the habitat's main pet; pets outside the chain never get fruits (user 2026-10-23).
const CHAINS: [&[(u32, u16)]; 5] = [
    &[(3, 45), (9, 70), (14, 100), (20, 100)],
    &[(3, 82), (12, 93), (18, 80), (20, 100)],
    &[(3, 100), (14, 100), (20, 100)],
    &[(2, 28), (7, 100), (15, 100), (16, 100), (20, 100)],
    &[(3, 45), (9, 100), (16, 100), (20, 100)],
];

/// Raw `petsSave` indexes: the pet level of id `n` is at `n + 1`, fights won per habitat at `210 + h` (the next enemy is
/// `explored + 1`), the level of that next enemy at `238 + h` (captured 2026-10-23).
pub(crate) fn level(nums: &[i64], id: u32) -> u16 {
    nums.get(id as usize + 1).map_or(0, |l| u16::try_from(*l).unwrap_or(0))
}

pub(crate) fn explored(nums: &[i64], h: usize) -> u32 {
    nums.get(210 + h).map_or(20, |e| u32::try_from(*e).unwrap_or(20))
}

fn enemy_level(nums: &[i64], h: usize) -> u16 {
    nums.get(238 + h).map_or(0, |l| u16::try_from(*l).unwrap_or(0))
}

/// Main pet of a habitat (id, level it should be fed to), or None while we own no chain pet there.
fn main_pet(nums: &[i64], h: usize) -> Option<(u32, u16)> {
    CHAINS[h].iter().rev().map(|(pos, target)| (h as u32 * 20 + pos, *target)).find(|(id, _)| level(nums, *id) > 0)
}

/// Is pet `id` (1..=100) one of the pets of the user's chains (`CHAINS`)?
pub(crate) fn in_chain(id: u32) -> bool {
    let (h, pos) = ((id as usize - 1) / 20, (id - 1) % 20 + 1);
    CHAINS.get(h).is_some_and(|c| c.iter().any(|(p, _)| *p == pos))
}

/// When the Tavern should hold back instead of spending Thirst for Adventure (user 2026-10-23): at night, while only Day pets are
/// missing, the Thirst for Adventure is kept for the morning (`petcatch::wait_until`). None = go on as usual.
pub fn tavern_wait(gs: &GameState) -> Option<chrono::DateTime<Local>> {
    let nums = latest();
    crate::petcatch::wait_until(&nums, &crate::petcatch::events(gs), Local::now(), gs.tavern.thirst_for_adventure_sec)
}

/// Says (at most once an hour) that the Thirst for Adventure is kept for the day.
pub fn report_wait(gs: &GameState) {
    static LAST: crate::ctx::PerChar<Option<chrono::DateTime<Local>>> = crate::ctx::PerChar::new();
    let Some(at) = tavern_wait(gs) else { return };
    let now = Local::now();
    let Ok(mut last) = LAST.lock() else { return };
    if last.is_none_or(|t| now - t >= chrono::Duration::hours(1)) {
        *last = Some(now);
        report!("[pets] No missing pet can be found now, only later today: keeping the Thirst for Adventure until {}", at.format("%H:%M"));
    }
}

/// Seconds until that window opens, for the wake-up schedule.
pub fn secs_until_window(gs: &GameState) -> Option<u64> {
    let at = tavern_wait(gs)?;
    Some(u64::try_from((at - Local::now()).num_seconds()).unwrap_or(0))
}

/// Pets themselves wait as a pending unlock `20/1` (user capture 2026-10-24: Novotne's login carries `unlockfeature:20/1/`, the game client
/// answers the "news" popup with `UnlockFeature`, then `ownpets.petsSave` arrives). Free, so the bot sends it itself.
async fn unlock_pets(session: &mut SimpleSession) -> Outcome {
    let todo: Vec<_> = session
        .game_state()
        .map(|gs| gs.pending_unlocks.iter().filter(|u| u.main_ident == 20 && u.sub_ident == 1).copied().collect())
        .unwrap_or_default();
    for u in todo {
        report!("[pets] Unlocking Pets (pending unlock 20/1)");
        match safe::send(session, Command::UnlockFeature { unlockable: u }).await {
            Ok(_) => report!("[pets] Pets unlock sent, petsSave now present: {}", session.pets_save().is_some()),
            Err(e) => {
                report!("[pets] Pets unlock failed: {e}");
                if crate::tavern::is_session_error(&e) {
                    return Outcome::SessionLost;
                }
            }
        }
    }
    Outcome::Done
}

/// A new pet waits to be taken (`unlockfeature` 21/<pet id> after an egg was picked; the game client sends it when the habitat is
/// opened, user 2026-10-23). Free, so the bot sends it itself. UNVERIFIED that this is what makes the pet ours: the result is logged.
async fn unlock_new(session: &mut SimpleSession) -> Outcome {
    let todo: Vec<_> = session
        .game_state()
        .map(|gs| gs.pending_unlocks.iter().filter(|u| u.main_ident == 21 && (1..=100).contains(&u.sub_ident)).copied().collect())
        .unwrap_or_default();
    for u in todo {
        let before = session.pets_save().map_or(0, |n| n.get(103).copied().unwrap_or(0));
        report!("[pets] Unlocking the new pet with id {}", u.sub_ident);
        if let Err(e) = safe::send(session, Command::UnlockFeature { unlockable: u }).await {
            report!("[pets] Unlocking the new pet failed: {e}");
            if crate::tavern::is_session_error(&e) {
                return Outcome::SessionLost;
            }
            continue;
        }
        let after = session.pets_save().map_or(0, |n| n.get(103).copied().unwrap_or(0));
        let level = session.pets_save().map_or(0, |n| level(n, u32::try_from(u.sub_ident).unwrap_or(0)));
        report!("[pets] Pets collected {before} -> {after}, level of that pet now {level}");
    }
    Outcome::Done
}

/// The strongest pet of a habitat for a dungeon fight: the main pet, else the highest level we own.
fn fighter(nums: &[i64], h: usize) -> Option<(u32, u16)> {
    main_pet(nums, h).map(|(id, _)| (id, level(nums, id))).or_else(|| {
        (1..=20).map(|p| h as u32 * 20 + p).map(|id| (id, level(nums, id))).filter(|(_, l)| *l > 0).max_by_key(|(id, l)| (*l, *id))
    })
}

fn fruits(gs: &GameState, h: usize) -> u16 {
    gs.pets.as_ref().map_or(0, |p| p.habitats[HABITAT_TYPES[h]].fruits)
}

/// Which `fruit_idx` offset the game wants (habitat index + offset): unknown until the first feed was verified (`pets_feed_idx.txt`).
fn confirmed_offset() -> Option<u32> {
    std::fs::read_to_string(format!("roster/{}/pets_feed_idx.txt", crate::ctx::name())).ok()?.trim().parse().ok()
}

/// A feed that went wrong (the wrong fruit left): feeding is off for this character until the file is removed.
fn feeding_off_path() -> String {
    format!("roster/{}/pets_feed_off.txt", crate::ctx::name())
}

/// Pets that refused a feed today (the daily limit, 3 or 9 in an event): no more tries until the next day.
static REFUSED: crate::ctx::PerChar<(Option<NaiveDate>, Vec<u32>)> = crate::ctx::PerChar::new();

fn refused_today(id: u32) -> bool {
    let today = Local::now().date_naive();
    REFUSED.lock().is_ok_and(|r| r.0 == Some(today) && r.1.contains(&id))
}

fn remember_refused(id: u32) {
    let today = Local::now().date_naive();
    if let Ok(mut r) = REFUSED.lock() {
        if r.0 != Some(today) {
            *r = (Some(today), Vec::new());
        }
        r.1.push(id);
    }
}

/// Chain pets we already said "not feeding" about today (one line per pet and day).
static SKIP_SAID: crate::ctx::PerChar<(Option<NaiveDate>, Vec<u32>)> = crate::ctx::PerChar::new();

enum Fed {
    Ok,
    Refused,
    Wrong,
    Lost,
}

/// One fruit to one pet, verified: the level goes up by 1 and exactly the habitat's own fruit stack goes down by 1.
async fn feed_one(session: &mut SimpleSession, id: u32, h: usize) -> Fed {
    let offsets: Vec<u32> = confirmed_offset().map_or_else(|| vec![1, 0], |o| vec![o]);
    for off in offsets {
        let before_level = session.pets_save().map_or(0, |n| level(n, id));
        let before: Vec<u16> = (0..5).map(|k| session.game_state().map_or(0, |gs| fruits(gs, k))).collect();
        let cmd = Command::PetFeed { pet_id: id, fruit_idx: h as u32 + off };
        if let Err(e) = safe::send(session, cmd).await {
            report!("[pets] Feeding pet {id} failed: {e}");
            if crate::tavern::is_session_error(&e) {
                return Fed::Lost;
            }
            continue;
        }
        let after_level = session.pets_save().map_or(0, |n| level(n, id));
        let after: Vec<u16> = (0..5).map(|k| session.game_state().map_or(0, |gs| fruits(gs, k))).collect();
        let used: Vec<usize> = (0..5).filter(|k| after[*k] < before[*k]).collect();
        if after_level > before_level && used == [h] {
            let _ = std::fs::write(format!("roster/{}/pets_feed_idx.txt", crate::ctx::name()), off.to_string());
            return Fed::Ok;
        }
        if after_level == before_level && used.is_empty() {
            // nothing happened: the daily limit, or this offset is not the game's – the next offset gets the same chance
            continue;
        }
        report!(
            "[pets] FEED MISMATCH pet {id} idx {} lvl {before_level}->{after_level} fruit stacks used {used:?} (expected [{h}]); feeding switched off",
            h as u32 + off
        );
        let _ = std::fs::write(feeding_off_path(), format!("pet {id} idx {} used {used:?}", h as u32 + off));
        return Fed::Wrong;
    }
    Fed::Refused
}

/// Feeds the main pet of every habitat up to its target level with the fruits we have (the daily limit stops it by itself). Fruits that
/// are not needed because the target is reached stay in stock.
async fn feed(session: &mut SimpleSession) -> Outcome {
    if std::path::Path::new(&feeding_off_path()).exists() {
        return Outcome::Done;
    }
    // Right after the Pets unlock the fruit stacks (`resources` 12..16) may not be in the state yet: refresh once, and say what we see.
    if session.game_state().is_none_or(|gs| (0..5).all(|h| fruits(gs, h) == 0)) {
        if let Err(e) = safe::send(session, Command::Update).await {
            if crate::tavern::is_session_error(&e) {
                return Outcome::SessionLost;
            }
        }
        let stacks: Vec<u16> = (0..5).map(|h| session.game_state().map_or(0, |gs| fruits(gs, h))).collect();
        report!("[pets] Fruit stacks (Shadow, Light, Earth, Fire, Water): {stacks:?}");
    }
    for h in 0..5 {
        loop {
            let Some(nums) = session.pets_save() else { return Outcome::Done };
            let Some((id, target)) = main_pet(nums, h) else { break };
            let cap = session.game_state().and_then(|gs| gs.pets.as_ref()).map_or(0, |p| p.max_pet_level).max(100);
            let lvl = level(nums, id);
            if lvl < target.min(cap) {
                let stack = session.game_state().map_or(0, |gs| fruits(gs, h));
                let today = Local::now().date_naive();
                let said = SKIP_SAID.lock().is_ok_and(|s| s.0 == Some(today) && s.1.contains(&id));
                if (stack == 0 || refused_today(id)) && !said {
                    report!(
                        "[pets] Not feeding {} pet #{} (id {id}, level {lvl}/{target}): {stack} fruits in stock, refused today: {}",
                        HABITATS[h],
                        id as usize - h * 20,
                        refused_today(id)
                    );
                    if let Ok(mut s) = SKIP_SAID.lock() {
                        if s.0 != Some(today) {
                            *s = (Some(today), Vec::new());
                        }
                        s.1.push(id);
                    }
                }
            }
            if lvl >= target.min(cap) || refused_today(id) || session.game_state().is_none_or(|gs| fruits(gs, h) == 0) {
                break;
            }
            match feed_one(session, id, h).await {
                Fed::Ok => {
                    let l = session.pets_save().map_or(0, |n| level(n, id));
                    report!("[pets] Fed {} pet #{} (id {id}): level {lvl} -> {l} (target {target})", HABITATS[h], id as usize - h * 20);
                }
                Fed::Refused => {
                    remember_refused(id);
                    break;
                }
                Fed::Wrong => return Outcome::Done,
                Fed::Lost => return Outcome::SessionLost,
            }
        }
    }
    Outcome::Done
}

/// Provisionally (user 2026-10-23) habitat fights only while the "Assembly of Awesome Animals" event runs (3x fruits from pet fights).
fn fights_allowed(gs: &GameState) -> bool {
    crate::tasks::event_active(gs, sf_api::gamestate::rewards::Event::AssemblyOfAwesomeAnimals)
}

/// One habitat dungeon fight when it is free: the habitat where our best pet has the biggest lead over the next enemy. A coin flip is
/// fine (user 2026-10-23: another try in an hour); a loss also starts the cooldown.
async fn fight(session: &mut SimpleSession) -> Outcome {
    if !session.game_state().is_some_and(|gs| fights_allowed(gs) && safe::pet_dungeon_is_free(gs)) {
        return Outcome::Done;
    }
    let Some(nums) = session.pets_save() else { return Outcome::Done };
    let best = (0..5)
        .filter(|h| explored(nums, *h) < 20)
        .filter_map(|h| {
            let (id, lvl) = fighter(nums, h)?;
            let enemy = enemy_level(nums, h).max(1);
            Some((h, id, lvl, enemy, f64::from(lvl) / f64::from(enemy)))
        })
        .max_by(|a, b| a.4.total_cmp(&b.4));
    let Some((h, id, lvl, enemy, _)) = best else { return Outcome::Done };
    let pos = explored(nums, h) + 1;
    report!("[pets] Habitat dungeon: {} #{pos} (lvl {enemy}) with pet id {id} (lvl {lvl})", HABITATS[h]);
    let cmd = Command::FightPetDungeon { use_mush: false, habitat: HABITAT_TYPES[h], enemy_pos: pos, player_pet_id: id };
    match safe::send_raw(session, cmd).await {
        Ok(raw) => {
            let me = session.game_state().map_or(0, |gs| gs.character.player_id);
            let won = raw.split('&').find_map(|kv| kv.strip_prefix("winnerid:")).is_some_and(|w| w.trim() == me.to_string());
            let reward = raw.split('&').find_map(|kv| kv.strip_prefix("fightresult.battlereward:")).map(|r| {
                let v: Vec<i64> = r.split('/').filter_map(|s| s.parse().ok()).collect();
                format!(" +{} XP, +{} copper", v.get(3).copied().unwrap_or(0), v.get(2).copied().unwrap_or(0))
            });
            report!("[pets] Habitat dungeon fight: {}{}", if won { "WON" } else { "lost" }, reward.unwrap_or_default());
            Outcome::Done
        }
        Err(e) => {
            report!("[pets] Habitat dungeon fight failed: {e}");
            if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done }
        }
    }
}


// ---- Pet PvP (user 2026-10-24) -------------------------------------------------------------------------------------------------------
// Raw `petsSave` (sf-api layout, verified by the 2026-10-24 loss capture): 223 + h = this habitat already fought today (1), 231 = the offered
// opponent, 232 = when the next fight is free (unix seconds, 0 = now), 233/234 = rank/honor, 235/236 = the opponent's pet count and level
// total (of the day's element), 237 = when he was chosen. One fight per habitat and day, 15 min apart, a win refills a fruit.

/// Our side must beat the opponent's level total by this factor to count as a "good chance"; below it the fight is a sacrifice (user
/// 2026-10-24: "overshoot a bit or undershoot a lot", nothing in between).
const PVP_WIN_RATIO: f64 = 1.2;

/// Type advantage: +25 % damage (user 2026-10-21). Chain Water > Fire > Earth > Light > Shadow > Water; indexes Shadow 0 .. Water 4.
const PVP_TYPE_BONUS: f64 = 1.25;

fn beats(a: usize, b: usize) -> bool {
    matches!((a, b), (4, 3) | (3, 2) | (2, 1) | (1, 0) | (0, 4))
}

/// When the next pet PvP fight is free (None = nothing known / free now).
pub(crate) fn pvp_free_at(nums: &[i64]) -> Option<chrono::DateTime<Local>> {
    use chrono::TimeZone;
    nums.get(232).copied().filter(|t| *t > 0).and_then(|t| Local.timestamp_opt(t, 0).single())
}

/// (pets owned, sum of their levels) of our habitat `h`.
fn side(nums: &[i64], h: usize) -> (u32, u32) {
    (1..=20).map(|p| level(nums, h as u32 * 20 + p)).filter(|l| *l > 0).fold((0, 0), |(n, s), l| (n + 1, s + u32::from(l)))
}

/// The habitat for this fight among those that have not fought today and own a pet. `day` = the defender's element if known.
/// Good chance (strength / opponent total >= `PVP_WIN_RATIO`) -> the WEAKEST such habitat; none -> sacrifice the weakest habitat.
/// Returns (habitat, strength / opponent total).
fn pvp_pick(nums: &[i64], day: Option<usize>) -> Option<(usize, f64)> {
    let opp = f64::from(u32::try_from(nums.get(236).copied().unwrap_or(0)).unwrap_or(0).max(1));
    let strength = |h: usize| {
        let s = f64::from(side(nums, h).1);
        match day {
            Some(d) if beats(h, d) => s * PVP_TYPE_BONUS,
            Some(d) if beats(d, h) => s / PVP_TYPE_BONUS,
            _ => s,
        }
    };
    let open: Vec<(usize, f64)> = (0..5)
        .filter(|h| nums.get(223 + h).copied().unwrap_or(1) == 0 && side(nums, *h).0 > 0)
        .map(|h| (h, strength(h)))
        .collect();
    let by = |a: &&(usize, f64), b: &&(usize, f64)| a.1.total_cmp(&b.1);
    open.iter()
        .filter(|(_, s)| *s / opp >= PVP_WIN_RATIO)
        .min_by(by)
        .or_else(|| open.iter().min_by(by))
        .map(|(h, s)| (*h, *s / opp))
}

/// The day's defender element learned from the first fight (per character): "YYYY-MM-DD <habitat index>".
fn day_element() -> Option<usize> {
    let text = std::fs::read_to_string(format!("roster/{}/pvp_element.txt", crate::ctx::name())).ok()?;
    let (date, h) = text.trim().split_once(' ')?;
    (date == Local::now().format("%Y-%m-%d").to_string()).then(|| h.parse().ok()).flatten().filter(|h| *h < 5)
}

/// The opponent's habitat from the response: his fighter entry `<id>/<id>/<level>/<hp>/<hp>/.../<image>` (field 10), the image id is
/// `-(800 + pet id - 1)`, so the habitat is `(image - 800) / 20` (our own Earth pets -840..-842, his Water pets -880..-882; 2026-10-24).
fn opponent_habitat(raw: &str, opponent: i64) -> Option<usize> {
    let header = raw.split('&').find_map(|kv| kv.strip_prefix("fightheader1.fighters:"))?;
    let t: Vec<i64> = header.split('/').map(|s| s.parse().unwrap_or(i64::MIN)).collect();
    let i = (0..t.len().saturating_sub(10)).find(|i| t[*i] == opponent && t[i + 1] == opponent)?;
    let image = -t[i + 10];
    (800..900).contains(&image).then(|| ((image - 800) / 20) as usize)
}

/// Winner of the whole fight = winner of the last duel (`winnerid<N>.s`, the duels run on with the damage carried over).
fn last_winner(raw: &str) -> Option<i64> {
    raw.split('&')
        .filter_map(|kv| {
            let (key, value) = kv.split_once(':')?;
            let n: u32 = key.strip_prefix("winnerid")?.strip_suffix(".s")?.parse().ok()?;
            Some((n, value.trim().parse::<i64>().ok()?))
        })
        .max_by_key(|(n, _)| *n)
        .map(|(_, w)| w)
}

/// One pet PvP fight when it is free (15 min apart, after the feeding): see `pvp_pick`. Every fight is written to `pvp.log` with all the
/// numbers the choice used, so the ratio threshold can be calibrated from real results.
async fn pvp(session: &mut SimpleSession) -> Outcome {
    let Some(nums) = session.pets_save().map(<[i64]>::to_vec) else { return Outcome::Done };
    let opp = nums.get(231).copied().unwrap_or(0);
    if nums.len() < 240 || opp <= 0 || !safe::cooldown_free(safe::Cooldown::PetPvp, pvp_free_at(&nums)) {
        return Outcome::Done;
    }
    let day = day_element();
    let Some((h, ratio)) = pvp_pick(&nums, day) else { return Outcome::Done };
    let ours: Vec<String> = (0..5).map(|k| format!("{} {}/{}", HABITATS[k], side(&nums, k).0, side(&nums, k).1)).collect();
    let (count, total) = (nums.get(235).copied().unwrap_or(0), nums.get(236).copied().unwrap_or(0));
    let day_text = day.map_or("unknown".to_string(), |d| HABITATS[d].to_string());
    report!("[pets] PvP: opponent {opp} ({count} pets, level total {total}, day element {day_text}); sending {} (strength/opponent {ratio:.2}; ours {})", HABITATS[h], ours.join(", "));
    let Ok(opponent_id) = u32::try_from(opp) else { return Outcome::Done };
    let before_rank = nums.get(233).copied().unwrap_or(0);
    let raw = match safe::send_raw(session, Command::FightPetOpponent { habitat: HABITAT_TYPES[h], opponent_id }).await {
        Ok(raw) => raw,
        Err(e) => {
            report!("[pets] PvP fight failed: {e}");
            return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
        }
    };
    let me = session.game_state().map_or(0, |gs| i64::from(gs.character.player_id));
    let won = last_winner(&raw) == Some(me);
    let reward: Vec<i64> = raw
        .split('&')
        .find_map(|kv| kv.strip_prefix("fightresult.battlereward:"))
        .map(|r| r.split('/').filter_map(|s| s.parse().ok()).collect())
        .unwrap_or_default();
    let item = raw.split('&').find_map(|kv| kv.strip_prefix("battlerewarditem:")).unwrap_or("").to_string();
    let element = opponent_habitat(&raw, opp);
    if let Some(e) = element {
        let _ = std::fs::write(format!("roster/{}/pvp_element.txt", crate::ctx::name()), format!("{} {e}", Local::now().format("%Y-%m-%d")));
    }
    let line = format!(
        "{} with {} ({} pets / level {}), ratio {ratio:.2}, day element {day_text}, opponent's habitat {}: reward {reward:?} item {item}; rank was {before_rank}; ours {}",
        if won { "WON" } else { "lost" },
        HABITATS[h],
        side(&nums, h).0,
        side(&nums, h).1,
        element.map_or("?".to_string(), |e| HABITATS[e].to_string()),
        ours.join(", ")
    );
    report!("[pets] PvP {line}");
    pvp_journal(&format!("opponent {opp} ({count} pets / level {total}): {line}"));
    // The next opponent and the new timer come with an update
    if let Err(e) = safe::send(session, Command::Update).await {
        return if crate::tavern::is_session_error(&e) { Outcome::SessionLost } else { Outcome::Done };
    }
    Outcome::Done
}

fn pvp_journal(line: &str) {
    use std::io::Write;
    let path = crate::ctx::log_path("pvp.log");
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {line}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// Seconds until the next pet PvP fight can start (None = nothing left today or no pets).
fn pvp_secs(nums: &[i64]) -> Option<u64> {
    if nums.len() < 240 || nums.get(231).copied().unwrap_or(0) <= 0 || pvp_pick(nums, None).is_none() {
        return None;
    }
    let t = pvp_free_at(nums).map_or(Local::now(), |t| t + chrono::Duration::seconds(safe::COOLDOWN_SAFETY_SEC));
    Some(u64::try_from((t - Local::now()).num_seconds()).unwrap_or(0))
}

/// Feeding first (so the pets are as strong as possible today), then the hourly dungeon fight.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    if let Outcome::SessionLost = unlock_pets(session).await {
        return Outcome::SessionLost;
    }
    if session.pets_save().is_none() {
        return Outcome::Done;
    }
    if let Outcome::SessionLost = unlock_new(session).await {
        return Outcome::SessionLost;
    }
    if session.game_state().is_none_or(|gs| gs.pets.is_none()) {
        return Outcome::Done;
    }
    if let Outcome::SessionLost = feed(session).await {
        return Outcome::SessionLost;
    }
    if let Outcome::SessionLost = fight(session).await {
        return Outcome::SessionLost;
    }
    pvp(session).await
}

/// Seconds until the pets' dungeon (event only) or the next pet PvP fight is free (with margin), for the wake-up schedule; None while
/// there is nothing to wait for.
pub fn secs_until_ready(gs: &GameState) -> Option<u64> {
    let dungeon = fights_allowed(gs)
        .then(|| gs.pets.as_ref()?.next_free_exploration)
        .flatten()
        .map(|free_at| {
            let t = free_at + chrono::Duration::seconds(safe::COOLDOWN_SAFETY_SEC);
            u64::try_from((t - Local::now()).num_seconds()).unwrap_or(0)
        });
    let nums = latest();
    let fights = if nums.is_empty() { None } else { pvp_secs(&nums) };
    [dungeon, fights].into_iter().flatten().min()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 2026-10-23 capture after the Penguwater fight: Tinck (id 23) lvl 7, Ocodile (id 83) lvl 16, Water explored 4.
    fn capture() -> Vec<i64> {
        let mut n = vec![0; 265];
        n[24] = 7;
        n[84] = 16;
        n[214] = 4;
        n[242] = 14;
        n[239] = 10;
        n
    }

    #[test]
    fn main_pet_is_the_highest_owned_chain_pet() {
        let n = capture();
        assert_eq!(main_pet(&n, 1), Some((23, 82)));
        assert_eq!(main_pet(&n, 4), Some((83, 45)));
        assert_eq!(main_pet(&n, 0), None);
    }

    /// Our pets per habitat for the PvP tests: (habitat, pet position, level); `fought` = habitats that already fought today.
    fn pvp_nums(pets: &[(usize, u32, i64)], fought: &[usize], opp_total: i64) -> Vec<i64> {
        let mut n = vec![0; 265];
        for (h, pos, lvl) in pets {
            n[(*h as u32 * 20 + pos + 1) as usize] = *lvl;
        }
        for h in fought {
            n[223 + h] = 1;
        }
        n[231] = 7755;
        n[235] = 3;
        n[236] = opp_total;
        n
    }

    /// User 2026-10-24: a winnable fight gets the WEAKEST habitat that still overshoots a bit; a hopeless one the weakest habitat.
    #[test]
    fn pvp_sends_the_weakest_habitat_that_still_wins() {
        // Shadow 100, Light 60, Earth 30, Fire 45, Water 20 levels; opponent total 40: Light/Shadow/Fire clear 1.2x, Earth (0.75) does not
        let n = pvp_nums(&[(0, 1, 100), (1, 1, 60), (2, 1, 30), (3, 1, 45), (4, 1, 20)], &[], 40);
        assert_eq!(pvp_pick(&n, None).map(|p| p.0), Some(1), "Fire 45/40 = 1.125 and Earth 0.75 are under 1.2; Light 60/40 = 1.5 is the weakest that clears it");
    }

    #[test]
    fn pvp_hopeless_sacrifices_the_weakest() {
        let n = pvp_nums(&[(0, 1, 100), (1, 1, 60), (2, 1, 30), (3, 1, 45), (4, 1, 20)], &[], 500);
        assert_eq!(pvp_pick(&n, None).map(|p| p.0), Some(4), "Water 20 is the weakest");
        // habitats that fought today are out
        let n = pvp_nums(&[(0, 1, 100), (1, 1, 60), (2, 1, 30), (3, 1, 45), (4, 1, 20)], &[4, 2], 500);
        assert_eq!(pvp_pick(&n, None).map(|p| p.0), Some(3));
    }

    #[test]
    fn pvp_type_advantage_counts() {
        // Day element Water: Shadow beats it (+25 %), Fire loses to it (-20 %): 40 * 1.25 = 50 vs 45, 45 / 1.25 = 36
        let n = pvp_nums(&[(0, 1, 40), (3, 1, 45)], &[], 40);
        assert_eq!(pvp_pick(&n, Some(4)).map(|p| p.0), Some(0));
        assert!(pvp_pick(&n, Some(4)).unwrap().1 > 1.2);
        let n = pvp_nums(&[(3, 1, 45)], &[], 40);
        assert!(pvp_pick(&n, Some(4)).unwrap().1 < 1.0);
    }

    /// The 2026-10-24 loss capture: we (23994) lost 2:3 to 7755 (Water), the last duel decides; his image ids -880.. are Water (habitat 4).
    #[test]
    fn pvp_response_is_read() {
        let raw = "fightheader1.fighters:14/0/0/0/1/23994/23994/1/230/230/23/11/11/23/17/-840/0/0/0/0/0/0/0/0/0/0/0/1/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/0/7755/7755/1/96/96/12/12/24/24/18/-880/0/0/0/0&winnerid1.s:7755&winnerid2.s:23994&winnerid5.s:7755&winnerid4.s:23994";
        assert_eq!(opponent_habitat(raw, 7755), Some(4));
        assert_eq!(opponent_habitat(raw, 23994), Some(2));
        assert_eq!(last_winner(raw), Some(7755));
        assert!(beats(0, 4) && beats(4, 3) && !beats(3, 4));
    }

    #[test]
    fn fight_uses_the_next_enemy_of_the_habitat() {
        let n = capture();
        assert_eq!(explored(&n, 4) + 1, 5);
        assert_eq!(enemy_level(&n, 1), 10);
        assert_eq!(fighter(&n, 4), Some((83, 16)));
    }
}
