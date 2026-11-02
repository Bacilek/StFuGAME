//! Toilet of the Arcane Gods (user 2026-10-25, `docs/toilet.md`): the epics we keep go here. One sacrifice a day (preferably a gem),
//! two during Tidy Toilet Time (preferably an epic); a full backpack makes an epic the preferred sacrifice. Flushing (+1 aura, a random
//! item that the normal inventory pass then equips, dismantles, enchants or sells) whenever the mana reaches the flush cost.
//! Potions and plain items are never thrown.

use chrono::{Local, NaiveDate};
use sf_api::{
    command::Command,
    error::SFError,
    gamestate::{GameState, items::{BagPosition, Item, ItemType}, rewards::Event},
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// Most flushes in one pass (the mana pool is small, this is only a loop guard).
const MAX_FLUSHES: usize = 20;

/// Said once per bot run that the toilet of a level 100+ character is still locked.
static LOCKED_NOTED: crate::ctx::PerChar<bool> = crate::ctx::PerChar::new();

/// When a fresh login was last forced because a level 100+ toilet looked locked.
static RELOGIN_AT: crate::ctx::PerChar<Option<std::time::Instant>> = crate::ctx::PerChar::new();

/// The server sends `arcanetoilet` only with a login or after the unlock; a session older than the unlock never sees it (user 2026-11-02:
/// Pjotr, level 121, toilet open in the game with aura and mana, the bot saw it locked). So a locked 100+ toilet asks for a new login.
const RELOGIN_EVERY: std::time::Duration = std::time::Duration::from_secs(2 * 3600);

/// Marker for "the server refused a sacrifice today".
const REFUSED: u32 = 99;

fn day_file() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("toilet_day.txt")
}

/// Sacrifices made today (`REFUSED` = the server said no); persisted so a restart does not lose count.
fn thrown_today() -> u32 {
    let text = std::fs::read_to_string(day_file()).unwrap_or_default();
    let mut it = text.split_whitespace();
    let (Some(day), Some(n)) = (it.next(), it.next()) else { return 0 };
    if day.parse::<NaiveDate>().ok() != Some(Local::now().date_naive()) {
        return 0;
    }
    n.parse().unwrap_or(0)
}

fn set_thrown_today(n: u32) {
    let _ = std::fs::write(day_file(), format!("{} {n}", Local::now().date_naive()));
}

pub fn event(gs: &GameState) -> bool {
    crate::tasks::event_active(gs, Event::TidyToiletTime)
}

/// An epic equipment piece or a gem may be thrown in; never a unique, a legendary, a potion or a plain item.
pub fn throwable(item: &Item) -> bool {
    match item.typ {
        ItemType::Gem(_) => true,
        _ => item.is_epic() && !item.is_legendary() && !item.is_unique() && item.typ.equipment_slot().is_some(),
    }
}

fn describe(item: &Item) -> String {
    match item.typ {
        ItemType::Gem(g) => format!("{:?} gem", g.typ),
        t => format!("epic {t:?}"),
    }
}

/// The cheapest backpack epic (the dearer ones may still be worth something to the witch or the shop).
fn spare_epic(gs: &GameState) -> Option<(BagPosition, String)> {
    gs.character
        .inventory
        .iter()
        .filter_map(|(pos, i)| Some((pos, i?)))
        .filter(|(_, i)| i.is_epic() && throwable(i) && !matches!(i.typ, ItemType::Gem(_)))
        .min_by_key(|(_, i)| i.price)
        .map(|(pos, i)| (pos, describe(i)))
}

/// What to throw now, or None. `thrown` = sacrifices made today. One a day outside the event (a gem; an epic only with a full backpack),
/// two during Tidy Toilet Time (epics first). The server's `sacrifices_left` decides too, except that a day with no throw yet is tried once
/// (the counter is only refreshed by responses that carry it).
pub fn candidate(gs: &GameState, thrown: u32, need_room: bool) -> Option<(BagPosition, String)> {
    let toilet = gs.tavern.toilet.as_ref().filter(|_| unlocked(gs))?;
    let event = event(gs);
    if thrown >= if event { 2 } else { 1 } || (thrown > 0 && toilet.sacrifices_left == 0) {
        return None;
    }
    let gem = || crate::inventory::spare_gem(gs).map(|(pos, _)| (pos, "gem".to_string()));
    let full = gs.character.inventory.free_slot().is_none();
    if event || full || need_room {
        spare_epic(gs).or_else(gem)
    } else {
        gem()
    }
}

