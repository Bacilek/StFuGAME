//! Potions (user 2026-10-07). At most 3 active potions. Target set: main attribute + Constitution + Eternal Life
//! (+25 % HP, 7 days); without an Eternal Life potion the third one is Luck. Stat potions last 3 days, sizes 10/15/25 %.
//! Only for gold (Eternal Life is rare and usually costs mushrooms → only when it is for gold).
//! - A target potion is missing and a slot is free → drink one from the backpack, otherwise buy the biggest for gold.
//! - A smaller one of the same type is active with at most 3 days left, a bigger one is for gold and the backpack is
//!   full → replace it (a long stacked one is kept). With room in the backpack the bigger one goes to the stock.
//! - Eternal Life for gold while all slots are full → buy it and keep it in the backpack until a slot frees up.
//! - Stock (user 2026-10-07): keep up to `MAX_STOCK` target potions in the backpack, buy them whenever they are for gold.
//! - Backpack full → make room from the least important potion: drink it when that type is active (it stacks,
//!   if not smaller than the active one), otherwise sell it. Eternal Life is never sold.
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
/// How many spare target potions to keep in the backpack.
const MAX_STOCK: usize = 4;

fn potion(item: &Item) -> Option<&Potion> {
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

fn in_bag(gs: &GameState, typ: PotionType) -> Option<(BagPosition, &Item)> {
    gs.character
        .inventory
        .iter()
        .filter_map(|(pos, i)| i.map(|i| (pos, i)))
        .filter(|(_, i)| potion(i).is_some_and(|p| p.typ == typ))
        .max_by(|a, b| {
            let size = |i: &Item| potion(i).map_or(0.0, |p| p.size.effect());
            size(a.1).total_cmp(&size(b.1))
        })
}

/// The biggest affordable gold-only potion of this type in the shops.
fn in_shop(gs: &GameState, typ: PotionType) -> Option<(ShopPosition, &Item)> {
    gs.shops
        .values()
        .flat_map(|s| s.iter())
        .filter(|(_, i)| i.mushroom_price == 0 && i.price > 0 && u64::from(i.price) <= gs.character.silver)
        .filter(|(_, i)| potion(i).is_some_and(|p| p.typ == typ))
        .max_by(|a, b| {
            let size = |i: &Item| potion(i).map_or(0.0, |p| p.size.effect());
            size(a.1).total_cmp(&size(b.1))
        })
}

/// The 3 potion types we want active, in priority order.
pub fn targets(gs: &GameState) -> [PotionType; 3] {
    let life = active(gs).iter().any(|(_, p)| p.typ == PotionType::EternalLife)
        || in_bag(gs, PotionType::EternalLife).is_some()
        || in_shop(gs, PotionType::EternalLife).is_some();
    let main = PotionType::from(gs.character.class.main_attribute());
    [main, PotionType::Constitution, if life { PotionType::EternalLife } else { PotionType::Luck }]
}

fn describe(item: &Item) -> String {
    potion(item).map_or_else(|| format!("{:?}", item.typ), |p| format!("{:?} {:.0} %", p.typ, p.size.effect() * 100.0))
}

/// Next thing to drink from the backpack: a missing target potion while a slot is free.
fn bag_step(gs: &GameState) -> Option<(BagPosition, &Item)> {
    let act = active(gs);
    if act.len() >= 3 {
        return None;
    }
    targets(gs)
        .into_iter()
        .filter(|t| !act.iter().any(|(_, p)| p.typ == *t))
        .find_map(|t| in_bag(gs, t))
}

/// What to buy in the shops now.
#[derive(Debug)]
pub enum ShopStep {
    /// Buy and drink (a free slot for a missing target potion)
    Drink(ShopPosition),
    /// Remove the active potion in this slot, buy and drink the bigger one
    Replace(usize, ShopPosition),
    /// Buy and keep in the backpack (stock, Eternal Life)
    Keep(ShopPosition),
}

/// Spare target potions in the backpack.
fn stock(gs: &GameState) -> usize {
    let t = targets(gs);
    gs.character.inventory.backpack.iter().flatten().filter(|i| potion(i).is_some_and(|p| t.contains(&p.typ))).count()
}

pub fn shop_step(gs: &GameState) -> Option<(ShopStep, String)> {
    let act = active(gs);
    let now = Local::now();
    for t in targets(gs) {
        let current = act.iter().find(|(_, p)| p.typ == t);
        let Some((pos, item)) = in_shop(gs, t) else { continue };
        let Some(new) = potion(item) else { continue };
        match current {
            None if act.len() < 3 && in_bag(gs, t).is_none() => {
                return Some((ShopStep::Drink(pos), format!("buying and drinking {}", describe(item))));
            }
            // Replace only with a full backpack (user 2026-10-07); otherwise the bigger one goes to the stock
            Some((slot, cur))
                if t != PotionType::EternalLife
                    && gs.character.inventory.free_slot().is_none()
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
    // Eternal Life for gold is rare: keep one in the backpack when it cannot be drunk now
    let life_active = act.iter().any(|(_, p)| p.typ == PotionType::EternalLife);
    if !life_active
        && in_bag(gs, PotionType::EternalLife).is_none()
        && let Some((pos, item)) = in_shop(gs, PotionType::EternalLife)
    {
        return Some((ShopStep::Keep(pos), format!("buying {} to keep until a slot is free", describe(item))));
    }
    // Stock: the most important target type available for gold, biggest size
    if stock(gs) < MAX_STOCK
        && let Some((pos, item)) = targets(gs).into_iter().find_map(|t| in_shop(gs, t))
    {
        return Some((ShopStep::Keep(pos), format!("buying {} for the stock ({} in the backpack)", describe(item), stock(gs))));
    }
    None
}

/// How important a potion is for us (0 = not a target at all). Smaller sizes are less important.
fn importance(gs: &GameState, p: &Potion) -> f64 {
    let rank = match targets(gs).iter().position(|t| *t == p.typ) {
        Some(i) => 3.0 - i as f64, // main 3, CON 2, third 1
        None => 0.0,
    };
    let life = if p.typ == PotionType::EternalLife { 10.0 } else { 0.0 };
    rank + life + p.size.effect()
}

/// Making room in a full backpack: the least important potion is drunk (its type is active and it is not smaller,
/// so it extends the duration) or sold. Eternal Life is never sold.
pub enum RoomStep {
    Drink(BagPosition),
    Sell(BagPosition),
}

pub fn room_step(gs: &GameState) -> Option<(RoomStep, String)> {
    let act = active(gs);
    let (pos, item, p) = gs
        .character
        .inventory
        .iter()
        .filter_map(|(pos, i)| i.and_then(|i| potion(i).map(|p| (pos, i, p))))
        .min_by(|a, b| importance(gs, a.2).total_cmp(&importance(gs, b.2)))?;
    let stacks = act.iter().any(|(_, a)| a.typ == p.typ && p.size.effect() >= a.size.effect());
    if stacks {
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
    let (RoomStep::Drink(pos) | RoomStep::Sell(pos)) = step;
    let Some(item) = gs.character.inventory.backpack.get(pos.backpack_pos()).and_then(|i| i.as_ref()) else {
        return Ok(false);
    };
    let item_ident = item.command_ident();
    report!("[potions] {what}");
    let cmd = match step {
        RoomStep::Drink(pos) => Command::UsePotion { from: pos.into(), item_ident },
        RoomStep::Sell(pos) => Command::SellShop { item_pos: pos.into(), item_ident },
    };
    match safe::send(session, cmd).await {
        Ok(_) => Ok(true),
        Err(e) => Err(fail(&e)),
    }
}

/// May this active potion be removed? Only a smaller stat potion with at most 3 days left (see `shop_step`).
pub fn removal_ok(gs: &GameState, slot: usize) -> bool {
    let now = Local::now();
    active(gs)
        .iter()
        .any(|(i, p)| *i == slot && p.typ != PotionType::EternalLife && p.expires.is_some_and(|e| e - now <= REPLACE_WITHIN))
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

/// Removes an active potion before a bigger one is drunk (called from `shops.rs`).
pub async fn remove(session: &mut SimpleSession, slot: usize) -> Outcome {
    match safe::send(session, Command::RemovePotion { pos: slot }).await {
        Ok(_) => Outcome::Done,
        Err(e) => fail(&e),
    }
}
