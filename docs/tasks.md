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

## Implementation (src/tasks.rs + shops.rs + dungeons.rs)
- sf-api parses `dailytasklist` / `eventtasklist` (`gs.specials.tasks`), chests `CollectDailyQuestReward` /
  `CollectEventTaskReward` (`DailyTaskClaim:1|2/<n>`).
- Every pass of the main loop: log the tasks once a day (`[tasks] daily tasks (chests at a/b/c points): …`),
  claim chests, guild skill (`UpgradeAnyGuildSkill`), attributes (`Upgrade(X)`, `UpgradeAnyAttribute` /
  `SpendGoldOnUpgrades` → main attribute) via `UpgradeSkill`.
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
