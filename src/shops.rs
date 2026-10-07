//! Weapon Shop and Magic Shop, once a day after the Tavern is done (the whole Thirst for Adventure used up).
//! Buys ONLY for gold (`mushroom_price == 0`), never for mushrooms, never `RefreshShop` (costs a mushroom).
//! 1. Upgrades: a gold item better than the equipped one (by `inventory::value`) → buy, `inventory::manage` equips it
//!    and sells the old one (never epic items). Upgrades may use all the gold.
//! 2. Spinning: buy the cheapest non-epic gold item and sell it right away (`inventory::manage`) to get a new offer.
//!    Only while the gold after the purchase stays at or above the reserve = the most expensive gold item for our class
//!    seen today, so an upgrade that shows up after a spin can always be bought. Ends when no gold items are left.


use chrono::{Local, NaiveDate};
use sf_api::{
    command::{Command, ShopType},
    gamestate::{
        GameState, ShopPosition,
        items::{Item, ItemType},
        rewards::TaskType,
    },
};

use crate::session::SimpleSession;

use crate::{inventory, safe, tavern::Outcome};

/// Safety limit on spins per day.
const MAX_SPINS: usize = 60;
/// Safety limit on purchases for tasks per day (in case the task counter does not update).
const MAX_TASK_BUYS: usize = 6;
/// Safety limit on potion purchases per day.
const MAX_POTION_BUYS: usize = 4;

/// Today's shopping: the reserve and whether shopping is done.
struct Day {
    date: NaiveDate,
    reserve: u32,
    done: bool,
}

static DAY: crate::ctx::PerChar<Option<Day>> = crate::ctx::PerChar::new();

/// Runs `f` on today's state (a new day starts with a zero reserve).
fn with_day<T>(f: impl FnOnce(&mut Day) -> T) -> Option<T> {
    let mut guard = DAY.lock().ok()?;
    let today = Local::now().date_naive();
    if guard.as_ref().is_none_or(|d| d.date != today) {
        *guard = Some(Day { date: today, reserve: 0, done: false });
    }
    guard.as_mut().map(f)
}

/// Today's reserve in silver: the most expensive gold item for our class seen today. Other gold spending
/// (attributes, guild skill for tasks) keeps this much so a better item can still be bought (user 2026-10-07).
pub fn reserve() -> u64 {
    with_day(|d| u64::from(d.reserve)).unwrap_or(0)
}

/// Can be bought for gold only (no mushrooms) and is a real equipment item.
fn gold_only(item: &Item) -> bool {
    item.mushroom_price == 0
        && item.price > 0
        && item.price != u32::MAX
        && !item.is_unique()
        && item.typ.equipment_slot().is_some()
}

/// Hourglasses for gold (user 2026-10-07): fine for spinning the shop, they are saved, never used.
fn gold_hourglass(item: &Item) -> bool {
    item.mushroom_price == 0 && item.price > 0 && item.price != u32::MAX && item.typ == ItemType::QuickSandGlass
}

/// Gold items that can be used to spin the shop: equipment (sold right away), hourglasses (kept) and potions
/// while the potion stock has room (kept, user 2026-10-07: cheap, spin the shop, can be sold any time).
fn spin_offers(gs: &GameState) -> Vec<(ShopPosition, &Item)> {
    let potions = crate::potions::stock_has_room(gs);
    gs.shops
        .values()
        .flat_map(|s| s.iter())
        .filter(|(_, i)| {
            gold_only(i)
                || gold_hourglass(i)
                || (potions
                    && crate::potions::potion(i).is_some()
                    && i.mushroom_price == 0
                    && i.price > 0
                    && i.price != u32::MAX)
        })
        .collect()
}

/// All items for gold only from both shops.
fn gold_offers(gs: &GameState) -> Vec<(ShopPosition, &Item)> {
    gs.shops.values().flat_map(|s| s.iter()).filter(|(_, i)| gold_only(i)).collect()
}

/// Raises the reserve to the most expensive gold item for our class in the current offer.
fn update_reserve(gs: &GameState) -> u32 {
    let class = gs.character.class;
    let max = gold_offers(gs)
        .iter()
        .filter(|(_, i)| i.can_be_equipped_by(class))
        .map(|(_, i)| i.price)
        .max()
        .unwrap_or(0);
    with_day(|d| {
        d.reserve = d.reserve.max(max);
        d.reserve
    })
    .unwrap_or(u32::MAX)
}

