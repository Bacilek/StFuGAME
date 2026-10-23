//! Potions (user 2026-10-07, reworked 2026-10-17). At most 3 active potions. Only four kinds are worth anything, in this
//! order of importance (user 2026-10-17): main attribute > Eternal Life > Constitution > Luck (Luck not at the crit cap; the
//! top three that are available get the three slots, so Luck only gets one when there is no Eternal Life). Within a kind the
//! biggest one wins (10/15/25 %, 3 days; Eternal Life +25 % HP, 7 days). Only for gold.
//! - No stock: a potion never waits in the backpack (backpack space is for gems first). Whatever lands there is settled at once
//!   (`bag_action`): a secondary-attribute potion (or one outranked by all three active ones) is SOLD; a wanted kind is drunk
//!   when its kind is not active (free slot, or it replaces a less important active potion) or when it is at least as big as
//!   the active one of its kind (same size extends by the full duration, a bigger one replaces it, user 2026-10-09/14); a
//!   smaller one than the active one is sold. Eternal Life is never sold or removed.
//! - Shops (`shop_step`): buy only what is drunk right away: a wanted kind that is not active (free slot or less important
//!   active potion to replace) or a bigger one than the active one of its kind. Secondary potions are never bought.
//!
//! Shop purchases run in `shops.rs` after equipment upgrades and before spinning (they may go below the reserve).

