# Inventory

## Rules (from the user)
- Whenever an equippable item arrives, decide right away whether it is better than the equipped one.
  Value = the same formula as Arena power: 100 % main attribute + 80 % CON + 27 % LCK + 10 % secondary (from the item's attributes).
- Weapons (user request: ignore armor, a separate equation for weapons):
  value = average damage × (1 + M / 20), M = the character's total main attribute with this weapon.
  Derived from the sf-api fight simulation (`simulate/damage.rs`): hit = weapon × (1 + A / 10), A = max(M / 2, M − M_enemy / 2),
  against an equally strong enemy A = M / 2.
  Plus the weapon's other stats with the same percentages as everywhere: 80 % CON + 27 % LCK + 10 % secondary
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

## Weapon damage on the attribute scale (2026-10-14, user)
Before, weapon damage (avg × (1 + M/20), thousands) was added to the weapon's other stats (tens of points), so Luck/secondary stats never mattered. Now `inventory::damage_points` converts the damage to main-attribute points relative to the weapon worn in that slot (bare hands without one): `damage / damage_ref × (20 + M)`, i.e. +1 main attribute point = +1 value, +10 % damage at M = 100 = +12 points. The weapon's other stats (80 % CON, 40 % LCK, 10 % secondary) are added as before. After the next release build check `inventory.log`: a clearly stronger weapon still wins, near-equal ones now go by their stats.

### Simulation check of the weights (2026-10-14, `src/weaponsim.rs`, ignored tests)
Run: `CARGO_TARGET_DIR=<elsewhere> cargo test --release weaponsim -- --ignored --nocapture` (needs `roster/*/now.json`, ~2 min). Uses sf-api's `simulate_battle` (armor included, runes/portal 0) against a pool of the 11 roster characters scaled to our main attribute ±20 % (mean win rate ≈ 50 %). Central differences = value of one point in "main attribute points" (= 1.0):

| | measured (mean of 11) | Arena formula |
|---|---|---|
| CON | **1.2** (0.8 – 1.6 by class) | 0.8 |
| LCK | **0.27** (first measurement 0.13 was a bug, see below) | 0.4 |
| secondary attribute | **0.15** | 0.10 |
| +1 % weapon damage | **0.70 × (20+M)/100** | (20+M)/100 |

Why damage is 0.7×: the sim's attribute bonus is A = max(M/2, M − M_enemy/2), so a main point is worth 1/(20+M) of the damage against a stronger enemy and 2/(20+M) against a weaker one (mean 1.5 → 0.67). The Assassin (two weapons) measured an outlier (0.64 instead of ~3.9 for 1 % damage per main point) – not understood, treat Assassin weapon results with care.
Validation on 400 random weapon swaps (damage ×0.8–1.25, ±30 main, −30…+60 CON, −20…+60 LCK, 0…40 secondary) against the simulated win-rate change: old raw-damage formula corr 0.81 / sign 86.5 %, `damage_points` with the Arena weights corr 0.94 / 94.8 %, **calibrated (0.7 × damage, CON 1.2, LCK 0.27, secondary 0.15) corr 0.98 / 98.5 %**. The calibrated weights are NOT in the code yet (user decision pending: they differ from the Arena formula the user dictated).

## Gem-aware item comparison (2026-10-12, user)
Rule stays: better equipment (raw power by our formula, armor and item quality ignored unless the items are otherwise identical) is kept, worse is sold, epics are never sold (kept for the toilet of the gods). New: gems count (`inventory::value_core`).
- A SOCKETED gem counts fully for the item (it cannot be taken out, so swapping the item loses it): main-attribute gem → main attribute (for weapons into the damage multiplier), Constitution 80 %, Luck 27 %, a secondary attribute 10 %, All/Legendary all of them (`gem_effect`).
- An EMPTY gem slot counts ONLY while a loose gem lies in the backpack (user 2026-10-15, was 50 % of the best gem we had, which let junk-stat items with a slot beat real upgrades and epics: Wecros STR+36 amulet 74 vs epic 39.3, TestChar1 DEX+30 gloves 64 vs epic 36.3). Then it is worth that gem as a main-attribute gem (full value). Without a loose gem the slot is worth 0. Even with a loose gem, an empty slot counts only on a non-weapon item that has main attribute, CON or LCK of its own (user 2026-10-15: side-stat-only gear must not win or be kept on a hoped-for gem, e.g. a Battle Mage INT breastplate beating STR+29); weapons are exempt (judged by damage).
- Keep instead of sell (`keep_for_gem`): an item that is worse only because its slot is empty, but would beat the equipped one with the loose backpack gem, is kept in the backpack while ≥ 2 slots are free. `gems::socket_backpack_gem` then sockets the next gem into such a stored item (`PlayerItemMove` gem → the item's backpack position, shape UNVERIFIED like the equipped case) when it beats the worn one; the next inventory pass equips it.
- Epic items arriving in the backpack are announced once (`shops::alert`: log `[ALERT]`, `roster/alerts.txt`, popup); the first look after a start only records what is already owned.

### Verified from the user's necklace capture (2026-10-12)
The same necklace seen before and after socketing a Constitution gem (type 33 = filled, power 66): all attribute values of the item are IDENTICAL, only the gem-slot field (1 → 33) and the gem power (0 → 66) changed. So `item.attributes` do NOT include the gem – the gem is separate and `value_core` correctly adds it on top. The user also confirmed that the gem goes into a stored backpack item with the same `PlayerItemMove` (target = the item's position, "should be so"). A socketed gem stays in the item when it is moved between equipment and backpack.

### Implemented 2026-10-14 (after the simulation, user: "we can lower Luck")
- `arena::LUCK_WEIGHT` = 0.27 (was 0.4; 0.15 for a few hours after a measuring bug) in `arena::power`, `arena::weight`, `gem_effect` (so potions, attribute buying and Arena opponent choice follow). CON/secondary unchanged.
- `inventory::DAMAGE_WEIGHT` = 0.75; the weapon's main-attribute bonus counts 1:1 (not weighed).
- Assassin: both hands together; the other hand counts with its own bare-hands floor. Simulator check on Sanek: the off-hand 28–48 is below the level-26 bare-hands floor 29–57, so the sim ignores it (scaling it changes nothing) – consistent with `effective_avg_damage`.

### Why the measured weights look the way they do (2026-10-14)
- In the simulator the attribute bonus is A = max(M/2, M − E/2) where E = the ENEMY's attribute of the same type as OUR main. Against enemies whose attribute E is small (typical) A = M − E/2, so one main point adds 1/(10 + A) ≈ 1/(M − 40) of the damage; one CON point adds 1/CON of the hit points. So **CON is worth ≈ 0.9 × (M − 40) / CON main points** – it depends on how big CON is relative to the main attribute (checked on all 11 roster characters: theory 0.87–1.75, measured 0.80–1.58, same order). A flat 0.8 is therefore only right for CON ≈ main × 0.8; a character with low CON compared to its main (Filminy, Mimimimi11, Mrožik, Novotné) gains more per CON point than per main point. Possible refinement: a per-character CON weight instead of 0.8.
- Secondary attributes are DEFENSE: they are the `E` of enemies whose main attribute has that type, so a secondary point only matters against about a third of the enemy classes (each hit −0.5 A) → ≈ 0.15. Luck only adds crit chance 5·LCK/(2·enemy level) % (crit ×2, so +1 point of crit = +1 % damage, verified in the simulator: +LCK and the equivalent +damage give the same win rate) → ≈ 0.27 (the first 0.13 was a measuring bug: negative LCK steps were ignored); the 50 % cap needs LCK = 20 × level (level 26: 520, the characters have 140–180), so the cap is far away for now.
- Bare-hands damage is only known from sf-api's code (`get_hand_damage`), not seen in the real game; the user prefers wearing an item to relying on it (an item below the floor still adds its stats and is never sold in favour of hands).
