//! Potions (user 2026-10-07). At most 3 active potions. Target set: main attribute + Constitution + Eternal Life
//! (+25 % HP, 7 days); without an Eternal Life potion the third one is Luck (unless Luck is at the crit cap, then the bigger secondary attribute). Stat potions last 3 days, sizes 10/15/25 %. Importance = strength gained by the 100/80/40/10 formula (size × attribute without potions × weight), see `importance`.
//! Drinking the same type again extends it by its full duration. Only for gold (Eternal Life usually costs mushrooms).
//! - A target potion is missing and a slot is free → drink one from the backpack, otherwise buy the biggest for gold.
//! - Stock: keep up to `MAX_STOCK` potions of any type in the backpack (better some than none, they are cheap, buying
//!   them spins the shop and they can be sold any time). Target types first; non-target ones are the first to go.
//! - Backpack full:
//!   - an active potion that is not a target, or a smaller stat one of a target type, is removed and a better target
//!     potion from the backpack drunk instead (user 2026-10-09: drinking replaces the active potion's remaining time
//!     rather than adding to it, but waiting for it to run low doesn't preserve more value – ongoing small-potion
//!     refills would just keep delaying the swap forever – so once the backpack is full and a strictly better potion
//!     is waiting, swap immediately regardless of days left on the active one),
//!   - otherwise the least important potion in the backpack is drunk (when it stacks onto an active one); a useful one (main/CON/Luck) swaps in for a less important active potion instead of being sold; else it is sold.
//!   - A bigger potion for a smaller active one is bought and swapped in only with a full backpack, with room it
//!     goes to the stock.
//! - Eternal Life is never removed or sold.
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

/// The 3 potion types we want active, in priority order: main attribute, CON, then Eternal Life; without it Luck – unless
/// Luck is at the crit cap (user 2026-10-09), then the bigger of the two secondary attributes.
pub fn targets(gs: &GameState) -> [PotionType; 3] {
    use sf_api::command::AttributeType as A;
    let life = active(gs).iter().any(|(_, p)| p.typ == PotionType::EternalLife)
        || in_bag(gs, PotionType::EternalLife).is_some()
        || in_shop(gs, PotionType::EternalLife).is_some();
    let main_attr = gs.character.class.main_attribute();
    let main = PotionType::from(main_attr);
    let third = if life {
        PotionType::EternalLife
    } else if !luck_capped(gs) {
        PotionType::Luck
    } else {
        let (pre, _, _) = bare_attributes(gs);
        let side = [A::Strength, A::Dexterity, A::Intelligence].into_iter().filter(|a| *a != main_attr).max_by_key(|a| pre[*a]);
        side.map_or(PotionType::Luck, PotionType::from)
    };
    [main, PotionType::Constitution, third]
}

