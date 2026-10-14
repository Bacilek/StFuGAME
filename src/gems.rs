//! Gem Mine (user 2026-10-12, rules in `docs/fortress.md`): start a gem search whenever the mine is idle, collect the gem
//! when there is room for it, and socket it where it helps most.
//!
//! Collect (`FortressGemStoneSearchFinish`, always `mushrooms: 0`) a finished gem when
//!  * at least 2 backpack slots are free (the gem takes one, at least one stays free), or
//!  * 1 slot is free but the gem will be socketed right away (an empty socket, or a worse gem to replace).
//!
//! A search starts when the mine is idle and the gold left afterwards is at least 2x the shop reserve plus what the fortress
//! wants for its next build (user 2026-10-14; wood/stone are ignored). Up to 1 gem (2 with 6+ free slots) is held in the
//! backpack for a future item with a slot. Luck/secondary gems go to the item we replace soonest, Constitution/main gems to
//! the best, hardest-to-replace one.
//!
//! Worth of a gem for a class (`tier`, lower is better): 0 = main attribute / "all attributes" / legendary, 1 = Constitution,
//! 2 = Luck, 3 = a secondary attribute. Socketing (`choose_socket`): an empty socket, or a filled one holding a tier 2-3 gem
//! that is strictly worse than the new gem (a Constitution or main-attribute gem is never replaced). Among the candidate
//! items the one that will last longest wins: epic first, then the higher item value. A gem with no place stays in the
//! backpack (user: save it if it does not get in the way). Socketing is a plain `PlayerItemMove` backpack -> equipment slot
//! (captured 2026-10-12; the exact request line is still to be confirmed, see docs/fortress.md).

use chrono::{DateTime, Local};
use sf_api::{
    command::{AttributeType, Command},
    gamestate::{
        GameState,
        character::Class,
        fortress::{FortressBuildingType as B, FortressResourceType as R},
        items::{BagPosition, EquipmentSlot, Gem, GemSlot, GemType, ItemCommandIdent, ItemType},
    },
};

use crate::{inventory, safe, session::SimpleSession, tavern::Outcome};

const EVERY: chrono::Duration = chrono::Duration::minutes(10);

static NEXT: crate::ctx::PerChar<Option<DateTime<Local>>> = crate::ctx::PerChar::new();
/// `gem_search.finish` of the last gem we collected: the server does not clear it, so without this a collected gem would
/// look ready forever.
static COLLECTED: crate::ctx::PerChar<Option<DateTime<Local>>> = crate::ctx::PerChar::new();

/// Also kept on disk (`roster/<character>/gem_collected.txt`) so a restart does not try to collect the same gem again.
fn collected_path() -> std::path::PathBuf {
    std::path::Path::new("roster").join(crate::ctx::name()).join("gem_collected.txt")
}

fn last_collected() -> Option<DateTime<Local>> {
    if let Some(t) = COLLECTED.lock().ok().and_then(|c| *c) {
        return Some(t);
    }
    let t = std::fs::read_to_string(collected_path()).ok()?.trim().parse::<DateTime<Local>>().ok()?;
    if let Ok(mut c) = COLLECTED.lock() {
        *c = Some(t);
    }
    Some(t)
}

fn mark_collected(t: DateTime<Local>) {
    if let Ok(mut c) = COLLECTED.lock() {
        *c = Some(t);
    }
    let _ = std::fs::write(collected_path(), t.to_rfc3339());
}
/// Backpack gems held for a future item (1; 2 when at least this many slots are free).
const HOLD_TWO_FREE: usize = 6;
/// A search leaves at least this multiple of the shop reserve in gold (user 2026-10-14; wood and stone do not matter, a
/// search costs next to nothing in them) on top of what the fortress wants for its next build.
const SEARCH_RESERVE_MULTIPLE: u64 = 2;
/// A socketing that did not change anything stops further attempts for the rest of the day (unverified request shape).
static SOCKET_FAILED_ON: crate::ctx::PerChar<Option<chrono::NaiveDate>> = crate::ctx::PerChar::new();

