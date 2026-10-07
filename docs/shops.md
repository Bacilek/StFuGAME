# Shops (Weapon Shop, Magic Shop)

## Rules (from the user, 2026-10-07)
- NEVER buy for mushrooms, only for gold. Never `RefreshShop` (a new offer costs a mushroom).
- Shopping only after the whole Thirst for Adventure is used up (Tavern done), so the best items are already in.
- An item that improves the equipment even a little (1 stat point is enough) → buy it and equip it. Epic items are still never sold.
- Spare gold → "spin" the shop: buy items for gold even when they are worse, sell them right away and look at the new offer.
  Continue until all items in the shops cost mushrooms.
- Spare gold = above a dynamic reserve: the price of the most expensive gold item for our class seen that day
  (so an upgrade that shows up after a spin can always be bought). Upgrades may use all the gold.

## Implementation (src/shops.rs)
- Once a day (`Day` with the date), called in the main loop when the Tavern is done, before City Guard.
- Gold item = `mushroom_price == 0`, real equipment (`equipment_slot`), not unique.
- Upgrade = the gold item with the biggest gain over the equipped one by `inventory::value` (same as the backpack), affordable.
  Bought into a free backpack slot, then `inventory::manage` equips it and sells the old one (unless epic).
- Spin = the cheapest non-epic gold item, only if gold − price ≥ reserve; `inventory::manage` sells it. Cost logged
  as `[shops] Spin cost` (purchase − sale). Limit 60 spins a day.
- Stops when: no gold items in either shop, not enough gold above the reserve, the backpack is full,
  or the bought shop slot did not change after `Update` (stale shop data, needs verification).
- `safe.rs`: `BuyShop` passes only when the item at `shop_pos` has `mushroom_price == 0`, matches `item_ident`
  (it contains both prices; the server also rejects a changed item) and we have the gold. Plus the mushroom watchdog.

## Shop refresh for an ad (idea, not implemented)
- The user: once a day each shop can be rerolled for watching an ad. Wanted: do it after everything costs mushrooms.
- sf-api 0.4.4 has no command for it (only `RefreshShop` = 1 mushroom, forbidden). The server sends a `skipvideo` key, sf-api ignores it.
- To implement we would need the exact server command the client sends after the ad (`Command::Custom`), e.g. captured
  from the browser DevTools (Network tab) when the user watches the ad. Until then not possible.

## Verification status
| What | Status |
|---|---|
| Gold-only items have `mushroom_price == 0` | ⏳ not verified |
| `BuyShop` costs only gold, mushrooms unchanged | ⏳ not verified |
| After a purchase the shop slot gets a new item (and `Update` shows it) | ⏳ not verified |
| Spin cost (purchase vs sale price) | ⏳ not verified |
| Shopping works during City Guard | ✅ 2026-10-07 per the user (everything works during a shift); bot run not seen yet |
