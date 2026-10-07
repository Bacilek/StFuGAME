//! Weapon Shop and Magic Shop, once a day after the Tavern is done (the whole Thirst for Adventure used up).
//! Buys ONLY for gold (`mushroom_price == 0`), never for mushrooms, never `RefreshShop` (costs a mushroom).
//! 1. Upgrades: a gold item better than the equipped one (by `inventory::value`) → buy, `inventory::manage` equips it
//!    and sells the old one (never epic items). Upgrades may use all the gold.
//! 2. Spinning: buy the cheapest non-epic gold item and sell it right away (`inventory::manage`) to get a new offer.
//!    Only while the gold after the purchase stays at or above the reserve = the most expensive gold item for our class
//!    seen today, so an upgrade that shows up after a spin can always be bought. Ends when no gold items are left.

use std::sync::Mutex;

use chrono::{Local, NaiveDate};
use sf_api::{
    command::Command,
    gamestate::{GameState, ShopPosition, items::Item},
};

use crate::session::SimpleSession;

use crate::{inventory, safe, tavern::Outcome};

/// Safety limit on spins per day.
const MAX_SPINS: usize = 60;

/// Today's shopping: the reserve and whether shopping is done.
struct Day {
    date: NaiveDate,
    reserve: u32,
    done: bool,
}

static DAY: Mutex<Option<Day>> = Mutex::new(None);

/// Runs `f` on today's state (a new day starts with a zero reserve).
fn with_day<T>(f: impl FnOnce(&mut Day) -> T) -> Option<T> {
    let mut guard = DAY.lock().ok()?;
    let today = Local::now().date_naive();
    if guard.as_ref().is_none_or(|d| d.date != today) {
        *guard = Some(Day { date: today, reserve: 0, done: false });
    }
    guard.as_mut().map(f)
}

/// Can be bought for gold only (no mushrooms) and is a real equipment item.
fn gold_only(item: &Item) -> bool {
    item.mushroom_price == 0
        && item.price > 0
        && item.price != u32::MAX
        && !item.is_unique()
        && item.typ.equipment_slot().is_some()
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

/// The cheapest non-epic gold item, if buying it keeps the gold at or above the reserve.
fn spin_candidate(gs: &GameState, reserve: u32) -> Option<(ShopPosition, &Item)> {
    gold_offers(gs)
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
    let Some(bag) = gs.character.inventory.free_slot() else {
        report!("[shops] Backpack full, stopping shopping");
        return Ok(false);
    };
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

        if gold_offers(gs).is_empty() {
            report!("[shops] All items cost mushrooms, nothing more to buy");
            return Outcome::Done;
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