/// 0 = best ... 3 = worst for this class.
fn tier(typ: GemType, class: Class) -> u8 {
    let main = match class.main_attribute() {
        AttributeType::Strength => GemType::Strength,
        AttributeType::Dexterity => GemType::Dexterity,
        _ => GemType::Intelligence,
    };
    match typ {
        t if t == main => 0,
        GemType::All | GemType::Legendary => 0,
        GemType::Constitution => 1,
        GemType::Luck => 2,
        _ => 3,
    }
}

/// A socket candidate: what it holds now and how long the item will last.
#[derive(Debug, Clone, Copy)]
struct Socket {
    content: Option<Gem>,
    epic: bool,
    value: f64,
}

/// The socket for `gem` (index into `sockets`), or None (keep the gem).
fn choose_socket(sockets: &[Socket], gem: Gem, class: Class) -> Option<usize> {
    let new = tier(gem.typ, class);
    let fit = sockets
        .iter()
        .enumerate()
        .filter(|(_, s)| match s.content {
            None => true,
            Some(old) => {
                let t = tier(old.typ, class);
                t >= 2 && t > new
            }
        });
    let by = |a: &(usize, &Socket), b: &(usize, &Socket)| (a.1.epic, a.1.value).partial_cmp(&(b.1.epic, b.1.value)).unwrap_or(std::cmp::Ordering::Equal);
    // Constitution and main-attribute gems go to the item that is best and hardest to replace; Luck and secondary gems to the
    // item we will swap out soonest (user 2026-10-14)
    (if new <= 1 { fit.max_by(by) } else { fit.min_by(by) }).map(|(i, _)| i)
}

/// Equipped items with a gem slot.
fn sockets(gs: &GameState) -> Vec<(EquipmentSlot, Socket)> {
    gs.character
        .equipment
        .0
        .iter()
        .filter_map(|(slot, item)| {
            let item = item.as_ref()?;
            let content = match item.gem_slot? {
                GemSlot::Empty => None,
                GemSlot::Filled(g) => Some(g),
            };
            Some((slot, Socket { content, epic: item.is_epic(), value: inventory::value(gs, item) }))
        })
        .collect()
}

