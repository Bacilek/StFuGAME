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
    for h in 0..5 {
        loop {
            let Some(nums) = session.pets_save() else { return Outcome::Done };
            let Some((id, target)) = main_pet(nums, h) else { break };
            let cap = session.game_state().and_then(|gs| gs.pets.as_ref()).map_or(0, |p| p.max_pet_level).max(100);
            let lvl = level(nums, id);
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

/// Feeding first (so the pets are as strong as possible today), then the hourly dungeon fight.
pub async fn run(session: &mut SimpleSession) -> Outcome {
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
    fight(session).await
}

/// Seconds until the pets' dungeon is free (with margin), for the wake-up schedule; None while Pets are not unlocked.
pub fn secs_until_ready(gs: &GameState) -> Option<u64> {
    if !fights_allowed(gs) {
        return None;
    }
    let free_at = gs.pets.as_ref()?.next_free_exploration?;
    let t = free_at + chrono::Duration::seconds(safe::COOLDOWN_SAFETY_SEC);
    Some(u64::try_from((t - Local::now()).num_seconds()).unwrap_or(0))
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

    #[test]
    fn fight_uses_the_next_enemy_of_the_habitat() {
        let n = capture();
        assert_eq!(explored(&n, 4) + 1, 5);
        assert_eq!(enemy_level(&n, 1), 10);
        assert_eq!(fighter(&n, 4), Some((83, 16)));
    }
}
