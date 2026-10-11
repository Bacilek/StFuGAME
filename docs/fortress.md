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
The user dictates the sequence; the bot builds the first entry below its target level and WAITS for it (saving the wood/stone for it) instead of skipping ahead: Fortress 1, Woodcutter's Hut 1, Quarry 1, Laborers' Quarters 1 (all done by hand), then **Fortress 2, Laborers' Quarters 2, Woodcutter's Hut 2, Quarry 2, Treasury 1**. After that the bot builds nothing until the user dictates more. Allowed in `safe.rs`: exactly the buildings of the plan (Fortress, Laborers' Quarters, Woodcutter's Hut, Quarry, Treasury). Capture after the Laborers' Quarters level 1: `fortress:1/1/1/1/…`; the build times fell from 1895 s to 1800 s (Laborers' Quarters effect) and the Laborers' Quarters level 2 costs 10 gold + 138 wood + 46 stone.

**Reserve (user 2026-10-11): the shop reserve does NOT apply to the fortress** – a build may use all the gold (it still comes after the day's shop pass, so better items are bought first).