/// Is the toilet really unlocked? sf-api creates `gs.tavern.toilet` already from `toiletstate`, which the server sends to locked
/// characters too (TestChar1, level 81, 2026-10-26: 14 epics rotted in a full backpack because "the toilet owns them"); only
/// `arcanetoilet` with a level > 0 fills `mana_total`, so that is the test.
pub fn unlocked(gs: &GameState) -> bool {
    gs.tavern.toilet.as_ref().is_some_and(|t| t.mana_total > 0)
}

/// Is the toilet in charge of getting rid of epics (unlocked)? Then no epic is sold or dismantled for a free slot, it waits for the next
/// sacrifice (`inventory::last_resort_epic`).
pub fn owns_epics(gs: &GameState) -> bool {
    unlocked(gs)
}

fn state(gs: &GameState) -> String {
    gs.tavern.toilet.as_ref().map_or("none".to_string(), |t| {
        format!("aura {}, mana {}/{}, {} sacrifices left", t.aura, t.mana_currently, t.mana_total, t.sacrifices_left)
    })
}

fn fail(e: &SFError) -> Outcome {
    if crate::tavern::is_session_error(e) {
        return Outcome::SessionLost;
    }
    report!("[toilet] Failed: {e}");
    Outcome::Done
}

/// One sacrifice if there is something to throw (`need_room`: the Tavern needs slots, so an epic goes even with a free slot left).
/// `Some(outcome)` = stop the pass.
async fn sacrifice(session: &mut SimpleSession, need_room: bool) -> Option<Outcome> {
    let thrown = thrown_today();
    if let Some((pos, what)) = session.game_state().and_then(|gs| candidate(gs, thrown, need_room)) {
        let Some(gs) = session.game_state() else { return Some(Outcome::Done) };
        let ident = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()).map(Item::command_ident);
        let was = state(gs);
        match safe::send(session, Command::ToiletDrop { item_pos: pos.into() }).await {
            Err(e) => {
                if matches!(e, SFError::ServerError(_)) {
                    set_thrown_today(REFUSED);
                }
                return Some(fail(&e));
            }
            Ok(_) => {}
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return Some(fail(&e));
        }
        let Some(gs) = session.game_state() else { return Some(Outcome::Done) };
        let gone = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()).map(Item::command_ident) != ident;
        if gone {
            set_thrown_today(thrown + 1);
            report!("[toilet] Threw a {what} in{}: {was} -> {}", if event(gs) { " (Tidy Toilet Time)" } else { "" }, state(gs));
        } else {
            set_thrown_today(REFUSED);
            report!("[toilet] The {what} is still in the backpack after the sacrifice, no more today ({})", state(gs));
        }
    }
    None
}

/// Frees a backpack slot with an epic thrown into the toilet (user 2026-11-02: Pjotr could not take a Tavern reward needing 2 slots, the
/// unlocked toilet stayed unused). `true` = an epic left the backpack.
pub async fn free_slot(session: &mut SimpleSession) -> Result<bool, Outcome> {
    let before = session.game_state().map_or(0, |gs| gs.character.inventory.count_free_slots());
    if let Some(o) = sacrifice(session, true).await {
        return Err(o);
    }
    Ok(session.game_state().is_some_and(|gs| gs.character.inventory.count_free_slots() > before))
}

