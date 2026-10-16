# Fortress

Unlocked at character level 25 (user 2026-10-11). Nothing is automated yet – this file holds the user's rules, what
`sf-api` offers and the first captured response. Implementation only after the open questions below are answered.

## Rules (user 2026-10-11)
- **Never upgrade the fortifications** (`FortressBuildingType::Wall`): the weaker our defence, the more players attack
  us, and the more we can counter-attack – counter-attacks are the main source of resources.
- **Gem Mine only from level 125** (my reading: CHARACTER level 125; fortress buildings top out far below 125 – to be
  confirmed): a low Gem Mine level means a lower cooldown for finding a gem, and at lower levels the equipment (and with
  it the gems) is swapped quickly anyway.
- **Focus the upgrades on the Fortress itself (main building) and, together with it, the Laborers' Quarters** (they
  shorten the build/upgrade times). A building cannot go above the Fortress level (`Fortress::can_build`).
- NEVER cancel a build and never speed up a construction/upgrade with mushrooms or hourglasses (user 2026-10-11). Only
  `FortressBuildFinish { mushrooms: 0 }` is whitelisted; `FortressBuildCancel` and every command with mushrooms stay out
  (unit test `safe::fortress_tests`).

## First capture (user, the very first Fortress upgrade, 15 min)
```
timestamp:1791723040&resources:23994/35/19271/263/271/10/0/7/0/2575/1711/0/0/0/0/0/0/0
&fortressstorage:0/0/0/0/0/0/0/0/0/0/0/1791723040/900/300/0/0
&fortress:0/0/0/0/0/0/0/0/0/0/0/0/1/1791723940/1791723040/0/100/0/1791722920/0/0/0/0/0/0/0/0
```
All building levels are still 0 (the Fortress is under construction); `1` = the building being built (Fortress = 0 + 1),
finish `…3940` − start `…3040` = 900 s = 15 min. The `resources` silver field is 19271 = 192.71 g after paying.

## What `sf-api` offers (v0.4.4, `src/gamestate/fortress.rs`, `command.rs`)
- State `gs.fortress`: `buildings` (level + `upgrade_cost` {time, wood, stone, silver}), `building_upgrade` (running
  build: target, start, finish), `resources` (wood/stone/experience: current, limit, production per hour,
  collectable), `units`, `gem_search`, `attack_target`, …; `Fortress::can_build(building, silver)` checks level, running
  build, resources.
- Commands: `FortressBuild { f_type }` (wood + stone + silver, no mushrooms), `FortressGather { resource }` (collect
  production), `FortressGatherSecretStorage`, `FortressBuildCancel`, `FortressBuildFinish { f_type, mushrooms }` (**never**),
  `FortressBuildUnit`, `FortressUpgradeUnit`, `FortressUpgradeHallOfKnights`, `FortressGemStoneSearch(+Cancel/Finish)`,
  `FortressAttack`, `FortressNewEnemy`, `FortressChangeEnemy`, `FortressSetCAEnemy`, `HallOfFameFortressPage`.
- Minimum Fortress level per building: Laborers' Quarters, Quarry, Smithy, Woodcutter's Hut 1; Treasury 2; Gem Mine 3;
  Barracks, Wall 4; Archery Guild 5; Academy 6; Mages' Tower 7.

## Implementation (src/fortress.rs)
- `fortress::run` (main loop after the shops, before the Gleeman tasks; also in the shop pass before spinning and in the late spin round): (1) finish a build whose timer ran out (`FortressBuildFinish` with `mushrooms: 0` – the game wants it after every build, not only when skipping), (2) `FortressGather` for Wood/Stone/Experience when at least half of the production limit is collectable and there is room, (3) start the next build when `Fortress::can_build` says yes with gold ABOVE the shop reserve and the day's shop pass is done.
- (SUPERSEDED by the plan list below) Order: Fortress → Woodcutter's Hut level 1 (second upgrade, user capture 2026-10-11) → Quarry level 1 (stone; user 2026-10-11: he built it by hand as the third upgrade, waiting for wood) → then Fortress and Laborers' Quarters together (the lower level first, Fortress on a tie). ONLY these four buildings (Fortress, Woodcutter's Hut, Quarry, Laborers' Quarters; also enforced in `safe.rs`); no Wall, no Gem Mine yet (character level 125, not implemented), nothing else until the user answers question 1.
- Gold priority (user 2026-10-11): better items > fortress > spinning the shops > attributes.
- Not handled yet: counter-attacks, units, the unlock of the Fortress feature itself for characters hitting level 25 (an `UnlockFeature` ident is probably pending like the dungeons' – unknown), waking up exactly when a build finishes (the main loop's ≤ 30 min cadence applies).

