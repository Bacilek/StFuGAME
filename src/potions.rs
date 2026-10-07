//! Potions (user 2026-10-07). At most 3 active potions. Target set: main attribute + Constitution + Eternal Life
//! (+25 % HP, 7 days); without an Eternal Life potion the third one is Luck. Stat potions last 3 days, sizes 10/15/25 %.
//! Drinking the same type again extends it by its full duration. Only for gold (Eternal Life usually costs mushrooms).
//! - A target potion is missing and a slot is free → drink one from the backpack, otherwise buy the biggest for gold.
//! - Stock: keep up to `MAX_STOCK` potions of any type in the backpack (better some than none, they are cheap, buying
//!   them spins the shop and they can be sold any time). Target types first; non-target ones are the first to go.
//! - Backpack full:
//!   - an active potion that is not a target (or a smaller stat one with ≤ 3 days left) is removed and a better target
//!     potion from the backpack drunk instead,
//!   - otherwise the least important potion in the backpack is drunk (when it stacks onto an active one) or sold.
//!   - A bigger potion for a smaller active one (≤ 3 days left) is bought and swapped in only with a full backpack,
//!     with room it goes to the stock.
//! - Eternal Life is never removed or sold.
//!
//! Shop purchases run in `shops.rs` after equipment upgrades and before spinning (they may go below the reserve).

use chrono::{Duration, Local};
use sf_api::{
    command::Command,
    gamestate::{
        GameState, ShopPosition,
        items::{BagPosition, Item, ItemType, Potion, PotionType},
    },
};

use crate::{safe, session::SimpleSession, tavern::Outcome};

/// A smaller active potion is replaced only when it has at most this much left (one potion = 3 days).
const REPLACE_WITHIN: Duration = Duration::days(3);
/// How many potions to keep in the backpack.
pub const MAX_STOCK: usize = 4;

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

/// The 3 potion types we want active, in priority order.
pub fn targets(gs: &GameState) -> [PotionType; 3] {
    let life = active(gs).iter().any(|(_, p)| p.typ == PotionType::EternalLife)
        || in_bag(gs, PotionType::EternalLife).is_some()
        || in_shop(gs, PotionType::EternalLife).is_some();
    let main = PotionType::from(gs.character.class.main_attribute());
    [main, PotionType::Constitution, if life { PotionType::EternalLife } else { PotionType::Luck }]
}

/// How important a potion is for us: target rank (main 3, CON 2, third 1, other 0) + size; Eternal Life highest.
fn importance(gs: &GameState, p: &Potion) -> f64 {
    if p.typ == PotionType::EternalLife {
        return 10.0;
    }
    let rank = targets(gs).iter().position(|t| *t == p.typ).map_or(0.0, |i| 3.0 - i as f64);
    rank + p.size.effect()
}

fn describe(item: &Item) -> String {
    potion(item).map_or_else(|| format!("{:?}", item.typ), |p| format!("{:?} {:.0} %", p.typ, p.size.effect() * 100.0))
}

/// Potions in the backpack (the stock).
pub fn stock(gs: &GameState) -> usize {
    bag_potions(gs).len()
}

/// Can a potion be bought just to spin the shop? (stock not full)
pub fn stock_has_room(gs: &GameState) -> bool {
    stock(gs) < MAX_STOCK
}

/// Next thing to drink from the backpack: a missing target potion while a slot is free.
fn bag_step(gs: &GameState) -> Option<(BagPosition, &Item)> {
    let act = active(gs);
    if act.len() >= 3 {
        return None;
    }
    targets(gs).into_iter().filter(|t| !act.iter().any(|(_, p)| p.typ == *t)).find_map(|t| in_bag(gs, t))
}

/// May this active potion be removed? Never Eternal Life. A non-target one, or a stat one with ≤ 3 days left.
pub fn removal_ok(gs: &GameState, slot: usize) -> bool {
    let now = Local::now();
    let t = targets(gs);
    active(gs).iter().any(|(i, p)| {
        *i == slot
            && p.typ != PotionType::EternalLife
            && (!t.contains(&p.typ) || p.expires.is_some_and(|e| e - now <= REPLACE_WITHIN))
    })
}

/// What to buy in the shops now.
#[derive(Debug)]
pub enum ShopStep {
    /// Buy and drink (a free slot for a missing target potion)
    Drink(ShopPosition),
    /// Remove the active potion in this slot, buy and drink the bigger one
    Replace(usize, ShopPosition),
    /// Buy and keep in the backpack (stock)
    Keep(ShopPosition),
}

