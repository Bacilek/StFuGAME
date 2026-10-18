//! Gem Mine (user 2026-10-12, rules in `docs/fortress.md`): start a gem search whenever the mine is idle, collect the gem
//! when there is room for it, and socket it where it helps most.
//!
//! Collect (`FortressGemStoneSearchFinish`, always `mushrooms: 0`) a finished gem when at least 1 backpack slot is free (user
//! 2026-10-18; a gem nobody wants is sold right after, so the mine never stalls on a nearly full backpack).
//!
//! A search starts when the mine is idle and the gold left afterwards is at least 2x the shop reserve plus what the fortress
//! wants for its next build (user 2026-10-14; wood/stone are ignored). Gems are held in the
//! backpack for a future item with a slot, as many as fit while one slot stays free (user 2026-10-16). Luck/secondary gems go to the item we replace soonest, Constitution/main gems to
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
    /// Weapon or gear with own main/CON/LCK (`inventory::worth_a_gem`); other items never get a gem (user 2026-10-15).
    worth: bool,
    /// The main-hand weapon: its gem counts double (user 2026-10-16), so it gets the strongest gem.
    weapon: bool,
}

/// Strength of a gem for this class (main + side effect, as `gem_effect` counts it).
fn gem_power(class: Class, g: Gem) -> f64 {
    let (m, o) = inventory::gem_effect(class, g);
    m + o
}

/// The weapon's socket when `gem` (main-attribute/All/Legendary or Constitution) belongs there (user 2026-10-16: a weapon gem counts
/// double, so a weapon with a gem slot always carries the strongest main gem, else a Constitution one): an empty socket, or one
/// holding a worse gem (a better tier, or the same tier and more power). The replaced gem is lost.
fn weapon_socket(sockets: &[Socket], gem: Gem, class: Class) -> Option<usize> {
    let new = tier(gem.typ, class);
    if new > 1 {
        return None;
    }
    sockets.iter().position(|s| {
        s.weapon
            && s.worth
            && s.content.is_none_or(|old| {
                let t = tier(old.typ, class);
                new < t || (new == t && gem_power(class, gem) > gem_power(class, old))
            })
    })
}

/// The socket for `gem` (index into `sockets`), or None (keep the gem).
fn choose_socket(sockets: &[Socket], gem: Gem, class: Class) -> Option<usize> {
    let new = tier(gem.typ, class);
    if new >= 3 {
        return None; // secondary-attribute gems are sold, not socketed (user 2026-10-16)
    }
    if let Some(i) = weapon_socket(sockets, gem, class) {
        return Some(i);
    }
    let fit = sockets
        .iter()
        .enumerate()
        .filter(|(_, s)| s.worth)
        .filter(|(_, s)| match s.content {
            None => true,
            Some(old) => {
                let t = tier(old.typ, class);
                t >= 2 && t > new
            }
        })
        .collect::<Vec<_>>();
    // A socket that is empty or holds a secondary-attribute gem costs nothing to fill; a Luck gem is only replaced when there is no
    // such socket (user 2026-10-17, trainer: a Strength gem went into the boots' secondary socket, not over the gloves' Luck gem)
    let free = |s: &Socket| s.content.is_none_or(|g| tier(g.typ, class) >= 3);
    let fit = if fit.iter().any(|(_, s)| free(s)) { fit.into_iter().filter(|(_, s)| free(s)).collect() } else { fit };
    let fit = fit.into_iter();
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
            let worth = inventory::worth_a_gem(gs.character.class, item);
            Some((slot, Socket { content, epic: item.is_epic(), value: inventory::value(gs, item), worth, weapon: slot == EquipmentSlot::Weapon }))
        })
        .collect()
}