## Second capture (Woodcutter's Hut level 1 started, user 2026-10-11)
`fortress:1/0/0/…/3/1791727864/1791726964/10/110/9060/…` – Fortress level 1, build target `3` = Woodcutter's Hut (3 − 1 = 2), duration 900 s again, silver 19271 → 19071 (2 g).

## Answers (2026-10-11)
2. Gold: better items > fortress > spinning > attributes. 3. Gem Mine from CHARACTER level 125 – yes. 4. Start on every character that reaches level 25 (TestChar1 will be first).

## Open questions (asked 2026-10-11, answers go to `docs/precedents.md`)
1. Wood and stone come from the Woodcutter's Hut and the Quarry – may the Quarry and other buildings (Treasury, Academy, units, Smithy, Hall of Knights) be upgraded too? The user will answer later.

## Price table (user capture 2026-10-11, after the Woodcutter's Hut was built)
`fortressprice` = 4 numbers per building in `FortressBuildingType` order: build time (s), silver (÷100 = gold), wood, stone, for the NEXT level. At Fortress level 1 / Woodcutter 1:
| Building | time | gold | wood | stone |
|---|---|---|---|---|
| Fortress (→2) | 1895 s | 20 | 150 | 50 |
| Laborers' Quarters | 900 s | 5 | 35 | 12 |
| Woodcutter's Hut | 1895 s | 4 | 30 | 20 |
| Quarry | 900 s | 3 | 22 | 0 |
| Gem Mine | 900 s | 15 | 50 | 17 |
| Academy | 900 s | 7 | 7 | 9 |
| Archery Guild | 900 s | 5 | 41 | 7 |
| Barracks | 900 s | 4 | 20 | 14 |
| Mages' Tower | 900 s | 6 | 61 | 20 |
| Treasury | 900 s | 25 | 40 | 13 |
| Smithy | 900 s | 4 | 25 | 8 |
| Wall | 900 s | 150 | 30 | 13 |
Storage after the Woodcutter: wood limit 900, stone limit 300, wood production limit 375, `woodperhournextlevel:220`, `stoneperhournextlevel:50` (no Quarry yet, so no stone production). Gold costs are tiny at these levels; wood/stone are the bottleneck.

## Tips from the internet (pasted by the user 2026-10-11, NOT decided yet)
- Fortress, Laborers' Quarters and Hall of Knights to level 6; Barracks 3–4; Gem Mine to 1 and keep it there until about character level 125 and dungeon 9 finished; Quarry/Woodcutter to 3–4.
- Farm resources with 1 soldier always, do not use gold to reshuffle the opponents, ALWAYS counter-attack (use a "fortress buddy" – the user's characters could attack each other).
- Do not build defences (Wall) – a waste. Archers and Mages are defence only; level 1 of each only to unlock the Smithy (needed to upgrade soldiers) – optional, especially with a buddy.

## Third and fourth capture (user 2026-10-11)
Quarry started: `fortress:1/0/1/0/…/4/1791729001/1791728101/…` (target 4 = Quarry, 900 s). The Woodcutter's collect (`FortressGather`) answered with `fortressstorage:0/0/0/375/…` (collectable back to 0) and `resources:…/17/0/13/…`: the starting stock is tiny (17 wood, 13 stone), so the bot now also gathers when the collectable amount covers what the next build still misses (not only at half of the production limit).

## Fifth/sixth/seventh capture (user 2026-10-11)
Quarry finished (`fortress:1/0/1/1/…`, `stoneperhournextlevel:70`), the Laborers' Quarters level 1 started right after (target 2, 900 s) – exactly the order the bot uses – and the first stone was collected from the Quarry (stone 13 → 14, the Quarry had just been built). The Quarry level 2 costs 6 gold + 90 wood + 16 stone, 1895 s.

## Build plan (user 2026-10-11, `PLAN` in `src/fortress.rs`)
The user dictates the sequence; the bot builds the first entry below its target level and WAITS for it (saving the wood/stone for it) instead of skipping ahead: Fortress 1, Woodcutter's Hut 1, Quarry 1, Laborers' Quarters 1 (all done by hand), then **Fortress 2, Laborers' Quarters 2, Woodcutter's Hut 2, Quarry 2, Treasury 1**. After that the bot builds nothing until the user dictates more. Allowed in `safe.rs`: exactly the buildings of the plan (Fortress, Laborers' Quarters, Woodcutter's Hut, Quarry, Treasury). Capture after the Laborers' Quarters level 1: `fortress:1/1/1/1/…`; the build times fell from 1895 s to 1800 s (Laborers' Quarters effect) and the Laborers' Quarters level 2 costs 10 gold + 138 wood + 46 stone. Extended 2026-10-11 (user): after Laborers' Quarters 2 comes Fortress 3, Woodcutter 3, Quarry 3, Gem Mine 1, Treasury 2, Laborers' Quarters 3.

