# Inventory

## Rules (from the user)
- Whenever an equippable item arrives, decide right away whether it is better than the equipped one.
  Value = the same formula as Arena power: 100 % main attribute + 80 % CON + 27 % LCK + 2 % secondary (from the item's attributes).
- Weapons (user request: ignore armor, a separate equation for weapons):
  value = average damage × (1 + M / 20), M = the character's total main attribute with this weapon.
  Derived from the sf-api fight simulation (`simulate/damage.rs`): hit = weapon × (1 + A / 10), A = max(M / 2, M − M_enemy / 2),
  against an equally strong enemy A = M / 2.
  Plus the weapon's other stats with the same percentages as everywhere: 80 % CON + 27 % LCK + 2 % secondary
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
- An EMPTY gem slot counts ONLY while a loose gem lies in the backpack (user 2026-10-15, was 50 % of the best gem we had, which let junk-stat items with a slot beat real upgrades and epics: Wecros STR+36 amulet 74 vs epic 39.3, TestChar1 DEX+30 gloves 64 vs epic 36.3). Then it is worth that gem with its REAL type (`best_backpack_gem`, via `gem_effect`; fixed 2026-10-15 – before, any loose gem was valued as a main-attribute gem of the same power, so Pjotr's Strength gem +86 added +86 to shop/backpack items). Without a loose gem the slot is worth 0. Even with a loose gem, an empty slot counts only on a non-weapon item that has main attribute, CON or LCK of its own (user 2026-10-15: side-stat-only gear must not win or be kept on a hoped-for gem, e.g. a Battle Mage INT breastplate beating STR+29); weapons are exempt (judged by damage). The same test (`inventory::worth_a_gem`) decides where `gems.rs` may socket a gem: never into side-stat-only gear (a socketed gem cannot be taken out). Found 2026-10-15 on already-socketed items: TestChar1 chest INT+44, Novotné talisman STR+47, Pjotr chest STR+26 and ring STR+38, Wecros gloves INT+33 – they stay, the gems are stuck.
- (2026-10-16, replaces the rule below) `keep_candidate`: one item kept for an empty gem slot, only if the expected gem makes it ≥ 20 points better than the worn one, even with a tight backpack; `swap_kept_item` when the backpack is completely full (after potions and a loose gem). Old rule: Keep instead of sell (`keep_for_gem`): an item that is worse only because its slot is empty, but would beat the equipped one with the loose backpack gem, is kept in the backpack while ≥ 2 slots are free. `gems::socket_backpack_gem` then sockets the next gem into such a stored item (`PlayerItemMove` gem → the item's backpack position, shape UNVERIFIED like the equipped case) when it beats the worn one; the next inventory pass equips it.
- Epic items arriving in the backpack are announced once (`shops::note`: log `[ALERT]` and `roster/alerts.txt`, NO popup since 2026-10-16); the first look after a start only records what is already owned.

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
- Gem types (user 2026-10-15): All (black sprite) = every attribute; Legendary (orange) = main attribute + CON only (`gem_effect`); Constitution = purple. Both rare.

## Completely full backpack (2026-10-16, user)
`inventory::last_resort_sale`, used by `potions::make_room` when no potion can free a slot and there are 0 free slots (with 1 free slot nothing is sold): 1) the weakest loose gem is sold (no socket for it), 2) the most expensive epic equipment (for the future toilet only the number of epics counts). Potions go first as before (the least important, e.g. a mid Luck potion). Unknown non-equipment items (event pumpkin bag, taco) are left alone.

## Weapon gets the strongest gem (2026-10-16, user)
A weapon's gem counts double, so a weapon with a gem slot should always carry the strongest main-attribute gem (else a Constitution one; All/Legendary gems count as main). `gems::weapon_socket`, tried before every other socket and before stored backpack items: an empty weapon socket takes any tier-0/1 gem, a filled one is REPLACED by a gem of a better tier (main over Constitution) or of the same tier with more power (`gem_power`); the replaced gem is lost (no extraction without mushrooms). Luck gems still go to the item that is swapped out soonest. Other sockets are unchanged. Only the `Weapon` slot – an Assassin's off-hand (`Shield` slot) is NOT treated specially (unknown whether its gem counts double). Unverified live; test `weapon_gets_the_strongest_gem`.

## Items are compared WITHOUT gems (2026-10-16, user) – supersedes the "socketed gem counts fully" lines above
Gems are cheap for us (the mine produces one about every 40 min, 24/7), so for equip/sell/buy decisions (`inventory::value_in`, `Gems::Rare`) a socketed gem is IGNORED; the only exception is a socketed Legendary or All (black) gem on the WORN item, which counts. An empty slot counts nothing. Swapping an item therefore throws its gem away (accepted), e.g. Filminy's epic gloves 46/46/46 in the backpack beat the worn 34/34/34 + DEX gem on stats and are put on at once; the incoming gems go into empty slots. The dashboard still shows the real value with every socketed gem (`value_as_worn`, `Gems::Socketed`).
Kept for a gem (`keep_candidate`, user: "if there is room, keep the new item and wait for a gem, we should have that written down" – it was): a non-epic item with an empty slot that is worse than the worn one by stats is kept in the backpack while the expected gem makes it ≥ `KEEP_MIN_GAIN` (20) better, but ONLY when the worn item has no gem slot at all (a worn item with a slot gets a gem as well, so the plain stat comparison decides). The gem pass sockets a new gem into the kept item (`gems::socket_backpack_gem`, stored branch uses `inventory::kept_item`), then the inventory pass equips it. Epics are never sold anyway and are not "kept" in this sense.

## Main attribute / Constitution balance (2026-10-16, user) – `src/balance.rs`
Ideal = Constitution at 80 % of the main attribute (1000 : 800). Item and gem values are now judged on the RESULTING (main, CON) pair of the character: `balance::value_of(main, con) = (main + 0.8·con) × (1 − BALANCE_STRENGTH × d²)`, `d = con / (0.8·main) − 1` (clamped to ±1). `BALANCE_STRENGTH = 0.25` (50 % deviation = 6 % less value); **to calibrate change `CON_TARGET` / `BALANCE_STRENGTH` at the top of `balance.rs`; 0.0 switches it off** (old linear weights). Near the ideal nothing changes (1000 : 799 loses 0.0002 %). Examples (tests): 1200 : 600 → CON +50 = +54, INT +60 = +48 (old weights 40 vs 60); Mimimimi11 1582 : 823 the same.
- Raw numbers only: bought points + worn items + their socketed gems; NO potions, pets, guild, ... (`inventory::raw_totals`).
- Item value (non-weapon) = `value_of(rest + item) − value_of(rest)` + Luck 27 % / secondary 10 % (fixed, outside the balance, so an unbalanced build never makes a Luck item better than a main one). Weapons: damage part as before, their main + CON (+ gem) go through the balance against the current build. Gems: `gem_parts` (main / CON / rest) in the same function; Legendary and All count as main + CON.
- Gem placement: the weapon still gets the strongest main gem first; every other place goes to the loose gem with the biggest `inventory::gem_gain` (balance included), e.g. with CON lacking a CON gem goes before a main one. Replacement rules by tier (never replace a main/CON gem) unchanged.
- NOT changed yet: potions (`potions.rs`) and bought attributes (`attributes.rs`) still use the fixed weights.
- (2026-10-16, user) Bought attributes: no balance, fixed weights per gold 100 : 80 : 40 (Luck 0.4 only here, `attributes::BUY_LUCK_WEIGHT`; the rest of the code uses 0.27).

**Verified 2026-10-17 (user):** gems can only be socketed into a worn item. `gems.rs` equips the kept item first, then socketing.
## Loose gems go first (2026-10-18, user)
No cap and no active selling. When a backpack slot is needed for anything (epic, potion, hunt stripping), `gems::free_slot` runs first (`potions::make_room`): a gem with a place is socketed (worn item worse), otherwise the weakest loose gem is sold (`inventory::spare_gem`). Replaces the "any number while one slot stays free" holding rule for the moment a slot is wanted; the no-epics hunt uses only this step.
n## Gem slot weight (2026-10-18, user)
Socketed main/CON/All/Legendary gems count fully in `inventory::value_core` (Luck/secondary stay replaceable); an item with a usable empty slot gets `slot_bonus` of `expected_gem`: the loose gem with the best `gem_gain`, else a gem of the lacking kind (Constitution below `CON_TARGET` x main, else main) as strong as the best worn gem. Case: Mrožik, gloves +46 (no slot) replaced +43 with CON gem +75.

## Worn gem vs a shop/drop item with an empty socket (2026-10-18, user, Pagan) – `inventory::compare`
Pagan (STR gem 147 in the weapon, CON gem 110 in the belt) was judged to KEEP both because the worn item's socketed main/CON gem counted in full while the shop item's empty socket counted nothing. User: a gem can simply be mined again for the new socket, so it is no advantage of the worn item; compare the plain stats. Rules now: (1) the new item has a usable socket (empty, or a Luck/secondary gem in it) -> BOTH items are valued WITHOUT socketed gems (only Legendary/All still count); (2) the new item has NO socket -> the worn gem counts in full; (3) a gem in the main weapon counts DOUBLE (`WEAPON_GEM_FACTOR`, also in the character's raw totals and in the damage via the main attribute). Used by `inventory::next_action` and the shops (`best_upgrade`, the offer journal). Test `empty_socket_beats_a_worn_gem_when_the_stats_are_better`. Not in a release yet.

## Endless swap fix (2026-10-19, Pagan + Mimimimi11) – `inventory::compare`
Rule (1) above was judged by the NEW item only: item A (empty socket) beat the worn B (socketed main/CON gem) in Bare mode, and once A was worn, B (no usable socket) beat A back in Rare mode (gem in full). Pagan swapped two talismans 30+ times in 6 minutes (value 96.3 vs 215.0), Mimimimi11 two weapons (593.8 vs 1425.0); every pass hit "Action limit for one pass reached" and Pagan's Tavern stalled with full Thirst for Adventure. Now a usable socket on EITHER item puts both in Bare mode (symmetric, so no cycle). Test `gem_item_and_empty_socket_item_do_not_swap_forever`. Needs a release build + bot restart to take effect.

## Epics are sold only from a backpack full of epics (2026-10-19, user)
`inventory::last_resort_epic` (reached via `potions::make_room`, used by `shops.rs` and `potions::trim_stock`; hunts no longer call it) used to fire for ANY completely full backpack although its comment said "full of epics". Now it returns `None` unless there is no free slot AND every slot holds an epic. Test `epic_is_sold_only_from_a_backpack_full_of_epics`. Needs a release build + restart.

## One backpack slot always stays free (2026-10-22, user)
Replaces the "epic sold only from a backpack full of epics" rule. Reason: the user wants the toilet (level 100) ASAP, so levels -> XP -> Dungeons -> gear/stats -> potions and gems; a main potion (big main/CON/Eternal Life) or a mined gem must always have a slot to land in. An epic is worth ~3k gold, so selling one is cheap.
- `inventory::last_resort_epic` now fires for ANY full backpack (no free slot), after `potions::make_room` tried a loose gem (user: always sell/socket the gem first), then potion steps (an unwanted/smaller potion is sold, a bigger bag potion is drunk instead of kept), then the kept item. Sells the most expensive epic (user: all epics are class-usable so far; count is what matters). Never unique, never a non-equipment epic.
- `potions::run` runs every pass (frees a slot when none is free); `dungeons::run` now frees a slot (`make_room`) instead of just skipping the fight, loot from a fight is handled on the next pass.
- Hunts still never sell an epic (`make_room_for_hunt`).
- Witch (user): a forced epic sale picks the best payout (`inventory::payout`, 2x at the witch for her type of the day, `WitchDropCauldron`); see `docs/witch.md`.
- Test `epic_is_sold_only_from_a_full_backpack`. Needs a release build + restart.