/// Loose gems in the backpack, best first for this class (tier, then strength): the strongest gem is placed first, so it
/// lands on the best item (user 2026-10-16).
fn backpack_gems(gs: &GameState) -> Vec<(BagPosition, Gem, ItemCommandIdent)> {
    let class = gs.character.class;
    let mut gems: Vec<_> = gs
        .character
        .inventory
        .iter()
        .filter_map(|(pos, item)| {
            let item = item?;
            match item.typ {
                ItemType::Gem(g) => Some((pos, g, item.command_ident())),
                _ => None,
            }
        })
        .collect();
    let power = |g: &Gem| {
        let (m, o) = inventory::gem_effect(class, *g);
        m + o
    };
    gems.sort_by(|a, b| tier(a.1.typ, class).cmp(&tier(b.1.typ, class)).then(power(&b.1).total_cmp(&power(&a.1))));
    gems
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[gems] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

pub async fn run(session: &mut SimpleSession) -> Outcome {
    {
        let Ok(mut next) = NEXT.lock() else { return Outcome::Done };
        let now = Local::now();
        // A search that just finished is collected at once, not at the next 10-minute slot (the "find a gem" task has to
        // be done and its chest claimed before midnight)
        let ready = session
            .game_state()
            .and_then(|gs| gs.fortress.as_ref())
            .and_then(|f| f.gem_search.finish)
            .is_some_and(|t| t <= now && last_collected() != Some(t));
        if !ready && next.is_some_and(|t| now < t) {
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
        // Collected with ONE free slot as well (user 2026-10-18): a gem nobody wants is sold right after (below), so the mine never
        // stalls on a nearly full backpack; only a completely full one waits
        if free >= 1 {
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
            // A gem collected on the last free slot that nobody wanted after all is sold, one slot stays free (user 2026-10-16)
            if let Some(o) = sell_unplaced_gem(session).await {
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
    if !idle {
        return Outcome::Done;
    }
    let cost = f.gem_search.cost;
    let (wood, stone) = (f.resources[R::Wood].current, f.resources[R::Stone].current);
    // With the "find a gem" task reaching a gold chest (`tasks::gem_task_wanted`) the 150 g are all that is needed, no reserve
    // (a search takes 50 min: from 23:00 it would no longer be collected today, so the usual reserve applies then)
    let keep = if chrono::Timelike::hour(&Local::now()) < 23 && crate::tasks::gem_task_wanted(gs, cost.silver) {
        0
    } else {
        SEARCH_RESERVE_MULTIPLE * crate::shops::reserve() + crate::fortress::gold_needed(gs)
    };
    if gs.character.silver < cost.silver + keep || wood < cost.wood || stone < cost.stone {
        return Outcome::Done;
    }
    report!("[gems] Starting a gem search (wood {}, stone {}, {})", cost.wood, cost.stone, crate::report::gold(cost.silver));
    if let Err(e) = safe::send(session, Command::FortressGemStoneSearch).await {
        return fail(&e);
    }
    Outcome::Done
}

/// The backpack is completely full after a collect and the gem found no socket: sell the weakest loose gem (user 2026-10-16:
/// gems may be held in any number, but one slot always stays free for Dungeons loot).
async fn sell_unplaced_gem(session: &mut SimpleSession) -> Option<Outcome> {
    let gs = session.game_state()?;
    let (pos, what) = inventory::last_resort_gem(gs)?;
    let ident = gs.character.inventory.backpack.get(pos.backpack_pos())?.as_ref()?.command_ident();
    report!("[gems] {what}");
    if let Err(e) = safe::send(session, Command::SellShop { item_pos: pos.into(), item_ident: ident }).await {
        return Some(fail(&e));
    }
    if let Err(e) = safe::send(session, Command::Update).await {
        return Some(fail(&e));
    }
    None
}

/// Loose secondary-attribute gems (tier 3) are sold at once (user 2026-10-16): they are never socketed and not worth a slot.
async fn sell_secondary_gems(session: &mut SimpleSession) -> Option<Outcome> {
    loop {
        let gs = session.game_state()?;
        let class = gs.character.class;
        let (pos, gem, ident) = backpack_gems(gs).into_iter().find(|g| tier(g.1.typ, class) >= 3)?;
        report!("[gems] Selling the loose secondary-attribute gem {:?} {}", gem.typ, gem.value);
        if let Err(e) = safe::send(session, Command::SellShop { item_pos: pos.into(), item_ident: ident }).await {
            return Some(fail(&e));
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return Some(fail(&e));
        }
        safe::human_pause().await;
    }
}

/// Frees a backpack slot with the loose gems (user 2026-10-18): they are the FIRST thing to go when room is needed for anything
/// (epic, potion, ...). A gem with a place is socketed (worn item worse), otherwise the weakest one is sold. No active selling
/// otherwise: loose gems just sit there until a slot is wanted. `true` = a slot was freed.
pub async fn free_slot(session: &mut SimpleSession) -> Result<bool, Outcome> {
    let before = session.game_state().map_or(0, |gs| backpack_gems(gs).len());
    if before == 0 {
        return Ok(false);
    }
    if let Some(o) = socket_backpack_gem(session).await {
        return Err(o);
    }
    let Some(gs) = session.game_state() else { return Ok(false) };
    if backpack_gems(gs).len() < before {
        return Ok(true);
    }
    let Some((pos, what)) = inventory::spare_gem(gs) else { return Ok(false) };
    let Some(ident) = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()).map(|i| i.command_ident()) else {
        return Ok(false);
    };
    report!("[gems] {what}");
    safe::send(session, Command::SellShop { item_pos: pos.into(), item_ident: ident }).await.map_err(|e| fail(&e))?;
    safe::send(session, Command::Update).await.map_err(|e| fail(&e))?;
    Ok(true)
}

/// Sockets the gems lying in the backpack, strongest first, one per socket, until none has a place left (user 2026-10-16:
/// gems are not kept back while a socket is empty, mining goes on). `Some(outcome)` = stop the pass.
async fn socket_backpack_gem(session: &mut SimpleSession) -> Option<Outcome> {
    if let Some(o) = sell_secondary_gems(session).await {
        return Some(o);
    }
    let today = Local::now().date_naive();
    for _ in 0..12 {
        if SOCKET_FAILED_ON.lock().is_ok_and(|d| *d == Some(today)) {
            return None;
        }
        let gs = session.game_state()?;
        let class = gs.character.class;
        let gems = backpack_gems(gs);
        let before = gems.len();
        let list = sockets(gs);
        let candidates: Vec<Socket> = list.iter().map(|s| s.1).collect();
        // First gem (best first) that has a place: a stored item kept for its empty gem slot that beats the worn one once it
        // has this gem (user 2026-10-12: socket it there first, the inventory logic then equips it), else a worn socket
        let mut placed = None;
        // The weapon takes the strongest main gem first (old order: tier, power); every other place goes to the gem that adds the most
        // right now, main/CON balance included (user 2026-10-16: with Constitution lacking, a CON gem beats a main one)
        let mut by_gain = gems.clone();
        by_gain.sort_by(|a, b| inventory::gem_gain(gs, b.1).total_cmp(&inventory::gem_gain(gs, a.1)));
        let order: Vec<_> = gems.iter().filter(|g| weapon_socket(&candidates, g.1, class).is_some()).chain(by_gain.iter()).cloned().collect();
        for (pos, gem, ident) in order {
            // The worn weapon comes first (its gem counts double, user 2026-10-16), before any stored item
            if let Some(i) = weapon_socket(&candidates, gem, class) {
                let slot = list[i].0;
                report!(
                    "[gems] Socketing {:?} {} into the weapon (now: {})",
                    gem.typ,
                    gem.value,
                    candidates[i].content.map_or("empty".to_string(), |g| format!("{:?} {}", g.typ, g.value))
                );
                placed = Some((Command::PlayerItemMove { from: pos.into(), to: slot.into(), item_ident: ident }, "weapon"));
                break;
            }
            // Only the item kept for a gem (`inventory::keep_candidate`: a worn item without any slot would be beaten once it has this gem)
            let stored = inventory::kept_item(gs).and_then(|bag| {
                let item = gs.character.inventory.backpack.get(bag.backpack_pos())?.as_ref()?;
                let eq_slot = item.typ.equipment_slot()?;
                let cur = gs.character.equipment.0[eq_slot].as_ref().map_or(-1.0, |c| inventory::value(gs, c));
                let gain = inventory::value_with_gem(gs, item, eq_slot, gem) - cur;
                (gain > 0.0).then_some((bag, gain, eq_slot, item.command_ident()))
            });
            if let Some((target, gain, eq_slot, item_ident)) = stored {
                // A gem can only go into a WORN item (user 2026-10-17): put the kept item on first, then socket it
                report!("[gems] Putting on the item kept for a gem, then socketing {:?} {} into it (it will be worth {gain:.1} more than the worn one)", gem.typ, gem.value);
                if let Err(e) = safe::send(session, Command::Equip { from_pos: target.into(), to_slot: eq_slot, item_ident }).await {
                    report!("[gems] Putting on the kept item failed: {e}");
                    return Some(fail(&e));
                }
                if let Err(e) = safe::send(session, Command::Update).await {
                    return Some(fail(&e));
                }
                placed = Some((Command::PlayerItemMove { from: pos.into(), to: eq_slot.into(), item_ident: ident }, "kept item, now worn"));
                break;
            }
            if let Some(i) = choose_socket(&candidates, gem, class) {
                let slot = list[i].0;
                report!(
                    "[gems] Socketing {:?} {} into {slot:?} (now: {})",
                    gem.typ,
                    gem.value,
                    candidates[i].content.map_or("empty".to_string(), |g| format!("{:?} {}", g.typ, g.value))
                );
                placed = Some((Command::PlayerItemMove { from: pos.into(), to: slot.into(), item_ident: ident }, "worn item"));
                break;
            }
        }
        let (cmd, what) = placed?; // None = no gem has a place, keep them
        if let Err(e) = safe::send(session, cmd).await {
            report!("[gems] Socketing into the {what} failed: {e}");
            if let Ok(mut d) = SOCKET_FAILED_ON.lock() {
                *d = Some(today);
            }
            return Some(fail(&e));
        }
        if let Err(e) = safe::send(session, Command::Update).await {
            return Some(fail(&e));
        }
        // Success = one gem fewer in the backpack (other loose gems being there says nothing: with four of them the old
        // check blocked all further socketing for the day after the first success)
        if session.game_state().is_none_or(|gs| backpack_gems(gs).len() >= before) {
            report!("[gems] The backpack still holds {before} gem(s) after the move, no more socketing today (request shape unverified)");
            if let Ok(mut d) = SOCKET_FAILED_ON.lock() {
                *d = Some(today);
            }
            return None;
        }
        safe::human_pause().await;
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
        Socket { content: content.map(gem), epic, value, worth: true, weapon: false }
    }

    /// 2026-10-17 (trainer #4): a main gem fills a socket with a secondary gem before it replaces a Luck gem, even on a better item.
    #[test]
    fn free_sockets_before_replacing_a_luck_gem() {
        let sockets = [sock(Some(GemType::Luck), true, 90.0), sock(Some(GemType::Dexterity), false, 50.0)];
        assert_eq!(choose_socket(&sockets, gem(GemType::Strength), Class::Warrior), Some(1));
        // without a free one the Luck gem is replaced
        let sockets = [sock(Some(GemType::Luck), true, 90.0), sock(Some(GemType::Constitution), false, 50.0)];
        assert_eq!(choose_socket(&sockets, gem(GemType::Strength), Class::Warrior), Some(0));
    }

    #[test]
    fn side_stat_only_items_get_no_gem() {
        let junk = Socket { worth: false, ..sock(None, true, 90.0) };
        assert_eq!(choose_socket(&[junk], gem(GemType::Intelligence), Class::Bard), None);
        assert_eq!(choose_socket(&[junk, sock(None, false, 1.0)], gem(GemType::Intelligence), Class::Bard), Some(1));
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
        assert_eq!(choose_socket(&[sock(Some(GemType::Strength), false, 1.0)], gem(GemType::Luck), c), Some(0)); // an old secondary gem still gets replaced
        assert_eq!(choose_socket(&[sock(Some(GemType::Luck), false, 1.0)], gem(GemType::Strength), c), None);
        // a secondary gem into nothing but a Luck socket: kept in the backpack
        assert_eq!(choose_socket(&[sock(Some(GemType::Luck), false, 1.0)], gem(GemType::Dexterity), c), None);
        // no sockets at all
        assert_eq!(choose_socket(&[], gem(GemType::Intelligence), c), None);
    }

    #[test]
    fn weapon_gets_the_strongest_gem() {
        let c = Class::Bard;
        let weapon = |content: Option<Gem>| Socket { content, epic: false, value: 1.0, worth: true, weapon: true };
        let strong_epic = sock(None, true, 90.0);
        // empty weapon socket beats a better item for main and Constitution gems
        assert_eq!(choose_socket(&[strong_epic, weapon(None)], gem(GemType::Intelligence), c), Some(1));
        assert_eq!(choose_socket(&[strong_epic, weapon(None)], gem(GemType::Constitution), c), Some(1));
        // Constitution in the weapon is replaced by a main gem; a stronger main gem replaces a weaker one; never the other way
        let con = Some(Gem { typ: GemType::Constitution, value: 99 });
        let weak_main = Some(Gem { typ: GemType::Intelligence, value: 30 });
        assert_eq!(choose_socket(&[weapon(con)], gem(GemType::Intelligence), c), Some(0));
        assert_eq!(choose_socket(&[weapon(weak_main)], gem(GemType::Intelligence), c), Some(0));
        assert_eq!(choose_socket(&[weapon(Some(gem(GemType::Intelligence)))], Gem { typ: GemType::Intelligence, value: 30 }, c), None);
        assert_eq!(choose_socket(&[weapon(Some(gem(GemType::Intelligence)))], gem(GemType::Constitution), c), None);
        // a weapon not worth a gem is skipped
        let junk = Socket { worth: false, ..weapon(None) };
        assert_eq!(choose_socket(&[junk], gem(GemType::Intelligence), c), None);
    }

    #[test]
    fn weak_gems_go_to_the_item_replaced_soonest() {
        let c = Class::Bard;
        // two empty sockets: a weak item (value 10) and a strong epic (value 90)
        let list = [sock(None, false, 10.0), sock(None, true, 90.0)];
        // a Luck or secondary gem goes to the weak item, Constitution and main-attribute gems to the strong one
        assert_eq!(choose_socket(&list, gem(GemType::Luck), c), Some(0));
        assert_eq!(choose_socket(&list, gem(GemType::Strength), c), None); // secondary: sold, never socketed
        assert_eq!(choose_socket(&list, gem(GemType::Constitution), c), Some(1));
        assert_eq!(choose_socket(&list, gem(GemType::Intelligence), c), Some(1));
    }
}