/// One sacrifice if there is something to throw, then flushes while the mana allows (and a backpack slot is free for the new item).
pub async fn run(session: &mut SimpleSession) -> Outcome {
    // The toilet waits as a pending unlock 32/1 (user capture 2026-11-01, from level 100); the game client answers it with `UnlockFeature`
    // and `arcanetoilet` arrives. Free, so the bot sends it itself.
    let pending: Vec<_> = session
        .game_state()
        .map(|gs| gs.pending_unlocks.iter().filter(|u| u.main_ident == 32 && u.sub_ident == 1).copied().collect())
        .unwrap_or_default();
    for u in pending {
        report!("[toilet] Unlocking the toilet (pending unlock 32/1)");
        if let Err(e) = safe::send(session, Command::UnlockFeature { unlockable: u }).await {
            return fail(&e);
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
        if let Some(gs) = session.game_state() {
            report!("[toilet] After the unlock: {}", state(gs));
        }
    }
    if session.game_state().is_none_or(|gs| !unlocked(gs)) {
        // Once per run, so the state of a locked toilet is visible in the log
        if let Some(gs) = session.game_state().filter(|gs| gs.character.level >= 100)
            && LOCKED_NOTED.lock().is_ok_and(|mut seen| !std::mem::replace(&mut *seen, true))
        {
            report!("[toilet] Locked at level {}: {}", gs.character.level, state(gs));
            let pending: Vec<String> = gs.pending_unlocks.iter().map(|u| format!("{}/{}", u.main_ident, u.sub_ident)).collect();
            crate::alerts::raise_limited(
                "toilet",
                "toilet-locked",
                "Bot vidí záchod jako zamčený (level 100+), epiky se neházejí.",
                &format!("level {}, state: {}, pending unlocks: {:?}", gs.character.level, state(gs), pending),
                720,
            );
        }
        let high = session.game_state().is_some_and(|gs| gs.character.level >= 100 && !gs.pending_unlocks.iter().any(|u| u.main_ident == 32));
        // The first sighting only starts the clock (the login that just happened would have brought `arcanetoilet`)
        let due = high && RELOGIN_AT.lock().is_ok_and(|mut at| match *at {
            None => {
                *at = Some(std::time::Instant::now());
                false
            }
            Some(t) if t.elapsed() >= RELOGIN_EVERY => {
                *at = Some(std::time::Instant::now());
                true
            }
            Some(_) => false,
        });
        if due {
            report!("[toilet] Looks locked at level 100+, asking for a fresh login");
            return Outcome::SessionLost;
        }
        return Outcome::Done;
    }
    if let Some(o) = sacrifice(session, false).await {
        return o;
    }
    flush(session).await
}

async fn flush(session: &mut SimpleSession) -> Outcome {
    for _ in 0..MAX_FLUSHES {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some(t) = gs.tavern.toilet.as_ref() else { return Outcome::Done };
        if t.mana_total == 0 || t.mana_currently < t.mana_total || gs.character.inventory.free_slot().is_none() {
            return Outcome::Done;
        }
        let before: Vec<_> = gs.character.inventory.iter().map(|(_, i)| i.map(Item::command_ident)).collect();
        let was = state(gs);
        if let Err(e) = safe::send(session, Command::ToiletFlush).await {
            return fail(&e);
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return fail(&e);
        }
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let got: Vec<String> = gs
            .character
            .inventory
            .iter()
            .zip(before)
            .filter_map(|((_, i), b)| (i.map(Item::command_ident) != b).then_some(i).flatten())
            .map(|i| format!("{:?}{}", i.typ, if i.is_epic() { " (epic)" } else { "" }))
            .collect();
        let line = format!("[toilet] Flushed: {was} -> {}; got {}", state(gs), if got.is_empty() { "nothing visible".into() } else { got.join(", ") });
        report!("{line}");
        crate::inventory::journal(&line);
    }
    Outcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_api::gamestate::items::ItemType;

    #[test]
    fn only_epic_equipment_and_gems_may_be_thrown() {
        assert!(throwable(&crate::smith::test_item(ItemType::Belt, 60)));
        assert!(!throwable(&crate::smith::test_item(ItemType::Belt, 5)), "a plain item");
        assert!(!throwable(&crate::smith::test_item(ItemType::Belt, 95)), "a legendary");
    }
}