use chrono::Local;
use sf_api::{
    command::Command,
    gamestate::{
        GameState, ShopPosition,
        items::{BagPosition, Item, ItemType, Potion, PotionType},
    },
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

pub fn potion(item: &Item) -> Option<&Potion> {
    match &item.typ {
        ItemType::Potion(p) => Some(p),
        _ => None,
    }
}

/// Active (not expired) potions with their slot.
fn active(gs: &GameState) -> Vec<(usize, &Potion)> {
    let now = Local::now();
    gs.character
        .active_potions
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
        .filter(|(_, p)| p.expires.is_none_or(|e| e > now))
        .collect()
}

/// Potions in the backpack.
fn bag_potions(gs: &GameState) -> Vec<(BagPosition, &Item, &Potion)> {
    gs.character.inventory.iter().filter_map(|(pos, i)| i.and_then(|i| potion(i).map(|p| (pos, i, p)))).collect()
}

fn in_bag(gs: &GameState, typ: PotionType) -> Option<(BagPosition, &Item)> {
    bag_potions(gs)
        .into_iter()
        .filter(|(_, _, p)| p.typ == typ)
        .max_by(|a, b| a.2.size.effect().total_cmp(&b.2.size.effect()))
        .map(|(pos, i, _)| (pos, i))
}

/// Gold-only potions in the shops that we can afford.
fn shop_potions(gs: &GameState) -> Vec<(ShopPosition, &Item, &Potion)> {
    gs.shops
        .values()
        .flat_map(|s| s.iter())
        .filter(|(_, i)| i.mushroom_price == 0 && i.price > 0 && u64::from(i.price) <= gs.character.silver)
        .filter_map(|(pos, i)| potion(i).map(|p| (pos, i, p)))
        .collect()
}

/// The biggest affordable gold-only potion of this type in the shops.
fn in_shop(gs: &GameState, typ: PotionType) -> Option<(ShopPosition, &Item)> {
    shop_potions(gs)
        .into_iter()
        .filter(|(_, _, p)| p.typ == typ)
        .max_by(|a, b| a.2.size.effect().total_cmp(&b.2.size.effect()))
        .map(|(pos, i, _)| (pos, i))
}

/// Attributes without any potion (base + equipment, what the potion bonus is a percentage of), the class and level.
fn bare_attributes(gs: &GameState) -> (enum_map::EnumMap<sf_api::command::AttributeType, u32>, sf_api::gamestate::character::Class, u16) {
    let mut f = sf_api::simulate::PlayerFighterSquad::new(gs).character;
    f.active_potions = Default::default();
    (f.attributes(), f.class, f.level)
}

/// Luck is at the crit cap already (50 % crit = Luck ≥ 20 × level, see `derived()` in roster.rs): a Luck potion is useless.
fn luck_capped(gs: &GameState) -> bool {
    let (pre, _, level) = bare_attributes(gs);
    f64::from(pre[sf_api::command::AttributeType::Luck]) >= 20.0 * f64::from(level)
}

/// The kinds we want active, most important first (at most 3): main attribute, Eternal Life (when active / in the backpack /
/// for gold in a shop), Constitution, Luck (unless at the crit cap). User 2026-10-17: main > Eternal Life > CON > Luck.
pub fn targets(gs: &GameState) -> Vec<PotionType> {
    let life = active(gs).iter().any(|(_, p)| p.typ == PotionType::EternalLife)
        || in_bag(gs, PotionType::EternalLife).is_some()
        || in_shop(gs, PotionType::EternalLife).is_some();
    let mut t = vec![PotionType::from(gs.character.class.main_attribute())];
    if life {
        t.push(PotionType::EternalLife);
    }
    t.push(PotionType::Constitution);
    if !luck_capped(gs) {
        t.push(PotionType::Luck);
    }
    t.truncate(3);
    t
}

/// How much a potion is worth to us: its kind decides (main attribute > Eternal Life > CON > Luck), the size breaks ties
/// within a kind. 0 = not wanted (secondary attributes, a kind outranked by three better ones).
fn importance(gs: &GameState, p: &Potion) -> f64 {
    let tg = targets(gs);
    let Some(i) = tg.iter().position(|t| *t == p.typ) else { return 0.0 };
    (tg.len() - i) as f64 * 1000.0 + p.size.effect() * 100.0
}

fn describe(item: &Item) -> String {
    potion(item).map_or_else(|| format!("{:?}", item.typ), |p| format!("{:?} {:.0} %", p.typ, p.size.effect() * 100.0))
}

/// May this active potion be removed? Never Eternal Life; any other active potion can be swapped out for a
/// better one (days left on it don't matter, see module docs).
pub fn removal_ok(gs: &GameState, slot: usize) -> bool {
    active(gs).iter().any(|(i, p)| *i == slot && p.typ != PotionType::EternalLife)
}

/// The active potion to push out for a potion of importance `new`: the least important removable one, if it is worth less.
fn weakest_removable(gs: &GameState, new: f64) -> Option<(usize, &Potion)> {
    active(gs)
        .into_iter()
        .filter(|(slot, p)| removal_ok(gs, *slot) && importance(gs, p) < new)
        .min_by(|a, b| importance(gs, a.1).total_cmp(&importance(gs, b.1)))
}

/// What to buy in the shops now.
#[derive(Debug)]
pub enum ShopStep {
    /// Buy and drink (a free slot for a missing wanted potion)
    Drink(ShopPosition),
    /// Remove the active potion in this slot, buy and drink the new one
    Replace(usize, ShopPosition),
}

/// Buys only potions that are drunk right away: a wanted kind that is not active, or a bigger one than the active one.
pub fn shop_step(gs: &GameState) -> Option<(ShopStep, String)> {
    let act = active(gs);
    for t in targets(gs) {
        if in_bag(gs, t).is_some() {
            continue; // settled from the backpack first
        }
        let Some((pos, item)) = in_shop(gs, t) else { continue };
        let Some(new) = potion(item) else { continue };
        match act.iter().find(|(_, p)| p.typ == t) {
            None if act.len() < 3 => {
                return Some((ShopStep::Drink(pos), format!("buying and drinking {}", describe(item))));
            }
            None => {
                if let Some((slot, old)) = weakest_removable(gs, importance(gs, new)) {
                    return Some((ShopStep::Replace(slot, pos), format!("replacing {:?} with {}", old.typ, describe(item))));
                }
            }
            Some((slot, cur)) if t != PotionType::EternalLife && new.size.effect() > cur.size.effect() => {
                return Some((
                    ShopStep::Replace(*slot, pos),
                    format!("replacing {:?} {:.0} % with {}", cur.typ, cur.size.effect() * 100.0, describe(item)),
                ));
            }
            _ => {}
        }
    }
    None
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[potions] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Removes an active potion (before a better one is drunk).
pub async fn remove(session: &mut SimpleSession, slot: usize) -> Outcome {
    match safe::send(session, Command::RemovePotion { pos: slot }).await {
        Ok(_) => Outcome::Done,
        Err(e) => fail(&e),
    }
}

/// What to do with a potion lying in the backpack.
enum RoomStep {
    /// Remove the active potion in this slot and drink this better potion from the backpack
    Swap(usize, BagPosition),
    /// Drink it (extends or replaces the active one of its kind, or takes a free slot)
    Drink(BagPosition),
    Sell(BagPosition),
}

/// How short of backpack space we are (user 2026-10-17): a smaller potion of a wanted kind is KEPT while there is room, drunk when
/// space gets tight and the bigger active one has little time left, sold only when nothing else can be sold.
#[derive(Clone, Copy, PartialEq)]
enum Pressure {
    Room,
    Tight,
    Desperate,
}


/// Days an active potion still lasts.
fn days_left(p: &Potion) -> f64 {
    p.expires.map_or(999.0, |e| (e - Local::now()).num_minutes() as f64 / 1440.0)
}

/// A bigger active potion with less than this many days left is replaced by a smaller one of the same kind (user 2026-10-18).
const REPLACE_BELOW_DAYS: f64 = 1.0;

/// The next thing to do with the potions in the backpack (see the module docs); `None` = nothing to do.
fn bag_action(gs: &GameState, pressure: Pressure) -> Option<(RoomStep, String)> {
    let act = active(gs);
    let mut bag = bag_potions(gs);
    bag.sort_by(|a, b| importance(gs, b.2).total_cmp(&importance(gs, a.2)));
    for (pos, item, p) in bag {
        let sell = |why: &str| (p.typ != PotionType::EternalLife).then(|| (RoomStep::Sell(pos), format!("{why}: selling {}", describe(item))));
        if importance(gs, p) == 0.0 {
            if let Some(s) = sell("not a wanted potion") {
                return Some(s);
            }
            continue;
        }
        match act.iter().find(|(_, a)| a.typ == p.typ) {
            Some((_, a)) if p.size.effect() >= a.size.effect() => {
                return Some((RoomStep::Drink(pos), format!("drinking {} (extends/replaces the active one)", describe(item))));
            }
            // The server refuses a downgrade, so the bigger active potion is removed first, but only when it has less than a day left
            // (user 2026-10-18); otherwise the smaller one is kept until the active one expires, sold only as the very last thing
            Some((slot, a)) if days_left(a) < REPLACE_BELOW_DAYS => {
                return Some((
                    RoomStep::Swap(*slot, pos),
                    format!("the active {:?} has {:.1} days left: removing it and drinking {}", a.typ, days_left(a), describe(item)),
                ));
            }
            Some(_) => match pressure {
                Pressure::Room | Pressure::Tight => {}
                Pressure::Desperate => {
                    if let Some(s) = sell("smaller than the active one, nothing else to sell") {
                        return Some(s);
                    }
                }
            },
            None if act.len() < 3 => return Some((RoomStep::Drink(pos), format!("drinking {} from the backpack", describe(item)))),
            None => {
                if let Some((slot, old)) = weakest_removable(gs, importance(gs, p)) {
                    return Some((
                        RoomStep::Swap(slot, pos),
                        format!("removing the active {:?}, drinking {}", old.typ, describe(item)),
                    ));
                }
                if pressure == Pressure::Desperate
                    && let Some(s) = sell("outranked by the active potions, nothing else to sell")
                {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Frees one backpack slot: a loose gem goes first (socketed or sold, `gems::free_slot`), then a potion, then the item kept for a gem (swapped
/// for the worn one), then an epic. Returns true when a slot was freed.
pub async fn make_room(session: &mut SimpleSession) -> Result<bool, Outcome> {
    if crate::gems::free_slot(session).await? {
        return Ok(true);
    }
    let Some(gs) = session.game_state() else { return Ok(false) };
    let step = bag_action(gs, Pressure::Tight)
        .or_else(|| bag_action(gs, Pressure::Desperate));
    if let Some((step, what)) = step {
        return execute(session, step, &what).await;
    }
    if crate::inventory::swap_kept_item(session).await? {
        return Ok(true);
    }
    let Some(gs) = session.game_state() else { return Ok(false) };
    let Some((p, what)) = crate::inventory::last_resort_epic(gs) else { return Ok(false) };
    execute(session, RoomStep::Sell(p), &what).await
}

/// Room for the items a Hall of Fame hunt (bare hands, no chest plate, no epics) takes off (user 2026-10-19): a loose gem first
/// (socketed or sold, gems are cheap and mined without end), then only the gentle potion steps (`Pressure::Tight`: sell an UNWANTED
/// potion, drink or swap one). Never a wanted potion (Eternal Life, main attribute, Constitution) sold for the sake of a 1-point
/// task, never a kept item swapped and never an epic sold (`make_room`'s `Desperate` step and `last_resort_epic`).
pub async fn make_room_for_hunt(session: &mut SimpleSession) -> Result<bool, Outcome> {
    if crate::gems::free_slot(session).await? {
        return Ok(true);
    }
    let Some(gs) = session.game_state() else { return Ok(false) };
    match bag_action(gs, Pressure::Tight) {
        Some((step, what)) => execute(session, step, &what).await,
        None => Ok(false),
    }
}

async fn execute(session: &mut SimpleSession, step: RoomStep, what: &str) -> Result<bool, Outcome> {
    let Some(gs) = session.game_state() else { return Ok(false) };
    let pos = match step {
        RoomStep::Swap(_, pos) | RoomStep::Drink(pos) | RoomStep::Sell(pos) => pos,
    };
    let Some(item) = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()) else {
        return Ok(false);
    };
    let item_ident = item.command_ident();
    // An epic (only the forced free-slot sale gets here) is dismantled from level 90 when the Blacksmith can (user 2026-10-24)
    let mut cauldron = crate::witch::type_matches(gs, item);
    let dismantle = matches!(step, RoomStep::Sell(_)) && item.is_epic() && crate::smith::epic_disposal(gs, item) == crate::smith::Disposal::Dismantle;
    report!("[potions] {what}");
    if dismantle {
        let RoomStep::Sell(pos) = step else { return Ok(false) };
        match crate::smith::dismantle(session, pos, what).await {
            Ok(true) => return Ok(true),
            Ok(false) => {
                // refused: sell it (the witch gets her type at twice the price) as before
                cauldron = session.game_state().and_then(|gs| gs.character.inventory.backpack.get(pos.backpack_pos())?.as_ref().map(|i| crate::witch::type_matches(gs, i))).unwrap_or(false);
            }
            Err(e) => return Err(fail(&e)),
        }
    }
    if let RoomStep::Swap(slot, _) = step
        && let Outcome::SessionLost = remove(session, slot).await
    {
        return Err(Outcome::SessionLost);
    }
    let cmd = match step {
        RoomStep::Swap(_, pos) | RoomStep::Drink(pos) => Command::UsePotion { from: pos.into(), item_ident },
        // An epic of the witch's type goes into her cauldron for twice the price (user 2026-10-22)
        RoomStep::Sell(pos) if cauldron => Command::WitchDropCauldron { item_pos: pos.into() },
        RoomStep::Sell(pos) => Command::SellShop { item_pos: pos.into(), item_ident },
    };
    match safe::send(session, cmd).await {
        Ok(_) => Ok(true),
        Err(e) => Err(fail(&e)),
    }
}

/// Settles every potion in the backpack (drink or sell, see the module docs). Sends nothing when there is none.
pub async fn drink_from_bag(session: &mut SimpleSession) -> Outcome {
    for _ in 0..8 {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let free = gs.character.inventory.iter().filter(|(_, i)| i.is_none()).count();
        let Some((step, what)) = bag_action(gs, if free <= 1 { Pressure::Tight } else { Pressure::Room }) else { return Outcome::Done };
        match execute(session, step, &what).await {
            Ok(true) => {}
            Ok(false) => return Outcome::Done,
            Err(o) => return o,
        }
    }
    Outcome::Done
}

/// Same as `drink_from_bag`; called after a potion bought for spinning landed in the backpack.
pub async fn trim_stock(session: &mut SimpleSession) -> Outcome {
    drink_from_bag(session).await
}

/// Every pass of the main loop: settle the potions in the backpack; with a full backpack free one slot.
pub async fn run(session: &mut SimpleSession) -> Outcome {
    if let Outcome::SessionLost = drink_from_bag(session).await {
        return Outcome::SessionLost;
    }
    if session.game_state().is_some_and(|gs| gs.character.inventory.free_slot().is_none())
        && let Err(Outcome::SessionLost) = make_room(session).await
    {
        return Outcome::SessionLost;
    }
    Outcome::Done
}