fn backpack_gem(gs: &GameState) -> Option<(BagPosition, Gem, ItemCommandIdent)> {
    gs.character.inventory.iter().find_map(|(pos, item)| {
        let item = item?;
        match item.typ {
            ItemType::Gem(g) => Some((pos, g, item.command_ident())),
            _ => None,
        }
    })
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[gems] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

pub async fn run(session: &mut SimpleSession) -> Outcome {
    {
        let Ok(mut next) = NEXT.lock() else { return Outcome::Done };
        let now = Local::now();
        if next.is_some_and(|t| now < t) {
            return Outcome::Done;
        }
        let Some(gs) = session.game_state() else { return Outcome::Done };
        if gs.fortress.as_ref().is_none_or(|f| f.buildings[B::GemMine].level == 0) {
            return Outcome::Done;
        }
        *next = Some(now + EVERY);
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        return fail(&e);
    }

    // 1. A gem in the backpack: socket it if there is a good place
    if let Some(o) = socket_backpack_gem(session).await {
        return o;
    }

    // 2. A finished search: collect when there is room
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    let now = Local::now();
    let collected = last_collected();
    let finished = f.gem_search.finish.filter(|t| *t <= now && collected != Some(*t));
    if let Some(t) = finished {
        let free = gs.character.inventory.count_free_slots();
        let class = gs.character.class;
        let list: Vec<Socket> = sockets(gs).iter().map(|s| s.1).collect();
        let fits_now = f.gem_search.target.is_some_and(|typ| choose_socket(&list, Gem { typ, value: 0 }, class).is_some());
        // Holding a gem for a future item with a free slot is fine: 1, or 2 with plenty of room (user 2026-10-14)
        let held = gs.character.inventory.iter().filter(|(_, i)| i.is_some_and(|i| matches!(i.typ, ItemType::Gem(_)))).count();
        let cap = if free >= HOLD_TWO_FREE { 2 } else { 1 };
        if fits_now && free >= 1 || free >= 2 && held < cap {
            report!("[gems] Collecting the mined gem ({:?}, {free} free backpack slots)", f.gem_search.target);
            if let Err(e) = safe::send(session, Command::FortressGemStoneSearchFinish { mushrooms: 0 }).await {
                return fail(&e);
            }
            mark_collected(t);
            if let Err(e) = safe::send(session, Command::Update).await {
                return fail(&e);
            }
            if let Some(o) = socket_backpack_gem(session).await {
                return o;
            }
        } else {
            report!("[gems] A gem is ready but the backpack has {free} free slot(s) and no socket wants it, leaving it in the mine");
            return Outcome::Done;
        }
    }

    // 3. Start the next search when the mine is idle
    let Some(gs) = session.game_state() else { return Outcome::Done };
    let Some(f) = gs.fortress.as_ref() else { return Outcome::Done };
    let collected = last_collected();
    let idle = f.gem_search.finish.is_none_or(|t| t <= Local::now() && collected == Some(t));
    if !idle || !crate::shops::pass_done() {
        return Outcome::Done;
    }
    let cost = f.gem_search.cost;
    let (wood, stone) = (f.resources[R::Wood].current, f.resources[R::Stone].current);
    let keep = SEARCH_RESERVE_MULTIPLE * crate::shops::reserve() + crate::fortress::gold_needed(gs);
    if gs.character.silver < cost.silver + keep || wood < cost.wood || stone < cost.stone {
        return Outcome::Done;
    }
    report!("[gems] Starting a gem search (wood {}, stone {}, {})", cost.wood, cost.stone, crate::report::gold(cost.silver));
    if let Err(e) = safe::send(session, Command::FortressGemStoneSearch).await {
        return fail(&e);
    }
    Outcome::Done
}

/// Sockets the gem in the backpack (if any and if a place exists). `Some(outcome)` = stop the pass.
async fn socket_backpack_gem(session: &mut SimpleSession) -> Option<Outcome> {
    let today = Local::now().date_naive();
    if SOCKET_FAILED_ON.lock().is_ok_and(|d| *d == Some(today)) {
        return None;
    }
    let gs = session.game_state()?;
    let (pos, gem, ident) = backpack_gem(gs)?;
    let class = gs.character.class;
    // A stored item that was kept only for its empty gem slot (inventory::keep_for_gem) and beats what is worn once it has
    // this gem: socket it there first, the inventory logic then equips it (user 2026-10-12)
    let stored = gs
        .character
        .inventory
        .iter()
        .filter_map(|(bag, item)| {
            let item = item?;
            let eq_slot = item.typ.equipment_slot()?;
            if !item.can_be_equipped_by(class) || !matches!(item.gem_slot, Some(GemSlot::Empty)) {
                return None;
            }
            let cur = gs.character.equipment.0[eq_slot].as_ref().map_or(-1.0, |c| inventory::value(gs, c));
            let gain = inventory::value_with_gem(gs, item, eq_slot, gem) - cur;
            (gain > 0.0).then_some((bag, gain))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((target, gain)) = stored {
        report!("[gems] Socketing {:?} {} into a stored backpack item (it will be worth {gain:.1} more than the worn one)", gem.typ, gem.value);
        if let Err(e) = safe::send(session, Command::PlayerItemMove { from: pos.into(), to: target.into(), item_ident: ident }).await {
            report!("[gems] Socketing into the stored item failed: {e}");
            if let Ok(mut d) = SOCKET_FAILED_ON.lock() {
                *d = Some(today);
            }
            return Some(fail(&e));
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return Some(fail(&e));
        }
        if session.game_state().and_then(backpack_gem).is_some_and(|g| g.1 == gem) {
            report!("[gems] The gem is still in the backpack after the move, no more socketing today (request shape unverified)");
            if let Ok(mut d) = SOCKET_FAILED_ON.lock() {
                *d = Some(today);
            }
        }
        return None;
    }
    let list = sockets(gs);
    let candidates: Vec<Socket> = list.iter().map(|s| s.1).collect();
    let i = choose_socket(&candidates, gem, class)?; // None = keep the gem
    let slot = list[i].0;
    report!(
        "[gems] Socketing {:?} {} into {slot:?} (now: {})",
        gem.typ,
        gem.value,
        candidates[i].content.map_or("empty".to_string(), |g| format!("{:?} {}", g.typ, g.value))
    );
    if let Err(e) = safe::send(session, Command::PlayerItemMove { from: pos.into(), to: slot.into(), item_ident: ident }).await {
        report!("[gems] Socketing failed: {e}");
        if let Ok(mut d) = SOCKET_FAILED_ON.lock() {
            *d = Some(today);
        }
        return Some(fail(&e));
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        return Some(fail(&e));
    }
    if session.game_state().and_then(backpack_gem).is_some() {
        report!("[gems] The gem is still in the backpack after the move, no more socketing today (request shape unverified)");
        if let Ok(mut d) = SOCKET_FAILED_ON.lock() {
            *d = Some(today);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gem(typ: GemType) -> Gem {
        Gem { typ, value: 50 }
    }
    fn sock(content: Option<GemType>, epic: bool, value: f64) -> Socket {
        Socket { content: content.map(gem), epic, value }
    }

    #[test]
    fn tiers_for_a_bard() {
        // Bard: Intelligence is the main attribute
        assert_eq!(tier(GemType::Intelligence, Class::Bard), 0);
        assert_eq!(tier(GemType::All, Class::Bard), 0);
        assert_eq!(tier(GemType::Legendary, Class::Bard), 0);
        assert_eq!(tier(GemType::Constitution, Class::Bard), 1);
        assert_eq!(tier(GemType::Luck, Class::Bard), 2);
        assert_eq!(tier(GemType::Strength, Class::Bard), 3);
    }

    #[test]
    fn best_lasting_item_gets_the_gem_and_good_gems_are_not_replaced() {
        let c = Class::Bard;
        // an empty socket on a weak item and one on an epic: the epic wins
        let list = [sock(None, false, 90.0), sock(None, true, 40.0)];
        assert_eq!(choose_socket(&list, gem(GemType::Intelligence), c), Some(1));
        // a Constitution gem is never replaced, not even by a main-attribute gem
        assert_eq!(choose_socket(&[sock(Some(GemType::Constitution), true, 99.0)], gem(GemType::Intelligence), c), None);
        // a Luck gem is replaced by Constitution, a secondary one by Luck, but not the other way round
        assert_eq!(choose_socket(&[sock(Some(GemType::Luck), false, 1.0)], gem(GemType::Constitution), c), Some(0));
        assert_eq!(choose_socket(&[sock(Some(GemType::Strength), false, 1.0)], gem(GemType::Luck), c), Some(0));
        assert_eq!(choose_socket(&[sock(Some(GemType::Luck), false, 1.0)], gem(GemType::Strength), c), None);
        // a secondary gem into nothing but a Luck socket: kept in the backpack
        assert_eq!(choose_socket(&[sock(Some(GemType::Luck), false, 1.0)], gem(GemType::Dexterity), c), None);
        // no sockets at all
        assert_eq!(choose_socket(&[], gem(GemType::Intelligence), c), None);
    }

    #[test]
    fn weak_gems_go_to_the_item_replaced_soonest() {
        let c = Class::Bard;
        // two empty sockets: a weak item (value 10) and a strong epic (value 90)
        let list = [sock(None, false, 10.0), sock(None, true, 90.0)];
        // a Luck or secondary gem goes to the weak item, Constitution and main-attribute gems to the strong one
        assert_eq!(choose_socket(&list, gem(GemType::Luck), c), Some(0));
        assert_eq!(choose_socket(&list, gem(GemType::Strength), c), Some(0));
        assert_eq!(choose_socket(&list, gem(GemType::Constitution), c), Some(1));
        assert_eq!(choose_socket(&list, gem(GemType::Intelligence), c), Some(1));
    }
}
