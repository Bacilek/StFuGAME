//! Game of Dice (user 2026-11-06, `docs/dice.md`): unlocks with the Tower, one free game an hour, 10 a day (20 during Lucky Day; the
//! server's `remaining` decides). A game = one roll of 5 dice, then one re-roll in which we hold one type and re-roll the rest; 3+ equal
//! dice pay. After 23:00 hourglasses skip the wait so all of the day's games get played (no reserve, user). Never mushrooms, never a paid re-roll.

use chrono::{DateTime, Duration, Local, Timelike};
use sf_api::{
    command::{Command, DiceType, RollDicePrice},
    error::SFError,
    gamestate::GameState,
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// Seconds between two free games (user: "every hour like a dungeon"; the first capture showed 3001 s left ~10 min after a roll).
const COOLDOWN_SEC: i64 = 3600;
/// From this hour on, a wait may be skipped with an hourglass when the day's games would not fit otherwise.
const SKIP_FROM_HOUR: u32 = 23;
/// Loop guard (20 games at most on a Lucky Day).
const MAX_GAMES_PER_PASS: usize = 25;
/// After a failed game the next try comes only after this long.
const RETRY: std::time::Duration = std::time::Duration::from_secs(15 * 60);

static FAILED_AT: crate::ctx::PerChar<Option<std::time::Instant>> = crate::ctx::PerChar::new();

/// Is the game open? Locked characters get no `dice` key (remaining 0, no time).
pub fn unlocked(gs: &GameState) -> bool {
    let d = &gs.tavern.dice_game;
    d.remaining > 0 || d.next_free.is_some()
}

/// Free games that still fit before midnight, counting from `next_free` (the first at that time, then one an hour).
fn free_capacity_at(next_free: Option<DateTime<Local>>, now: DateTime<Local>) -> i64 {
    let start = next_free.map_or(now, |t| t + Duration::seconds(safe::COOLDOWN_SAFETY_SEC)).max(now);
    let midnight = (now.date_naive() + Duration::days(1)).and_hms_opt(0, 0, 0).and_then(|m| m.and_local_timezone(Local).earliest());
    let Some(midnight) = midnight else { return 1 };
    if start >= midnight { 0 } else { (midnight - start).num_seconds() / COOLDOWN_SEC + 1 }
}

/// Pay an hourglass for the next game? Only from 23:00, with games left that the free ones cannot cover before midnight.
fn skip_wanted_at(remaining: u8, next_free: Option<DateTime<Local>>, glasses: u32, now: DateTime<Local>) -> bool {
    now.hour() >= SKIP_FROM_HOUR && remaining > 0 && glasses > 0 && i64::from(remaining) > free_capacity_at(next_free, now)
}

/// Used by `safe.rs` before it lets an hourglass-paid first roll through.
pub fn skip_with_hourglass(gs: &GameState) -> bool {
    let d = &gs.tavern.dice_game;
    skip_wanted_at(d.remaining, d.next_free, gs.tavern.quicksand_glasses, Local::now())
}

/// Seconds until the next game could be played (free time, or 23:00 when skipping may start); `None` = nothing to do today.
pub fn secs_until_ready(gs: &GameState) -> Option<u64> {
    if !unlocked(gs) || gs.tavern.dice_game.remaining == 0 {
        return None;
    }
    let now = Local::now();
    let free = gs.tavern.dice_game.next_free.map_or(0, |t| (t + Duration::seconds(safe::COOLDOWN_SAFETY_SEC) - now).num_seconds().max(0));
    let to_skip = if now.hour() < SKIP_FROM_HOUR {
        let at = now.date_naive().and_hms_opt(SKIP_FROM_HOUR, 0, 0).and_then(|t| t.and_local_timezone(Local).earliest());
        at.map_or(free, |t| (t - now).num_seconds().max(0))
    } else {
        free
    };
    Some(u64::try_from(free.min(to_skip)).unwrap_or(0).max(60))
}

/// Value of a die face when choosing what to hold (user 2026-11-06 gave only: epic above all, wood uninteresting, gold a little;
/// the rest is provisional). An unknown face (> 6) is the event's epic guess.
fn rank(face: u8, epic_event: bool) -> u8 {
    match face {
        4 if epic_event => 100, // during Epic Good Luck the user saw an epic on the face sf-api calls Souls (capture 2026-11-06)
        5 => 6,      // Arcane
        4 => 5,      // Souls
        6 => 4,      // Hourglass
        1 => 3,      // Silver
        2 => 2,      // Stone
        3 => 1,      // Wood
        0 => 0,      // nothing
        _ => 100,    // unknown = epic
    }
}

/// What to hold after the first roll: one slot value per die, 0 = re-roll. An unknown face (epic) wins over everything; otherwise
/// the most frequent face, ties by value.
fn choose_hold(faces: &[u8], epic_event: bool) -> [u8; 5] {
    let mut hold = [0u8; 5];
    let Some(best) = faces
        .iter()
        .copied()
        .filter(|f| *f != 0)
        .max_by_key(|f| (rank(*f, epic_event) == 100, faces.iter().filter(|x| *x == f).count(), rank(*f, epic_event)))
    else {
        return hold;
    };
    for (slot, f) in hold.iter_mut().zip(faces) {
        if *f == best {
            *slot = best;
        }
    }
    hold
}

fn face_name(face: u8) -> String {
    match face {
        0 => "-".into(),
        1 => "Silver".into(),
        2 => "Stone".into(),
        3 => "Wood".into(),
        4 => "Souls/Epic".into(),
        5 => "Arcane".into(),
        6 => "Hourglass".into(),
        n => format!("UNKNOWN({n})"),
    }
}

fn names(faces: &[u8]) -> String {
    faces.iter().map(|f| face_name(*f)).collect::<Vec<_>>().join(" ")
}

fn dice_type(face: u8) -> Option<DiceType> {
    Some(match face {
        0 => DiceType::ReRoll,
        1 => DiceType::Silver,
        2 => DiceType::Stone,
        3 => DiceType::Wood,
        4 => DiceType::Souls,
        5 => DiceType::Arcane,
        6 => DiceType::Hourglass,
        _ => return None,
    })
}

/// The roll command; a face sf-api has no name for (the event's epic?) goes out as a custom command of the same wire shape.
fn roll_cmd(payment: RollDicePrice, hold: [u8; 5]) -> Command {
    let types: Option<Vec<DiceType>> = hold.iter().map(|f| dice_type(*f)).collect();
    if let Some(t) = types.and_then(|t| <[DiceType; 5]>::try_from(t).ok()) {
        return Command::RollDice { payment, dices: t };
    }
    let mut arguments = vec![(payment as u8).to_string()];
    arguments.extend(hold.iter().map(u8::to_string));
    Command::Custom { cmd_name: "RollDice".into(), arguments }
}

/// The value list of a response key, e.g. `dicestatus:6/1/6/3/1`.
fn key_values(raw: &str, key: &str) -> Vec<u8> {
    raw.split('&')
        .find_map(|part| part.strip_prefix(key).and_then(|p| p.strip_prefix(':')))
        .map(|v| v.split('/').filter_map(|n| n.trim().parse::<u8>().ok()).collect())
        .unwrap_or_default()
}

/// `dicereward:<face+1>/<amount>` -> (face, amount).
fn reward(raw: &str) -> Option<(u8, u64)> {
    let v = raw.split('&').find_map(|part| part.strip_prefix("dicereward:"))?;
    let mut it = v.split('/');
    let face = it.next()?.trim().parse::<u8>().ok()?.checked_sub(1)?;
    Some((face, it.next()?.trim().parse().ok()?))
}

fn fail(e: &SFError) -> Outcome {
    if crate::tavern::is_session_error(e) {
        return Outcome::SessionLost;
    }
    report!("[dice] Failed: {e}");
    if let Ok(mut at) = FAILED_AT.lock() {
        *at = Some(std::time::Instant::now());
    }
    Outcome::Done
}

fn odd(title: &str, detail: &str) {
    crate::alerts::raise_limited("dice", title, &format!("Kostky: {title}"), detail, 360);
}

/// One game: the first roll (`payment`), then the re-roll holding the best type. `Ok(true)` = played, go on.
async fn play(session: &mut SimpleSession, payment: RollDicePrice) -> Result<bool, Outcome> {
    let raw = safe::send_raw(session, roll_cmd(payment, [0; 5])).await.map_err(|e| fail(&e))?;
    let faces = key_values(&raw, "dicestatus");
    if faces.len() != 5 {
        report!("[dice] First roll without a usable dicestatus: {raw:.200}");
        odd("první hod bez dicestatus", &raw.chars().take(300).collect::<String>());
        fail(&SFError::InvalidRequest("no dicestatus"));
        return Ok(false);
    }
    if faces.iter().any(|f| *f > 6) {
        odd("neznámá kostka (epic?)", &format!("dicestatus {faces:?}"));
    }
    let epic_event = session.game_state().is_some_and(|gs| crate::tasks::event_active(gs, sf_api::gamestate::rewards::Event::EpicGoodLuckExtravaganza));
    let hold = choose_hold(&faces, epic_event);
    let raw2 = safe::send_raw(session, roll_cmd(RollDicePrice::Free, hold)).await.map_err(|e| fail(&e))?;
    let after = key_values(&raw2, "dicestatus");
    let won = reward(&raw2);
    if let Err(e) = safe::send(session, Command::Update).await {
        return Err(fail(&e));
    }
    let left = session.game_state().map_or(0, |gs| gs.tavern.dice_game.remaining);
    let paid = if payment == RollDicePrice::Hourglass { " (hourglass)" } else { "" };
    let line = format!(
        "[dice] Rolled{paid}: {} | held {} | now {} | {} | {left} games left today",
        names(&faces),
        names(&hold),
        names(&after),
        won.map_or("no reward".to_string(), |(f, n)| format!("won {n} {}", face_name(f))),
    );
    report!("{line}");
    crate::inventory::journal(&line);
    // No three of a kind = no reward is normal; three of a kind without one is not
    if won.is_none() && after.iter().any(|f| *f != 0 && after.iter().filter(|x| *x == f).count() >= 3) {
        odd("výhra se nepřipsala", &line);
    }
    Ok(true)
}

/// Plays every game that is due now: free ones off cooldown, hourglass ones from 23:00 when the day's games would not fit.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    if FAILED_AT.lock().is_ok_and(|at| at.is_some_and(|t| t.elapsed() < RETRY)) {
        return Outcome::Done;
    }
    for _ in 0..MAX_GAMES_PER_PASS {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        if !unlocked(gs) || gs.tavern.dice_game.remaining == 0 {
            return Outcome::Done;
        }
        // 4 epics paid an epic weapon earlier today (user 2026-11-06): an item reward needs a free backpack slot
        if crate::tasks::event_active(gs, sf_api::gamestate::rewards::Event::EpicGoodLuckExtravaganza) && gs.character.inventory.free_slot().is_none() {
            report!("[dice] Backpack full during Epic Good Luck, an epic reward would have no slot - waiting");
            return Outcome::Done;
        }
        let payment = if safe::cooldown_free(safe::Cooldown::Dice, gs.tavern.dice_game.next_free) {
            RollDicePrice::Free
        } else if skip_with_hourglass(gs) {
            RollDicePrice::Hourglass
        } else {
            return Outcome::Done;
        };
        match play(session, payment).await {
            Ok(true) => {}
            Ok(false) => return Outcome::Done,
            Err(o) => return o,
        }
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 11, 6, h, m, 0).unwrap()
    }

    #[test]
    fn holds_the_most_frequent_face_then_the_dearer_one() {
        assert_eq!(choose_hold(&[6, 1, 6, 3, 1], false), [6, 0, 6, 0, 0], "hourglass pair beats the silver pair");
        assert_eq!(choose_hold(&[3, 3, 3, 5, 1], false), [3, 3, 3, 0, 0]);
        assert_eq!(choose_hold(&[1, 2, 3, 4, 5], false), [0, 0, 0, 0, 5], "all different: the dearest one");
    }

    #[test]
    fn holds_the_arcane_pair_of_capture_3() {
        assert_eq!(choose_hold(&[2, 3, 5, 2, 5], false), [0, 0, 5, 0, 5]);
        assert_eq!(reward("&dicestatus:6/6/5/5/5&dicereward:6/778&"), Some((5, 778)));
    }

    #[test]
    fn the_epic_face_wins_during_the_epic_event() {
        // capture 4: 2 3 4 5 6, the user saw Stone Wood Epic Splinters Hourglass and kept the epic
        assert_eq!(choose_hold(&[2, 3, 4, 5, 6], true), [0, 0, 4, 0, 0]);
        assert_eq!(choose_hold(&[2, 3, 4, 5, 6], false), [0, 0, 0, 5, 0], "without the event Souls rank below Arcane");
        assert_eq!(choose_hold(&[4, 5, 5, 1, 2], true), [4, 0, 0, 0, 0], "a single epic beats a pair");
    }

    #[test]
    fn an_unknown_face_is_held_over_everything() {
        assert_eq!(choose_hold(&[7, 5, 5, 5, 1], false), [7, 0, 0, 0, 0]);
    }

    #[test]
    fn parses_the_captured_responses() {
        let raw = "owntower.towerSave:1&dicestatus:6/3/6/3/3&dicereward:4/4968&timestamp:1";
        assert_eq!(key_values(raw, "dicestatus"), vec![6, 3, 6, 3, 3]);
        assert_eq!(reward(raw), Some((3, 4968)));
    }

    #[test]
    fn hourglasses_only_from_23_and_only_when_the_day_does_not_fit() {
        // 22:30, 3 games left, next free 22:40: hour < 23 -> never
        assert!(!skip_wanted_at(3, Some(at(22, 40)), 5, at(22, 30)));
        // 23:05, 3 left, next free 23:30: free capacity 1 (23:30), the rest need hourglasses
        assert!(skip_wanted_at(3, Some(at(23, 30)), 5, at(23, 5)));
        // 23:05, 1 left, next free 23:30: the free one fits
        assert!(!skip_wanted_at(1, Some(at(23, 30)), 5, at(23, 5)));
        // no hourglasses: nothing to pay with
        assert!(!skip_wanted_at(3, Some(at(23, 30)), 0, at(23, 5)));
    }
}