/// How much a potion is worth to us: the strength it adds by the user's formula (main 100 %, CON 80 %, Luck 40 %, the two
/// other attributes 10 %; user 2026-10-09) = potion size × the attribute without potions (base + equipment, which is
/// what the game's potion bonus is a percentage of) × that weight. So a small main-attribute potion (10 % × 100) and a big
/// Luck one (25 % × 40) compare by what the character really has. Luck stops counting at the crit cap (50 % crit =
/// Luck ≥ 20 × level; only the part below the cap is worth anything). Eternal Life is always the most valuable.
fn importance(gs: &GameState, p: &Potion) -> f64 {
    use sf_api::command::AttributeType;
    if p.typ == PotionType::EternalLife {
        return 1e6;
    }
    let (pre, class, level) = bare_attributes(gs);
    let Some(attr) = [
        AttributeType::Strength,
        AttributeType::Dexterity,
        AttributeType::Intelligence,
        AttributeType::Constitution,
        AttributeType::Luck,
    ]
    .into_iter()
    .find(|a| PotionType::from(*a) == p.typ) else {
        return 0.0;
    };
    let mut gain = f64::from(pre[attr]) * p.size.effect();
    if attr == AttributeType::Luck {
        gain = gain.min((20.0 * f64::from(level) - f64::from(pre[attr])).max(0.0));
    }
    gain * crate::arena::weight(class, attr)
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

/// Next thing to drink from the backpack: a missing target potion while a slot is free. Falls back to the best
/// non-target potion sitting in the backpack (user 2026-10-08: even a secondary attribute helps a bit in a fight,
/// better active than rotting in the backpack or eventually sold) – it gets swapped out later once a target
/// potion turns up (`removal_ok` already allows removing a non-target active one).
fn bag_step(gs: &GameState) -> Option<(BagPosition, &Item)> {
    let act = active(gs);
    if act.len() >= 3 {
        return None;
    }
    if let Some(step) = targets(gs).into_iter().filter(|t| !act.iter().any(|(_, p)| p.typ == *t)).find_map(|t| in_bag(gs, t)) {
        return Some(step);
    }
    bag_potions(gs)
        .into_iter()
        .filter(|(_, _, p)| !act.iter().any(|(_, a)| a.typ == p.typ))
        .max_by(|a, b| importance(gs, a.2).total_cmp(&importance(gs, b.2)))
        .map(|(pos, i, _)| (pos, i))
}

/// May this active potion be removed? Never Eternal Life; any other active potion can be swapped out for a
/// better one (days left on it don't matter, see module docs).
pub fn removal_ok(gs: &GameState, slot: usize) -> bool {
    active(gs).iter().any(|(i, p)| *i == slot && p.typ != PotionType::EternalLife)
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
    let bag_full = gs.character.inventory.free_slot().is_none();
    for t in targets(gs) {
        let current = act.iter().find(|(_, p)| p.typ == t);
        let Some((pos, item)) = in_shop(gs, t) else { continue };
        let Some(new) = potion(item) else { continue };
        match current {
            None if act.len() < 3 && in_bag(gs, t).is_none() => {
                return Some((ShopStep::Drink(pos), format!("buying and drinking {}", describe(item))));
            }
            // Swap in a bigger one only with a full backpack (user 2026-10-07); days left on the active one don't
            // matter (user 2026-10-09, see module docs); otherwise it goes to the stock
            Some((slot, cur)) if t != PotionType::EternalLife && bag_full && new.size.effect() > cur.size.effect() => {
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

/// A target potion waits in the backpack (its type not active) while a less important removable one is active:
/// remove that one and drink the target. `only_secondary`: the removed one must be a non-target (secondary stat)
/// potion – that is the swap done at once, without waiting for a full backpack (user 2026-10-09).
fn target_swap(gs: &GameState, only_secondary: bool) -> Option<(RoomStep, String)> {
    let act = active(gs);
    let tg = targets(gs);
    for t in tg {
        if act.iter().any(|(_, p)| p.typ == t) {
            continue;
        }
        let Some((pos, item)) = in_bag(gs, t) else { continue };
        let Some(new) = potion(item) else { continue };
        let removable = act
            .iter()
            .filter(|(slot, p)| {
                removal_ok(gs, *slot)
                    && importance(gs, p) < importance(gs, new)
                    && (!only_secondary || !tg.contains(&p.typ))
            })
            .min_by(|a, b| importance(gs, a.1).total_cmp(&importance(gs, b.1)));
        if let Some((slot, old)) = removable {
            let why = if only_secondary { "better target potion in the backpack" } else { "backpack full" };
            return Some((RoomStep::Swap(*slot, pos), format!("{why}: removing the active {:?}, drinking {}", old.typ, describe(item))));
        }
    }
    None
}

fn room_step(gs: &GameState) -> Option<(RoomStep, String)> {
    let act = active(gs);
    // 1. A better target potion waits in the backpack while a removable (non-target, or smaller same-type) one is active
    if let Some(step) = target_swap(gs, false) {
        return Some(step);
    }
    // 2. The least important potion in the backpack: drink it if it stacks; a useful one (main/CON/Luck) rather replaces a
    // less useful active potion than gets sold (user 2026-10-09); anything else is sold
    let (pos, item, p) =
        bag_potions(gs).into_iter().min_by(|a, b| importance(gs, a.2).total_cmp(&importance(gs, b.2)))?;
    if act.iter().any(|(_, a)| a.typ == p.typ && p.size.effect() >= a.size.effect()) {
        return Some((RoomStep::Drink(pos), format!("backpack full: drinking {} (replaces the active one)", describe(item))));
    }
    // A bigger active one of the same type makes this one redundant (sold below); otherwise look for a worse active potion
    if p.typ != PotionType::EternalLife
        && importance(gs, p) > 0.0
        && !act.iter().any(|(_, a)| a.typ == p.typ)
        && let Some((slot, old)) = act
            .iter()
            .filter(|(slot, a)| removal_ok(gs, *slot) && importance(gs, a) < importance(gs, p))
            .min_by(|a, b| importance(gs, a.1).total_cmp(&importance(gs, b.1)))
    {
        return Some((
            RoomStep::Swap(*slot, pos),
            format!("backpack full: removing the active {:?}, drinking {} instead of selling it", old.typ, describe(item)),
        ));
    }
    if p.typ != PotionType::EternalLife {
        Some((RoomStep::Sell(pos), format!("backpack full: selling {}", describe(item))))
    } else {
        None
    }
}

/// Frees one backpack slot using a potion, if possible. Returns true when a slot was freed.
pub async fn make_room(session: &mut SimpleSession) -> Result<bool, Outcome> {
    let Some(gs) = session.game_state() else { return Ok(false) };
    let Some((step, what)) = room_step(gs) else { return Ok(false) };
    execute(session, step, &what).await
}

/// Replaces a drunk secondary-stat potion with a better target potion from the backpack at once, no full backpack
/// needed (user 2026-10-09). Returns true when a swap was made.
async fn swap_secondary(session: &mut SimpleSession) -> Result<bool, Outcome> {
    let Some(gs) = session.game_state() else { return Ok(false) };
    let Some((step, what)) = target_swap(gs, true) else { return Ok(false) };
    execute(session, step, &what).await
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
    if let Err(Outcome::SessionLost) = swap_secondary(session).await {
        return Outcome::SessionLost;
    }
    if session.game_state().is_some_and(|gs| gs.character.inventory.free_slot().is_none())
        && let Err(Outcome::SessionLost) = make_room(session).await
    {
        return Outcome::SessionLost;
    }
    Outcome::Done
}