/// The affordable gold item with the biggest improvement over the equipped one.
fn best_upgrade(gs: &GameState) -> Option<(ShopPosition, &Item, f64, f64)> {
    let ch = &gs.character;
    gold_offers(gs)
        .into_iter()
        .filter(|(_, i)| u64::from(i.price) <= ch.silver && i.can_be_equipped_by(ch.class))
        .filter_map(|(pos, i)| {
            let slot = i.typ.equipment_slot()?;
            let new = inventory::value(gs, i);
            let cur = ch.equipment.0[slot].as_ref().map_or(-1.0, |c| inventory::value(gs, c));
            (new > cur).then_some((pos, i, new, cur))
        })
        .max_by(|a, b| (a.2 - a.3).total_cmp(&(b.2 - b.3)))
}

/// The cheapest non-epic affordable gold item that an open task wants bought (Gleeman/event tasks:
/// buy a weapon in the Weapon Shop, buy N items in a shop, buy hourglasses). Ignores the reserve: equipment is sold
/// again right away, hourglasses are kept.
fn task_purchase(gs: &GameState) -> Option<(ShopPosition, &Item)> {
    let weapon = crate::tasks::remaining(gs, |t| t == TaskType::BuyWeaponInWeaponsShop) > 0;
    let from = |shop: ShopType| crate::tasks::remaining(gs, |t| t == TaskType::BuyFromShop(shop)) > 0;
    let hourglasses = crate::tasks::remaining(gs, |t| t == TaskType::BuyHourGlasses) > 0;
    spin_offers(gs)
        .into_iter()
        .filter(|(_, i)| !i.is_epic() && u64::from(i.price) <= gs.character.silver)
        .filter(|(pos, i)| {
            (weapon && pos.typ == ShopType::Weapon && matches!(i.typ, ItemType::Weapon { .. }))
                || (hourglasses && i.typ == ItemType::QuickSandGlass)
                || (from(pos.typ) && gold_only(i))
        })
        .min_by_key(|(_, i)| i.price)
}

/// The cheapest non-epic gold item, if buying it keeps the gold at or above the reserve.
fn spin_candidate(gs: &GameState, reserve: u32) -> Option<(ShopPosition, &Item)> {
    spin_offers(gs)
        .into_iter()
        .filter(|(_, i)| !i.is_epic())
        .min_by_key(|(_, i)| i.price)
        .filter(|(_, i)| gs.character.silver >= u64::from(i.price) + u64::from(reserve))
}

fn fail(e: &sf_api::error::SFError) -> Outcome {
    report!("[shops] Error: {e}");
    if crate::tavern::is_session_error(e) { Outcome::SessionLost } else { Outcome::Done }
}

/// Buys one item into a free backpack slot, refreshes the state and lets the inventory equip/sell it.
/// Returns false when shopping must stop (no free slot, the shop did not change).
async fn buy(session: &mut SimpleSession, shop_pos: ShopPosition) -> Result<bool, Outcome> {
    let Some(gs) = session.game_state() else { return Err(Outcome::Done) };
    if gs.character.inventory.free_slot().is_none() && !crate::potions::make_room(session).await? {
        report!("[shops] Backpack full, stopping shopping");
        return Ok(false);
    }
    let Some(gs) = session.game_state() else { return Err(Outcome::Done) };
    let Some(bag) = gs.character.inventory.free_slot() else { return Ok(false) };
    let Some(item) = gs.shops[shop_pos.typ].items.get(shop_pos.pos) else { return Ok(false) };
    let item_ident = item.command_ident();
    safe::send(session, Command::BuyShop { shop_pos, new_pos: bag.into(), item_ident })
        .await
        .map_err(|e| fail(&e))?;
    safe::send(session, Command::Update).await.map_err(|e| fail(&e))?;
    let Some(gs) = session.game_state() else { return Err(Outcome::Done) };
    // The bought slot must get a new item, otherwise we would try to buy the same one again
    if gs.shops[shop_pos.typ].items.get(shop_pos.pos).is_some_and(|i| i.command_ident() == item_ident) {
        report!("[shops] The shop offer did not change after the purchase, stopping (verify)");
        return Ok(false);
    }
    if let Outcome::SessionLost = inventory::manage(session).await {
        return Err(Outcome::SessionLost);
    }
    Ok(true)
}