pub fn shop_step(gs: &GameState) -> Option<(ShopStep, String)> {
    let act = active(gs);
    let now = Local::now();
    let bag_full = gs.character.inventory.free_slot().is_none();
    for t in targets(gs) {
        let current = act.iter().find(|(_, p)| p.typ == t);
        let Some((pos, item)) = in_shop(gs, t) else { continue };
        let Some(new) = potion(item) else { continue };
        match current {
            None if act.len() < 3 && in_bag(gs, t).is_none() => {
                return Some((ShopStep::Drink(pos), format!("buying and drinking {}", describe(item))));
            }
            // Swap in a bigger one only with a full backpack (user 2026-10-07); otherwise it goes to the stock
            Some((slot, cur))
                if t != PotionType::EternalLife
                    && bag_full
                    && new.size.effect() > cur.size.effect()
                    && cur.expires.is_some_and(|e| e - now <= REPLACE_WITHIN) =>
            {
                return Some((
                    ShopStep::Replace(*slot, pos),
                    format!("replacing {:?} {:.0} % with {}", cur.typ, cur.size.effect() * 100.0, describe(item)),
                ));
            }
            _ => {}
        }
    }
    // Stock: the most important potion available for gold, if the stock has room or it beats the least important one
    let (pos, item, p) =
        shop_potions(gs).into_iter().max_by(|a, b| importance(gs, a.2).total_cmp(&importance(gs, b.2)))?;
    let new = importance(gs, p);
    let worst = bag_potions(gs).into_iter().map(|(_, _, p)| importance(gs, p)).min_by(f64::total_cmp);
    if stock_has_room(gs) || worst.is_some_and(|w| new > w) {
        return Some((ShopStep::Keep(pos), format!("buying {} for the stock ({} in the backpack)", describe(item), stock(gs))));
    }
    None
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[potions] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Drinks missing target potions from the backpack. Sends nothing when there is nothing to do.
pub async fn drink_from_bag(session: &mut SimpleSession) -> Outcome {
    for _ in 0..3 {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let Some((pos, item)) = bag_step(gs) else { return Outcome::Done };
        report!("[potions] Drinking {} from the backpack", describe(item));
        let cmd = Command::UsePotion { from: pos.into(), item_ident: item.command_ident() };
        if let Err(e) = safe::send(session, cmd).await {
            return fail(&e);
        }
    }
    Outcome::Done
}

/// Removes an active potion (before a better one is drunk).
pub async fn remove(session: &mut SimpleSession, slot: usize) -> Outcome {
    match safe::send(session, Command::RemovePotion { pos: slot }).await {
        Ok(_) => Outcome::Done,
        Err(e) => fail(&e),
    }
}

/// How to free one backpack slot with a potion.
enum RoomStep {
    /// Remove the active potion in this slot and drink this better target potion from the backpack
    Swap(usize, BagPosition),
    /// Drink it (stacks onto an active one of the same type)
    Drink(BagPosition),
    Sell(BagPosition),
}

fn room_step(gs: &GameState) -> Option<(RoomStep, String)> {
    let act = active(gs);
    // 1. A better target potion waits in the backpack while a removable (non-target / weak expiring) one is active
    for t in targets(gs) {
        if act.iter().any(|(_, p)| p.typ == t) {
            continue;
        }
        let Some((pos, item)) = in_bag(gs, t) else { continue };
        let Some(new) = potion(item) else { continue };
        let removable = act
            .iter()
            .filter(|(slot, p)| removal_ok(gs, *slot) && importance(gs, p) < importance(gs, new))
            .min_by(|a, b| importance(gs, a.1).total_cmp(&importance(gs, b.1)));
        if let Some((slot, old)) = removable {
            return Some((
                RoomStep::Swap(*slot, pos),
                format!("backpack full: removing the active {:?}, drinking {}", old.typ, describe(item)),
            ));
        }
    }
    // 2. The least important potion in the backpack: drink it if it stacks, otherwise sell it
    let (pos, item, p) =
        bag_potions(gs).into_iter().min_by(|a, b| importance(gs, a.2).total_cmp(&importance(gs, b.2)))?;
    if act.iter().any(|(_, a)| a.typ == p.typ && p.size.effect() >= a.size.effect()) {
        Some((RoomStep::Drink(pos), format!("backpack full: drinking {} (extends the active one)", describe(item))))
    } else if p.typ != PotionType::EternalLife {
        Some((RoomStep::Sell(pos), format!("backpack full: selling {}", describe(item))))
    } else {
        None
    }
}

/// Frees one backpack slot using a potion, if possible. Returns true when a slot was freed.
pub async fn make_room(session: &mut SimpleSession) -> Result<bool, Outcome> {
    let Some(gs) = session.game_state() else { return Ok(false) };
    let Some((step, what)) = room_step(gs) else { return Ok(false) };
    let pos = match step {
        RoomStep::Swap(_, pos) | RoomStep::Drink(pos) | RoomStep::Sell(pos) => pos,
    };
    let Some(item) = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()) else {
        return Ok(false);
    };
    let item_ident = item.command_ident();
    report!("[potions] {what}");
    if let RoomStep::Swap(slot, _) = step
        && let Outcome::SessionLost = remove(session, slot).await
    {
        return Err(Outcome::SessionLost);
    }
    let cmd = match step {
        RoomStep::Swap(_, pos) | RoomStep::Drink(pos) => Command::UsePotion { from: pos.into(), item_ident },
        RoomStep::Sell(pos) => Command::SellShop { item_pos: pos.into(), item_ident },
    };
    match safe::send(session, cmd).await {
        Ok(_) => Ok(true),
        Err(e) => Err(fail(&e)),
    }
}

/// Keeps the stock at `MAX_STOCK`: extra potions go from the least important (drink if it stacks, else sell).
pub async fn trim_stock(session: &mut SimpleSession) -> Outcome {
    for _ in 0..3 {
        if session.game_state().is_none_or(|gs| stock(gs) <= MAX_STOCK) {
            return Outcome::Done;
        }
        match make_room(session).await {
            Ok(true) => {}
            Ok(false) => return Outcome::Done,
            Err(o) => return o,
        }
    }
    Outcome::Done
}

/// Every pass of the main loop: drink missing target potions; with a full backpack free one slot via a potion.
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
