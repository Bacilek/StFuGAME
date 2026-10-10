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
  - Attribute cost is fixed (depends only on how many times that attribute was ever bought with gold, never on
    character level or the attribute's own value, user 2026-10-09) and we keep earning more gold over time, so
    attributes effectively get *cheaper relative to our income* the longer we wait – no reason to spend on them
    without a task forcing it. Only when gold is genuinely surplus (spinning the shop otherwise just burns it on
    the buy/sell spread) does the bot spend on attributes with no open task, see below.
  - Drink beer costs a mushroom. Default: only when it is the last task missing for the max reward (10 mushrooms)
    and no other task can be completed anymore – 1 mushroom for 10 is worth it. During events (later) more may be allowed.
  - Tasks for specific expedition locations (each expedition shows 2 locations): prefer such an expedition even when it
    is longer / costs more Thirst for Adventure. The same for a task requiring a specific expedition type.
  - "Win N fights against <class>": if the XP fights during the day do not do it (or it is late), search the Hall of Fame
    far below our rank (e.g. we are 8600th → look at 10000) for a weak player of that class with low honor and beat them.
    Hall of Fame fights do not count towards the 10 XP wins.
  - "Win bare-handed": take the weapon off, beat a weak player low in the Hall of Fame, put the weapon back on.
    An Assassin has a weapon in both the Weapon and Shield slot (dual-wield) – both come off, both go back on
    (`src/hunt.rs`, 2026-10-08, ahead of Sanek's first run).
  - "Spin the Wheel of Fortune 5×": 1 free, the rest costs lucky coins (collected from offers, ads, …). Do not waste them,
    use them only when a reward would not be reached otherwise.
  - "Drink beer" may ask for 10 beers (10 mushrooms). Only when it brings more (e.g. 10 mushrooms back + the Thirst for
    Adventure), otherwise be careful, 10 mushrooms is a lot.
  - Whether a costly task is worth it depends on what the chests actually contain → look at the chest rewards.
  - Attribute tasks that help to a better chest go before spinning the shops (still after equipment upgrades).

## Implementation (src/tasks.rs + shops.rs + dungeons.rs + tavern.rs + arena.rs + hunt.rs)
- sf-api parses `dailytasklist` / `eventtasklist` (`gs.specials.tasks`), chests `CollectDailyQuestReward` /
  `CollectEventTaskReward` (`DailyTaskClaim:1|2/<n>`).
- Every pass of the main loop: log the tasks once a day (`[tasks] daily tasks (chests at a/b/c points): …`), claim chests.
  After the Tavern and the shops: guild skill (`UpgradeAnyGuildSkill`, price + shop reserve ≤ gold), attributes
  (`Upgrade(X)` → that attribute; `UpgradeAnyAttribute` / `SpendGoldOnUpgrades` → whichever attribute is currently
  the best weight-per-gold, `attributes::best_attribute`) via `UpgradeSkill` while gold > reserve + last price.
- **Attribute cost & choice (`src/attributes.rs`, user 2026-10-09):** the price of the next gold-bought point for
  an attribute depends only on how many times *that* attribute has ever been bought with gold (the "attribute
  increasement level"), never on its current value or the character's level. The server does not expose this
  lifetime counter, so the bot tracks it itself per character (`roster/<nick>/attribute_levels.json`, local only),
  advancing it after every purchase and self-correcting (`[check] MISMATCH`, searches ±50 levels) if the price
  actually paid does not match what we expected (e.g. a manual purchase outside the bot). Prices come from a
  lookup table (`COST_TABLE`, levels 1..=216, source: sf.kalais.net/english/attributes.html, 1 gold = 100 silver) –
  truncated there because the site's own table turns unreliable higher up (blank/`"?"` cells); beyond level 216
  the price is extrapolated with the table's last step, **not yet verified live** (no character has gotten an
  attribute that high yet). `attributes::best_attribute` picks whichever of the 5 attributes gives the best
  weight-per-gold, weight = the same as the Arena power formula (`arena::weight`: main 100 %, CON 80 %,
  LCK 40 %, the other two side attributes 10 % each) divided by the next purchase's price.
- **Surplus attribute buying (`tasks::buy_surplus_attributes`, no task needed, user 2026-10-09):** once gold piles
  up past 5× the shop reserve (`SURPLUS_RESERVE_MULTIPLE`), the bot spends the excess on whichever attribute is
  currently the best weight-per-gold, keeping at least that 5× floor. Runs every pass, right after the task-driven
  `buy_attributes`, only when there's no open attribute task to handle it instead. Not yet verified live.
- Expeditions (`choose_expedition`): an affordable expedition through a `TravelTo(location)` location wins over shorter ones.
  A task for an expedition type is not known in sf-api yet – when it shows up (Unknown in the task log), map it.
- Arena: `WinFightsAgainst(class)` → among the 3 opponents one of that class weaker than us is chosen.
- Hunt (`hunt.rs`): `WinFightsAgainst`, `WinFightsBareHands`, `WinFightsNoChestplate`, `WinFightsNoEpicsLegendaries` whenever the Arena
  is free – BEFORE the Arena XP fights (user 2026-10-10: quests first; `main.rs` runs `hunt::run` before `arena::run`). Hall of Fame pages
  from the very BOTTOM upwards (max 6 pages; level 1 players live there, a weaker opponent is always found; user 2026-10-10), players of our level
  or lower (and the class), 3 lowest honor inspected via `ViewPlayer`, fight one with strength ≤ 60 % of ours (30 % bare hands,
  50 % without chest plate). Item off via `PlayerItemMove` (equipment → backpack, whitelisted only this way), back via `Equip`.
- **`WinFightsNoEpicsLegendaries` (hunt, user 2026-10-10, was deferred 2026-10-09):** the user saw TestChar1 miss this easy 4-point task
  while holding 1 epic and having a free backpack slot. `hunt::Hunt::NoEpics`: every equipped epic/legendary item (`is_epic`, >= model 50)
  is moved to the backpack, the Hall of Fame fight is done (opponent power <= 50 % of ours), everything is equipped back. It first
  checks that the backpack has at least as many free slots as items to take off (`count_free_slots`) and otherwise skips
  without touching anything. `WinFightsNoGear` is still not mapped. Not verified live yet.
- Attribute tasks before spinning (`attributes_needed`, called from `shops.rs` before the first spin): when some unopened
  chest is above earned + natural points and within reach with the attribute tasks (+ affordable costly tasks).
  They still keep the shop reserve. Otherwise attributes come after the shops as before.
- Costly tasks (`plan`, separately for the daily and the event list): Wheel of Fortune beyond the free spin
  (`SpinWheelOfFortune`, lucky coins), beer (`DrinkBeer`, mushrooms). Expected = earned + "natural" points –
  **only** Arena and City Guard tasks (`natural_points`), **not** dungeon tasks any more (bug found + fixed live
  2026-10-08: an open `DefeatMonstersLightDungeon`/`FightInDungeons` task used to count its full points as
  "will finish today" too, which could silently convince `plan` a chest was already covered and skip beer even
  while stuck well short of 10/10 – dungeons are capped at 1 real attempt/hour with a real win/loss outcome,
  unlike Arena/City Guard which the bot retries every cooldown with no attempt cap). Of all combinations that we
  can afford (enough lucky coins for all remaining spins
  incl. the free one; enough beers left today) take the one reaching the highest unopened chest above "expected",
  and the cheapest for it (weights per unit: lucky coin 10, beer 100). A chest counts only if it is worth it
  (`chest_worth`): beers → chest mushrooms ≥ beers; lucky coins → chest has mushrooms or ≥ as many lucky coins as
  spent (10 per spin). Run after the Tavern and the shops: lucky-coin spins, then one beer per pass (beer gives
  Thirst for Adventure, the Tavern runs before the next one).
  `safe.rs` re-checks: `lucky_spin_justified`, `beer_justified`; the watchdog allows exactly 1 mushroom per beer.
- **Shell game (`DefeatGambler`) is handled separately from the rest of `plan`** (user 2026-10-08: gold is more
  dispensable than mushrooms/lucky coins, so there's no reason to ever skip a day of it). `planned()` adds
  `Extra::Gamble` whenever the task is open and affordable (`Means::gamble`, ≥ 5 g), regardless of whether any
  chest "needs" it – not gated by `chest_worth`, and `tasks::run` calls it right after `claim_chests`, independent
  of `tavern_done` (any time of day, not just after the Tavern/shops like the other costly tasks). Bet 1 silver
  (`GAMBLE_BET`), stop below 5 g (`GAMBLE_MIN_SILVER`). `safe.rs` re-checks `gamble_ok` (bet ≤ 1/10 gold, ≥ 5 g).
- The task/chest log (`log_tasks`) repeats every 15 min (`TASK_LOG_EVERY`) while anything today is still unclaimed,
  not just once a day (user 2026-10-08: once a day was too stale to tell whether a chest's points are actually
  within reach) – stops once every daily and event chest is `opened`, until tomorrow's reset. Lists every chest
  with its rewards (`[tasks] daily chest 2 (8 points): Mushrooms 2, …`).
- Shops (`task_purchase`): `BuyWeaponInWeaponsShop`, `BuyFromShop(shop)` → buy the cheapest non-epic gold item
  (weapon from the Weapon Shop / any item from that shop) even below the reserve, `inventory::manage` sells it. Max 6 a day.
- Dungeons: `DefeatMonstersLightDungeon(d)` open → fight in that dungeon instead of the usual pick.
- Not doable (mushrooms): `SkipQuest`, … – ignored. `BuyHourGlasses` only when hourglasses are for gold in a shop (`shops.rs`).
  `DrinkPotion(type)` happens when `potions.rs` drinks that type.
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
| Chest claim (`DailyTaskClaim`) | ✅ 2026-10-08 (daily chest 1+2, all 3 event chests claimed and logged as `opened`) |
| `UpgradeSkill` buys an attribute for gold, task counter rises | ✅ 2026-10-07/08 (Upgrade(Strength)/Upgrade(Constitution) both reached 5/5 after buying) |
| `GuildIncreaseSkill` for gold, task counter rises | ⏳ not verified |
| Shell game: minimal bet 1 silver accepted? result in `gamble_result` | ✅ accepted; wins do happen too (2026-10-08: two `SilverChange(+1)` in a row, net `+0.02 g` for the session – the earlier 10-loss streak was just bad luck) |
| Shop task purchase counts for `BuyWeaponInWeaponsShop` / `BuyFromShop` | ⏳ not verified |
| `SpendGoldOnUpgrades` unit (silver or gold?) and whether attributes count | ⏳ not verified |
| Beer for a task: 1 mushroom each, chest opens | ⏳ not verified (only when it happens) |
| Lucky-coin wheel spin costs 10 lucky coins | ✅ 2026-10-07 per the user (an ad at Dr. Abawuwu gives 3 lucky coins) |
| Lucky-coin wheel spin counts for the task | ⏳ not verified |
| Chest rewards parsed correctly (`dailytaskrewardpreview`) | ⏳ check the `[tasks] … chest` log lines against the game |
| Expedition through a task location counts for `TravelTo` | ⏳ not verified |
| Hall of Fame page around a rank (`HallOfFamePage`), fight by name, Arena cooldown after it | ⏳ not verified |
| Hall of Fame fights do not count towards the XP wins | ✅ 2026-10-07 per the user |
| Bare hands: `PlayerItemMove` equipment → backpack, fight, `Equip` back | ⏳ not verified |
| New event theme at the daily reset: task list can lag behind the reward-chest reset | ❌ bug, fixed 2026-10-09 (below) |

## Bug: "invalid chest" spam right after a new event theme starts (found and fixed 2026-10-09)
At the 2026-10-09 midnight reset, all 11 running characters logged `[tasks] event tasks: ` (empty) together with
`event chest 1/2/3 (0 points)`, then `Claiming the event chest 1 (0 points)` → `Error: Server responded with
error: invalid chest`, repeating on every `claim_chests` call afterwards (pure noise, no mushroom risk – chest
claims are free). Root cause: `gs.specials.tasks.event.rewards` (the reward-chest array, parsed from
`eventtaskrewardpreview`) reset to a zeroed/empty-looking state for the new theme before
`gs.specials.tasks.event.tasks` (the actual task list, parsed from a separate `eventtasklist` server message) had
synced – `can_open_chest(0)` then reads `earned_points() (0) >= required_points (0)` as "claimable" even though
there's no real event data yet. Fixed in `claim_chests` (`src/tasks.rs`): only considers an event chest when
`t.event.tasks` is non-empty. `log_tasks` also skips the whole "event" section while the list is empty, so it
doesn't keep printing a misleading "0 points" line every 15 min either. Yesterday's "Epic Shopping Spree" event
(see `docs/todo.md`, weekly server-wide events) had a real, non-empty task list with real point thresholds the
whole time, so this guard only ever suppresses the brief sync-lag window, not legitimate low/zero thresholds.
**Needs a release rebuild + bot restart to take effect** – not yet verified live (the running bot still has the
old behaviour until restarted).