fn describe(pos: ShopPosition, item: &Item) -> String {
    format!(
        "{:?} from the {:?} shop (slot {}) for {}{}",
        item.typ,
        pos.typ,
        pos.pos + 1,
        crate::report::gold(u64::from(item.price)),
        if item.is_epic() { ", epic" } else { "" }
    )
}

/// Shopping once a day after the Tavern is done. Sends nothing when there is nothing to do.
pub async fn run(session: &mut SimpleSession, tavern_done: bool) -> Outcome {
    if !tavern_done || with_day(|d| d.done).unwrap_or(true) {
        return Outcome::Done;
    }
    with_day(|d| d.done = true);
    let outcome = shop(session).await;
    if let Some(gs) = session.game_state() {
        report!("[shops] Done, gold left {}", crate::report::gold(gs.character.silver));
    }
    outcome
}

async fn shop(session: &mut SimpleSession) -> Outcome {
    let mut spins = 0;
    let mut task_buys = 0;
    let mut potion_buys = 0;
    let mut attributes_done = false;
    loop {
        let Some(gs) = session.game_state() else { return Outcome::Done };
        let reserve = update_reserve(gs);

        if let Some((pos, item, new, cur)) = best_upgrade(gs) {
            report!("[shops] Buying an upgrade: {} (value {new:.1} > {cur:.1})", describe(pos, item));
            match buy(session, pos).await {
                Ok(true) => continue,
                Ok(false) => return Outcome::Done,
                Err(o) => return o,
            }
        }

        // Potions after equipment, before tasks and spinning (user 2026-10-07; may go below the reserve)
        if let Some((step, what)) = crate::potions::shop_step(gs).filter(|_| potion_buys < MAX_POTION_BUYS) {
            potion_buys += 1;
            report!("[shops] Potion: {what}");
            let (pos, remove) = match step {
                crate::potions::ShopStep::Drink(pos) | crate::potions::ShopStep::Keep(pos) => (pos, None),
                crate::potions::ShopStep::Replace(slot, pos) => (pos, Some(slot)),
            };
            match buy(session, pos).await {
                Ok(true) => {}
                Ok(false) => return Outcome::Done,
                Err(o) => return o,
            }
            // Bought first, only then remove the old one, so a failed purchase never costs a potion
            if let Some(slot) = remove
                && let Outcome::SessionLost = crate::potions::remove(session, slot).await
            {
                return Outcome::SessionLost;
            }
            if let Outcome::SessionLost = crate::potions::drink_from_bag(session).await {
                return Outcome::SessionLost;
            }
            if let Outcome::SessionLost = crate::potions::trim_stock(session).await {
                return Outcome::SessionLost;
            }
            continue;
        }

        if let Some((pos, item)) = task_purchase(gs).filter(|_| task_buys < MAX_TASK_BUYS) {
            task_buys += 1;
            report!("[shops] Buying for a task: {}", describe(pos, item));
            match buy(session, pos).await {
                Ok(true) => continue,
                Ok(false) => return Outcome::Done,
                Err(o) => return o,
            }
        }

        if spin_offers(gs).is_empty() {
            report!("[shops] All items cost mushrooms, nothing more to buy");
            return Outcome::Done;
        }
        // Attribute tasks that help to a better chest go before spinning (user 2026-10-07); they keep the reserve
        if !attributes_done && crate::tasks::attributes_needed(gs) {
            attributes_done = true;
            report!("[shops] Attribute tasks help to a better chest, buying them before spinning");
            if let Outcome::SessionLost = crate::tasks::buy_attributes(session).await {
                return Outcome::SessionLost;
            }
            continue;
        }
        if spins >= MAX_SPINS {
            report!("[shops] Spin limit ({MAX_SPINS}) for today reached");
            return Outcome::Done;
        }
        let Some((pos, item)) = spin_candidate(gs, reserve) else {
            report!(
                "[shops] Not spinning: gold {}, reserve {} (most expensive item seen today)",
                crate::report::gold(gs.character.silver),
                crate::report::gold(u64::from(reserve))
            );
            return Outcome::Done;
        };
        let silver_before = gs.character.silver;
        report!("[shops] Spin {}: buying {} to sell it again", spins + 1, describe(pos, item));
        match buy(session, pos).await {
            Ok(true) => {}
            Ok(false) => return Outcome::Done,
            Err(o) => return o,
        }
        spins += 1;
        if let Some(gs) = session.game_state() {
            report!(
                "[shops] Spin cost {}",
                crate::report::gold(silver_before.saturating_sub(gs.character.silver))
            );
        }
    }
}
