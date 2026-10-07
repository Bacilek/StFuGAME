# Goblin Gleeman (daily tasks) and event tasks

## Rules (from the user, 2026-10-07)
- Every day the Goblin Gleeman (Quarter) offers tasks; completed tasks give points, points open up to 3 reward chests.
  Day 1 of a new character is extra important (top chest = 10 mushrooms).
- Do the tasks that cost only gold or nothing, claim the chests. Never anything with mushrooms (beer, paid Wheel of Fortune
  spins, hourglasses, skips, mushroom shop refresh).
- Event tasks (own list, own 3 chests): the same rules.
- Upgrade guild skill: the cheaper of Treasure/Instructor, Instructor on equal price; gold only.
- Shell game (3 shells, ~33 % win): allowed, but gambling is not an income. Smallest bets, when gold is low
  (after shopping / before the Thirst for Adventure), and only when the points are needed – the top chest does not
  require every task. The game allows it from 5 gold, the bet is at most 1/10 of the gold.

- More rules (user 2026-10-07):
  - Attributes for tasks yes, but never so that a better item cannot be bought → only after the shops, keep the shop reserve.
  - Drink beer costs a mushroom. Default: only when it is the last task missing for the max reward (10 mushrooms)
    and no other task can be completed anymore – 1 mushroom for 10 is worth it. During events (later) more may be allowed.
  - Tasks for specific expedition locations (each expedition shows 2 locations): prefer such an expedition even when it
    is longer / costs more Thirst for Adventure. The same for a task requiring a specific expedition type.
  - "Win N fights against <class>": if the XP fights during the day do not do it (or it is late), search the Hall of Fame
    far below our rank (e.g. we are 8600th → look at 10000) for a weak player of that class with low honor and beat them.
    Hall of Fame fights do not count towards the 10 XP wins.
  - "Win bare-handed": take the weapon off, beat a weak player low in the Hall of Fame, put the weapon back on.

## Implementation (src/tasks.rs + shops.rs + dungeons.rs + tavern.rs + arena.rs + hunt.rs)
- sf-api parses `dailytasklist` / `eventtasklist` (`gs.specials.tasks`), chests `CollectDailyQuestReward` /
  `CollectEventTaskReward` (`DailyTaskClaim:1|2/<n>`).
- Every pass of the main loop: log the tasks once a day (`[tasks] daily tasks (chests at a/b/c points): …`), claim chests.
  After the Tavern and the shops: guild skill (`UpgradeAnyGuildSkill`, price + shop reserve ≤ gold), attributes (`Upgrade(X)`,
  `UpgradeAnyAttribute` / `SpendGoldOnUpgrades` → main attribute) via `UpgradeSkill` while gold > reserve + last price.
- Beer (`beer_justified`, also checked in `safe.rs`, watchdog allows exactly 1 mushroom): after the shell game, only when
  earned + natural points < a chest ≤ that + the beer points and the chest has more than 1 mushroom; beers left today.
- Expeditions (`choose_expedition`): an affordable expedition through a `TravelTo(location)` location wins over shorter ones.
  A task for an expedition type is not known in sf-api yet – when it shows up (Unknown in the task log), map it.
- Arena: `WinFightsAgainst(class)` → among the 3 opponents one of that class weaker than us is chosen.
- Hunt (`hunt.rs`): `WinFightsAgainst`, `WinFightsBareHands`, `WinFightsNoChestplate` when the Arena is free and the 10 XP
  wins are done or after 21:00. Hall of Fame page at our rank + 1500 / 3000 / 6000, players of our level or lower
  (and the class), 3 lowest honor inspected via `ViewPlayer`, fight one with strength ≤ 60 % of ours (30 % bare hands,
  50 % without chest plate). Item off via `PlayerItemMove` (equipment → backpack, whitelisted only this way), back via `Equip`.
- Shell game (`DefeatGambler`): after the Tavern is done (shops run just before → gold is low), only when
  earned + "natural" points (Arena, Dungeons, City Guard tasks) < some chest ≤ that + the shell game points.
  Bet 1 silver (`GAMBLE_BET`, verify the minimum), stop below 5 gold. `safe.rs` (`gamble_ok`): bet ≤ 1/10 gold, ≥ 5 g.
- Shops (`task_purchase`): `BuyWeaponInWeaponsShop`, `BuyFromShop(shop)` → buy the cheapest non-epic gold item
  (weapon from the Weapon Shop / any item from that shop) even below the reserve, `inventory::manage` sells it. Max 6 a day.
- Dungeons: `DefeatMonstersLightDungeon(d)` open → fight in that dungeon instead of the usual pick.
- Not doable (mushrooms): `DrinkBeer`, `SpinWheelOfFortune` beyond the free spin, `BuyHourGlasses`, `SkipQuest`, … – ignored.
- Not done yet: `RequestNewGoods` (only the ad, see `docs/shops.md`), `ClaimNewCustomerPack` (`docs/daily-rewards.md`),
  `ThrowItemInToilet`, `AddFriend`, potions, pets, Fortress, Underworld, … (later).

## TestChar1 tasks on 2026-10-07 (from a captured response)
LeaseMount 1/1 (1 p), Upgrade STR 0/5 (3), Upgrade CON 0/5 (2), BuyWeaponInWeaponsShop 0/1 (3), SpinWheelOfFortune 1/5 (2),
WinFightsInArena 5/10 (2), DefeatGambler 0/3 (1), DrinkBeer 0/1 (2), RequestNewGoods 1/1 (1), JoinOrCreateGuild 1/1 (2),
UpgradeAnyGuildSkill 1/1 (1), DefeatMonstersLightDungeon(TrainingCamp) 8/10 (3), ClaimNewCustomerPack 1/1 (1).
Event: SpendGoldOnUpgrades 0/200, BuyHourGlasses 0/3, BuyFromShop(Weapon) 2/3, BuyFromShop(Magic) 0/3 (1 p each).

## Verification status
| What | Status |
|---|---|
| Task list refreshed by `Poll` (progress visible during the day) | ⏳ not verified |
| Chest claim (`DailyTaskClaim`) | ⏳ not verified |
| `UpgradeSkill` buys an attribute for gold, task counter rises | ⏳ not verified |
| `GuildIncreaseSkill` for gold, task counter rises | ⏳ not verified |
| Shell game: minimal bet 1 silver accepted? result in `gamble_result` | ⏳ not verified |
| Shop task purchase counts for `BuyWeaponInWeaponsShop` / `BuyFromShop` | ⏳ not verified |
| `SpendGoldOnUpgrades` unit (silver or gold?) and whether attributes count | ⏳ not verified |
| Beer for a task: 1 mushroom, chest opens | ⏳ not verified (only when it happens) |
| Expedition through a task location counts for `TravelTo` | ⏳ not verified |
| Hall of Fame page around a rank (`HallOfFamePage`), fight by name, Arena cooldown after it | ⏳ not verified |
| Hall of Fame fights do not count towards the XP wins | ✅ 2026-10-07 per the user |
| Bare hands: `PlayerItemMove` equipment → backpack, fight, `Equip` back | ⏳ not verified |