**Reserve (user 2026-10-11): the shop reserve does NOT apply to the fortress** – a build may use all the gold (it still comes after the day's shop pass, so better items are bought first).

The login response carries `maxupgradelevel:20` (buildings go up to level 20) and the whole `fortressprice` table, so building prices are known right after login (TestChar1, level 22, 2026-10-11, had no fortress yet).

## Plan change (user 2026-10-11)
New order: Fortress 2, **Quarry 2, Woodcutter's Hut 2, then Laborers' Quarters 2**, Treasury 1. Reason: the production buildings are cheaper and raise the income, while the Laborers' Quarters only shorten the build times – pointless while we wait for wood/stone anyway. General principle for every future plan entry: income buildings first, time savers after. (`PLAN` in `src/fortress.rs`.)

## Generic rules after the dictated plan (user 2026-10-11, in case he runs out of time to dictate)
`PLAN` (dictated, in `src/fortress.rs`) is followed first: Fortress 2, Quarry 2, Woodcutter's Hut 2, Treasury 1, Laborers' Quarters 2. Afterwards `pick_generic` continues alone: the goal is the next Fortress level, but before every new Fortress level (current level n) the supporting buildings must be at level n, in this order: 1. income buildings (Woodcutter's Hut, Quarry; the one with the lower wood+stone cost first), 2. Treasury (level 1 from Fortress 2; more levels when the next Fortress level's wood/stone cost exceeds the storage limit – it could never be afforded), 3. Laborers' Quarters (time savers after income, but always before the new Fortress level), 4. Gem Mine at level 1 once the Fortress is level 3 (more levels only from character level 125; `fortress::build_ok` + `safe.rs`), 5. the next Fortress level. A building never exceeds the Fortress level; the Wall is never built. The bot always waits for the chosen building (resources are collected for it). Costs are weighed in the cheaper-first choice and in the storage check. NOT covered yet: Hall of Knights (`FortressUpgradeHallOfKnights`, the user wants it considered – needs a capture of what it costs/how it behaves before automating), units, Smithy, counter-attacks. When the user dictates his own order on his character, that order replaces the plan for the other characters too.

## Plan change: Fortress 4 + Barracks early (user 2026-10-11)
`PLAN` order after Laborers' Quarters 2: Fortress 3, Woodcutter 3, Quarry 3, **Fortress 4, Barracks 1**, Gem Mine 1, Treasury 2, Laborers' Quarters 3. Reason: the Barracks (min. Fortress level 4) are very cheap and fast and bring much more resources, so after Quarry 3 nothing else (Treasury, Gem Mine, Laborers' Quarters, no filler for short waits) is built – the bot saves for Fortress 4. Barracks is allowed in `safe.rs`/`build_ok` (the plan builds only level 1; units are still not automated). Watch: if Fortress 4 exceeds the wood/stone storage limit, the plan would stall – then the Treasury is needed (tell the user).

## Capture 2026-10-12 (user, Bacilek31 attacking by hand): attack, soldier count, training
Flow in the client: the attack target is looked at with `PlayerLookAt` (`otherplayer…` keys of the target "Icri", id 20386, level 31, guild Targaryen, `otherplayerfortressrank:7355`, `otherplayerfortressinfo:50/0/0/0/407/132`, `otherplayerunitlevel(4):5/25/25/25/`); choosing the soldier count (1) sends the same `PlayerLookAt` again (no attack yet); `FortressAttack:<soldiers>` (sf-api `Command::FortressAttack { soldiers }`) is the actual attack.
- Response of the attack: `fightresult.fortresspillagerv1:1/1/0/0/0/1/0/<rank before>/<rank after>/0…/0/<stone>/<wood>/1/1/0/0` – CORRECTION by the user the same day: the 7263/7162 are NOT loot but our fortress Hall of Fame rank before/after (rank 7161 in the list); the loot is the pair near the end, order STONE/WOOD (196/632 = 632 wood, 196 stone; the first attack 132/407). That pair equals the target.s `otherplayerfortressinfo` shown BEFORE the attack (`50/0/0/0/407/132` = upgrades, fortifications, archers, mages, lootable wood, lootable stone), so the loot is predictable. sf-api reads it all: `last_fight.has_player_won`, `honor_change` (= 1 fortress honor per win), `OtherPlayer.fortress.lootable_wood/lootable_stone`. Further: a normal `fight1.r` with `winnerid1.s:710` (our soldier), `resources:`, `fortressstorage:`, `fortressunits:`, `fortress:…/<attack target id>` (the offered target is still 20386 after both attacks – whether it rotates by itself is unknown). The state is fully refreshed by the response.
- Training one soldier (a separate request, only `timestamp`+`resources`+`fortressstorage`+`fortressunits` in the response): `fortressunits` changes `0/0/0/2/0/0/…` → `0/0/0/3/0/0/…` and the resources drop by 15 / 5 on the wood and stone fields (1188→1173, 323→318) – soldier price looks like 15 wood + 5 stone. Command name not captured (sf-api: `FortressBuildUnit { unit, count }` = `FortressBuildUnitStart:<unit+1>/<count>`).
- NOT implemented. The open questions before automating (asked the user): which target (`FortressNewEnemy` rerolls cost gold/mushrooms – the rule says never), how many soldiers (docs say 1 always), whether to train soldiers automatically (cost, `FortressBuildUnit`), counter-attacks via `FortressSetCAEnemy`, and the whitelist entries in `safe.rs`.

## Soldiers and attacks – implemented (2026-10-12, user decision) – `fortress::military`
User: train melee soldiers automatically and ALWAYS up to the maximum (cheap), attack with ONE soldier at a time, prefer the opponent the fortress itself offers; if one soldier would not do or the offered opponent has nothing to loot, use counter-attacks.
- Every 5 min per character with Barracks (`units[Soldier].limit > 0`): `Update`, then up to 10 attacks: offered target (`attack_target`) → `ViewPlayer{id}` → `lootable_wood + lootable_stone ≥ 2 × the price of a soldier` (`LOOT_PER_COST`, user 2026-10-12: measure the loot against the soldier cost; fallback price 20) → `FortressAttack{soldiers:1}`. Otherwise (too little loot) or after a LOST one-soldier attack: counter-attack targets from the combat log (`FortressDefense` entries, OLDEST first by user 2026-10-12, up to 3 per pass): `FortressSetCAEnemy{msg_id}` → look → attack. Never the same player twice in one pass. No soldiers left or no worthwhile target → stop. After the attacks: `FortressBuildUnit{Soldier, n}` with n = missing up to the limit, limited by wood/stone (`units[Soldier].training.cost`), only when nothing is in training.
- `safe.rs`: only `FortressBuildUnit{Soldier}`, `FortressAttack{soldiers:1}`, `FortressSetCAEnemy` (tested); `FortressNewEnemy` (reroll, gold/mushroom) stays forbidden. None of them takes mushrooms.
- Costs (captures): one soldier ~15 wood + 5 stone, ~10 min training; a won attack gave 632 wood + 196 stone + 1 honor, i.e. far more than the soldier costs. Training competes with the build plan for wood/stone but pays for itself.
- ⏳ Not run live. Open: does an attack use up the soldier (the count looked consumed), does the offered target rotate after an attack, is there a cooldown or daily attack limit, what happens at the Barracks cap, is `training.cost` per unit (assumed). Watch the `[fortress]` lines (`Attacking`, `Won against`, `Lost against`, `Training`). Also captured: `FortressGetHoF` = fortress Hall of Fame page (`Ranklistfortress.r:rank,name,guild,level,honor;…`), and `FortressBuildUnitStart` has only a `timestamp`+`resources`+`fortressstorage`+`fortressunits` response.
Confirmed by the user 2026-10-12: an attack ALWAYS uses up the soldier (even a won one) and `training.cost` is the price of ONE soldier. So every attack must be worth more than one soldier, hence the loot-vs-cost rule; open questions about consumption/cost are closed.

## Capture 2026-10-12 (user, Bacilek31): gem from the Gem Mine and socketing it
- `PlayerItemMove` from the necklace slot (equipment) to the backpack: ordinary response (`ownplayersavecharacter`, `backpack`, `ownplayersaveequipment`, …) – the same command the bot already uses for taking items off (allowed only equipment → backpack, `safe.rs`). Done first to free the necklace (the gem is put into a necklace with a gem slot).
- `FortressGemstoneFinished` (collecting the mined gem; sf-api `FortressGemStoneSearchFinish { mushrooms }`, here without mushrooms): response `gemstonebackpackslot:1` (the gem went to backpack slot 1), new `backpack:`, `fortressstorage:`, `fortress:` (…/20386 at the end again) and `achievement(284)`.
- `PlayerItemMove` of the gem onto the necklace (backpack → equipment necklace slot, which has a gem slot): the response shows the necklace in `ownplayersaveequipment` with a gem entry (`…/8/33/0/5/0/0/0/4/…/66/26/…` – the socket values `33`/`66` look like the gem type and its power; attribute totals went up, e.g. `ownequipmentaveragelevel` 20 → 22) and the gem gone from the backpack. So socketing = a plain `PlayerItemMove` backpack → equipment (not a separate gem command).
- NOT automated. The Gem Mine is limited to level 1 until character level 125 (rules above); `FortressGemStoneSearchFinish` is allowed in `safe.rs` only if it has `mushrooms: 0`? (currently NOT whitelisted at all, a test pins `mushrooms: 1` as forbidden). Open (asked the user): automate searching/collecting gems, where to socket (which item, which gem type fits a class), and whether the mining start (`FortressGemStoneSearchStart`) is wanted.

## Gem Mine automation – implemented (2026-10-12, user decision) – `src/gems.rs`
User rules: (1) collect a mined gem whenever at least ONE backpack slot stays free afterwards (e.g. 6 slots, 4 potions → we can take one gem and still have one free), or whenever a socket holds a SECONDARY gem (replace) or a LUCK gem (replace); (2) place by validity: main-attribute / orange / black gems into the best equipment that will last longest, then Constitution, then Luck, then secondary; (3) a gem with no place is kept in the backpack if it does not get in the way.
Implementation: every 10 min per character with a Gem Mine (`gems::run`): socket a gem lying in the backpack → collect a finished search (`FortressGemStoneSearchFinish{mushrooms:0}`) if ≥ 2 slots are free, or 1 slot is free and the gem (type known beforehand from `gem_search.target`) can be socketed right away → start the next search (`FortressGemStoneSearch`) when idle, after the day's shop pass, only if wood/stone/gold stay above the next planned building (`fortress::next_build_cost`).
- (2026-10-16) Tier 3 (secondary) gems are sold on sight and never socketed. Tiers per class (`gems::tier`): 0 = main attribute / All / Legendary (orange/black assumed = All/Legendary, user wording), 1 = Constitution, 2 = Luck, 3 = secondary. A filled socket is replaced only when it holds tier 2–3 AND the new gem is strictly better (Constitution and main gems are never replaced). Among candidate items: epic first, then `inventory::value`.
- `safe.rs`: `FortressGemStoneSearch`, `FortressGemStoneSearchFinish{mushrooms:0}` and a `PlayerItemMove` backpack → equipment only for a `Gem` onto an equipped item with a gem slot (`gem_socket_ok`); tests added.
- ⏳ Unverified: (a) the exact request line of the socketing move (the user sent only responses; implemented as backpack position → equipment slot; if the gem is still in the backpack afterwards the bot logs it and stops socketing for the day); (b) replacing a filled gem by moving a new one onto it (the user believes the game allows it); (c) the server never clears `gem_search.finish` (assumed; the collected gem is remembered in memory AND in `roster/<character>/gem_collected.txt`, so a restart does not collect it again – user 2026-10-12: "try to handle it"); (d) the cost of a gem search (`gem_search.cost`) and that starting needs no mushrooms. Watch the `[gems]` log lines. Accepted game mechanic (user 2026-10-12): a socketed gem cannot be taken out, nothing to handle when an item with a gem gets replaced.

## Facts after the first night of soldier attacks (2026-10-12, user, Bacilek31)
- Since attacking works, wood/stone are no longer a bottleneck at all: storage cap 6k wood / 2k stone, the user holds 19k wood / 7k stone – **loot can exceed the storage cap**. The only bottleneck left is the BUILD TIME. The "1 soldier → 1 attack" tactic is extremely strong.
- Consequences: never stop attacking because the storage is full (`fortress::military` has no cap check – keep it that way); from Fortress 4 / Barracks 1 the production buildings (Woodcutter's Hut, Quarry) look obsolete – both are full and cannot even be collected while over the cap (the collect step in `fortress::run` correctly skips a full storage, `room` check). The generic planner (`pick_generic`) still treats Woodcutter/Quarry (and the Treasury for storage) as prerequisites of every new Fortress level – that wastes build time now. ⏳ Decision pending with the user, see the proposal in the session of 2026-10-12: drop those prerequisites, prefer time savers (Laborers' Quarters) and soldier capacity (Barracks levels).

## New build rules (2026-10-12, user agreed): time is the only bottleneck
- `PLAN` now ends with Fortress 5, Fortress 6, Academy 1 (rush Fortress 6: the Academy gives XP). The Hall of Knights (HoK) is NOT automated: the user will report whether it has to wait for something and where it ranks; he is building Fortress 5 right now.
- `pick_generic` (after the plan): no Woodcutter/Quarry prerequisites any more. Before every new Fortress level n: Laborers' Quarters up to n (time saver), Barracks up to `BARRACKS_TARGET` 4 (soldier capacity), Treasury up to half of n rounded up (user: "at least a little" – it adds inventory slots for gems, epics, bare-hands/no-epic quests, potions), Gem Mine 1 from Fortress 3, Academy 1 from Fortress 6, then Fortress n+1. Academy is allowed in `build_ok` and `safe.rs`; Smithy and the rest stay forbidden. Tests: `generic_rules_after_the_plan`.

## Free change of the attack opponent (2026-10-12, user)
Changing the opponent (`FortressNewEnemy`, response = `fortress:…` with a new target id at the end) is free once per ~5 minutes (`attack_free_reroll`), otherwise it costs gold – "only when it is free!". Now allowed: `Command::FortressNewEnemy { use_mushroom: false }` in `safe.rs`, guarded by `safe::reroll_is_free` (server time passed AND newer than our last change, `Cooldown::Reroll`); the mushroom version stays forbidden (test). `fortress::military` order: offered target → counter-attacks (oldest first) → ONE free change per pass → look again → otherwise the soldiers wait.

## Hall of Knights (2026-10-12, user): instant, but only while nothing else is being built
User fact: the Hall of Knights (HoK) cannot be started while another building is under construction, but once chosen it is INSTANT (no waiting); with the resources he saved from fortress battles he took it to level 5 at once. Implemented: `fortress::run` upgrades it (`FortressUpgradeHallOfKnights`) whenever no build is running (`building_upgrade.target` is None), the price (`hall_of_knights_upgrade_price`: wood, stone, gold – no mushrooms are shown in the price struct) is affordable, and the HoK level stays below the Fortress level; it goes BEFORE the next build is chosen (after the day's shop pass, like builds). `safe.rs` allows the command only through the same check (`fortress::hok_affordable`). ⏳ Unverified live: that the upgrade really costs no mushrooms (the mushroom watchdog would stop the bot if it did), the real price at higher levels.

## Soldier advice (2026-10-12, user capture)
`PlayerLookAt` of a fortress target carries `soldieradvice:N` (sf-api `OtherFortress.soldier_advice`, "the amount of soldiers suggested when attacking"): the first captured target (Icri) advised 1 and one soldier won; a level 34 target (Myxt, `otherplayerfortressinfo:73/1/4/0/750/817` = 750 wood / 817 stone lootable) advised 7. Choosing a soldier count in the client sends only another `PlayerLookAt` (no attack yet). Rule now in `fortress::military`: attack with one soldier only when `soldier_advice <= 1` (and the loot is ≥ 2 × the soldier price); otherwise one soldier is not enough → counter-attack targets, then the free change of the opponent. ⏳ Unverified how reliable the advice is as a win predictor.

## Changes 2026-10-14 (user backlog, package A+B) – `src/fortress.rs`, needs a release rebuild and a live check
- **Plan:** after Fortress 6 the Academy is rushed to level 6 (`PLAN` ends `Fortress 5, Fortress 6, Academy 6`); `pick_generic` puts the Academy (up to the Fortress level, from Fortress 6) right after the Laborers' Quarters, ahead of Barracks and Treasury ("a little" Treasury = half the Fortress level, unchanged).
- **Collecting no longer stops while a build runs** (before, `run` returned as soon as a build was running – no gathering, no HoK). Overflow rule: a resource is collected when half of its production limit is ready OR it would be full within 45 min OR it covers what the next build misses; the main loop wakes up 40 min before the production fills up (`fortress::secs_until_due`, also build finish and the soldiers' next look). XP matters most, wood/stone only collect while the storage has room (loot can exceed the cap).
- **Hall of Knights** (user correction): can be upgraded while another building is being built; we do not try while the FORTRESS itself is under construction ("probably not", unverified – if the game accepts it, drop the `target != Some(Fortress)` check in `hok_affordable`). It runs after the day's shop pass, before the next build.
- **Barracks vs. training:** soldiers in training block a Barracks upgrade (`Fortress::in_use`). When the Barracks is the next planned build and affordable (shop pass done), training is limited to as many soldiers as finish before the running build ends (`barracks_due`; one soldier = `training.cost.time`, sequential assumption; `[check] Training … finishes at` logs the real finish to verify). Not restricted while the Barracks is not affordable (raids bring the resources, stopping them would deadlock).
- **Sacrifice:** soldiers full (`count + in_training >= limit`), no worthwhile target, the free change used or not available → ONE soldier attacks the offered opponent (normally lost) to make the game offer a new one; trained again afterwards. Once per pass.
- **Attack-only mode** for the user's main character: `SF_ATTACK_ONLY=Name` in `.env` (`;`-separated); `main::play_attack_only` runs only `fortress::run_attack_only` (Academy XP collect + soldiers: training, raids, free change, sacrifice) – no builds, no Arena, nothing else. On/off in the app window like any character (the user wants it overnight only). The account itself still has to be in `SF_ACCOUNTS`.
- **Fortress tab** in the app window (`roster/<character>/fortress.json` written by `fortress::write_snapshot`, read by `control::status_json`): per character building levels, Hall of Knights, resources (current/limit + uncollected, ⚠ when ≥ 80 % of the production limit), soldiers, gem search, running build with countdown, next build + what it waits for, the next 4 planned builds, snapshot time.
- Open: when to start a Gem Mine search (a gem costs 150 g, 50 min) – proposal pending with the user; the HoK-during-Fortress-build question above.

## Gem Mine rule + fortress gold (2026-10-14, user answers) – `src/gems.rs`, `fortress::gold_needed`
- A gem search starts when the mine is idle (after the day's shop pass) and the gold LEFT afterwards is at least 2× the shop reserve plus the gold of the fortress' next planned build. Wood/stone are ignored (they cost next to nothing).
- (2026-10-16, user: do not hoard) Gems are socketed at once, strongest first, one per socket, in a loop (`socket_backpack_gem`); a gem is only held when no socket wants it. Older rule: up to 1 gem is held in the backpack for a future item with a socket (2 with 6+ free slots, `HOLD_TWO_FREE`); a finished gem is collected when it can be socketed right now or when fewer gems than that are held and ≥ 2 slots are free. SUPERSEDED the same day: gems are held without a count limit while one slot stays free, see `docs/precedents.md` "Gem holding".
- Placement: Constitution and main-attribute (tier 0–1) gems go to the best, hardest-to-replace item (epic, then value); Luck and secondary gems to the item we will replace soonest (non-epic, lowest value). Test `weak_gems_go_to_the_item_replaced_soonest`.
- Fortress gold is not a one-off spend (the bot returns to it many times a day until builds take days): spinning the shops, guild upgrades without a task, surplus attribute buying and gem searches all leave `fortress::gold_needed` (the next planned build's gold) on top of their own reserve. Gold order stays: better items > fortress > spinning > guild upgrades > attributes.
- `fortress::diagnose`: one `[fortress] State: …` line per login (or "No fortress data … level N"), added because TestChar1 never logged anything about its fortress although the user saw a Barracks there – check this line after the next start.

## Capture 2026-10-14 (user, counter-attack target "Burbone" from the mail)
`PlayerLookAt` of a counter-attack target (fortress rank 4032, level 34, `otherplayerfortressinfo:108/2/8/1/551/175` = 551 wood / 175 stone lootable): `soldieradvice:8001` – NOT a plain soldier count like the earlier 1 and 7 (maybe 8 soldiers + a flag/1000s?). The user attacked with several soldiers (`fightadditionalplayers.r` lists 7 extra fighters `-730_1_8…-730_7_8`, i.e. 8 soldiers); the fight response carries `fightresult.fortresspillagerv1:0/1/0/0/0/0/0/4101/4101/…` (rank unchanged 4101 = nothing gained) and `fortressunits:6/0/0/2/0/0/…` afterwards. Our rule (`soldier_advice <= 1`, one soldier) skips such a target, so nothing to change; open: decode 8001 if counters turn out to be mostly unusable. Counter-attack targets come from the mail (`gs.mail.combat_log`, `FortressDefense` entries) – the bot already reads the mail.

## Raids with 1–2 soldiers (2026-10-14, user)
User: spending 1–2 soldiers is fine when it pays off in resources. `soldiers_for` (`fortress.rs`): the game's `soldier_advice` (the soldier count it suggests; 0 counts as 1) up to `MAX_RAID_SOLDIERS = 2`, only when we have that many and the lootable wood + stone is at least `LOOT_PER_COST` × the soldier price × the number of soldiers sent. Advice above 2 (also odd values like 8001) is skipped. `safe.rs` allows `FortressAttack { soldiers: 1 | 2 }`. The sacrifice (soldiers full, nobody worth it) stays at 1 soldier. Test `raid_soldiers_follow_the_advice_within_the_cap`.

## Build order after Fortress 6, Hall of Knights (2026-10-15, user) – `fortress::PLAN`, `pick_generic`
- User: storage capacity is irrelevant (it only limits the Woodcutter's Hut / Quarry production; raids/attacks ignore it and are a far better wood/stone farm) → resource buildings are never upgraded again. The Barracks does NOT need more than level 4 (the bot checks constantly, soldiers never pile up).
- Order (the first idea, oscillating by level/weight, was replaced the same day): `PLAN` ends Fortress 6 → Academy 6 → Laborers' Quarters 6; then `pick_generic` builds whatever is missing (Barracks to 4, Treasury up to the Fortress level – user 2026-10-15: it is not that expensive and holds more epics/gems/elixirs –, Gem Mine 1; the Treasury only after the Academy and the Laborers' Quarters); every new Fortress level is followed by the Academy, then the Laborers' Quarters up to that level, then the missing rest. Test `generic_rules_after_the_plan`.
- **Hall of Knights:** taken to at least level 8 (`HOK_MIN_TARGET`), beyond that up to the Fortress level. UNVERIFIED whether the game lets it exceed the Fortress level: after a refused upgrade `HOK_FAILED` stops retrying until the Hall or the Fortress level changes. Check the `[fortress] Upgrading the Hall of Knights` lines / errors after the release.
- IDEA, not built: the Smithy (upgrades soldiers) – only if opponents' defence gets so strong that raids stop paying (evaluate from the raid log). Its requirements are the Wall (fortifications), the Mages' Tower and the Archery Guild, buildings of units we never train/upgrade (we WANT to be attacked) – cheap and short. Conflicts with the standing rule "never upgrade the Wall" → ask the user before building any of them; `build_ok` / `safe.rs` currently refuse all three.

## Hall of Knights cap verified (2026-10-16)
The server answers `need higher building1` when the Hall of Knights (6) is upgraded above the Fortress level (6): Sanek and Chlamydie at 07:58. The old rule "at least 8" is dropped (`HOK_MIN_TARGET` removed); the Hall goes up only to the Fortress level, so level 8 needs Fortress 8.
Correction (user 2026-10-16): what he meant was a CAP, not a minimum: the Hall of Knights goes up to min(Fortress level, 8) and no further for now (`HOK_MAX_TARGET`).

## Fortress buddies (user 2026-10-16)
Seen in the log: `[Sanek] [fortress] Counter-attack target Wecros (27406): wood 596, stone 189` – Wecros is one of our own characters. This is WANTED ("fortress buddy"): our characters keep attacking each other via counter-attacks. Why it works: an attack loots the resource BUILDINGS (Woodcutter/Quarry), not the storage, and our storages are mostly full, so the buildings could not be collected anyway – the loot is free for the attacker and costs the victim nothing it could have used. Each side's bot attacks the other only when the loot passes its own worth-it check (`soldiers_for`, 2× soldier cost per soldier), so it is mutually worth it. No special code: `military` takes any `FortressDefense` entry of the combat log, own characters are not excluded and must NOT be excluded (and no extra buddy logic is needed).
