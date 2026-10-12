# Inventory

## Rules (from the user)
- Whenever an equippable item arrives, decide right away whether it is better than the equipped one.
  Value = the same formula as Arena power: 100 % main attribute + 80 % CON + 40 % LCK + 10 % secondary (from the item's attributes).
- Weapons (user request: ignore armor, a separate equation for weapons):
  value = average damage × (1 + M / 20), M = the character's total main attribute with this weapon.
  Derived from the sf-api fight simulation (`simulate/damage.rs`): hit = weapon × (1 + A / 10), A = max(M / 2, M − M_enemy / 2),
  against an equally strong enemy A = M / 2.
  Plus the weapon's other stats with the same percentages as everywhere: 80 % CON + 40 % LCK + 10 % secondary
  (the weapon's main attribute is already in the damage via M and is not counted twice). User 2026-10-07: "all their stats, by percentage".
- Better → equip. Worse (the new one or the one taken off) → sell. **Never sell epic items**, we keep them for later.

## Implementation (src/inventory.rs)
- Goes through the whole backpack (not just new items), so it also catches up on older items. Sends nothing to the server without an action.
- An item for another class (cannot be equipped) → sell, unless it is epic.
- Equal value = not better → sell (unless epic).
- Unique items (`is_unique`) and non-equipment (potions etc.) are left alone.
- `Update` after every action, decisions are always made on a fresh state.
- Selling: `SellShop` (sf-api picks the shop and slot itself), costs no mushrooms. Equipping: `Equip`, the removed item drops into the backpack
  and is evaluated in the next step (sold unless epic).
- Epic = `Item::is_epic()` (model_id ≥ 50, includes legendary).
- Assassin dual-wields: a weapon in the backpack can go into either `Weapon` or `Shield` (sf-api types the
  off-hand weapon as `ItemType::Weapon` too, not `Shield` – only Paladin/Warrior ever get a real shield there,
  per sf-api's `Class::can_wear_shield`). A new weapon competes against whichever of the two is currently
  weaker (or empty) and is equipped into that slot (2026-10-08, ahead of Sanek/Assassin's first run).

## Verification status
| What | Status |
|---|---|
| After `Equip` the old item drops into the backpack | ✅ 2026-10-07 (boots, weapon) |
| Selling via `SellShop` adds silver | ✅ 2026-10-07 (boots for 1 g 25 s) |
| Assassin off-hand weapon (Shield slot) gets filled/upgraded correctly | ⏳ not verified (no Assassin has run yet) |
- 2026-10-10: a weapon weaker than the bare-hand damage (min and max both below) is ignored by the game, so its value is the hands'. Decisions (equip/sell, shop weapon offers) are journaled in `roster/<character>/logs/inventory.log`.

## Gem-aware item comparison (2026-10-12, user)
Rule stays: better equipment (raw power by our formula, armor and item quality ignored unless the items are otherwise identical) is kept, worse is sold, epics are never sold (kept for the toilet of the gods). New: gems count (`inventory::value_core`).
- A SOCKETED gem counts fully for the item (it cannot be taken out, so swapping the item loses it): main-attribute gem → main attribute (for weapons into the damage multiplier), Constitution 80 %, Luck 40 %, a secondary attribute 10 %, All/Legendary all of them (`gem_effect`).
- An EMPTY gem slot counts `POTENTIAL_SHARE` (50 %) of the best gem we have (socketed or in the backpack), as a main-attribute gem – not every item has a slot (e.g. main hand weapon 30 + gem +66 beats a raw 50 weapon with an empty slot, so the new one is not equipped for its raw numbers).
- Keep instead of sell (`keep_for_gem`): an item that is worse only because its slot is empty, but would beat the equipped one with a gem of our current mining strength, is kept in the backpack while ≥ 2 slots are free. `gems::socket_backpack_gem` then sockets the next gem into such a stored item (`PlayerItemMove` gem → the item's backpack position, shape UNVERIFIED like the equipped case) when it beats the worn one; the next inventory pass equips it.
- Epic items arriving in the backpack are announced once (`shops::alert`: log `[ALERT]`, `roster/alerts.txt`, popup); the first look after a start only records what is already owned.
