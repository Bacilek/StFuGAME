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
- Never spend mushrooms or hourglasses on finishing a build (`FortressBuildFinish { mushrooms }` stays out of the
  whitelist, like every command with mushrooms).

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
- Order: Fortress → Woodcutter's Hut level 1 (second upgrade, user capture 2026-10-11) → then Fortress and Laborers' Quarters together (the lower level first, Fortress on a tie). ONLY these three buildings (also enforced in `safe.rs`); no Wall, no Gem Mine yet (character level 125, not implemented), nothing else until the user answers question 1.
- Gold priority (user 2026-10-11): better items > fortress > spinning the shops > attributes.
- Not handled yet: counter-attacks, units, the unlock of the Fortress feature itself for characters hitting level 25 (an `UnlockFeature` ident is probably pending like the dungeons' – unknown), waking up exactly when a build finishes (the main loop's ≤ 30 min cadence applies).

## Second capture (Woodcutter's Hut level 1 started, user 2026-10-11)
`fortress:1/0/0/…/3/1791727864/1791726964/10/110/9060/…` – Fortress level 1, build target `3` = Woodcutter's Hut (3 − 1 = 2), duration 900 s again, silver 19271 → 19071 (2 g).

## Answers (2026-10-11)
2. Gold: better items > fortress > spinning > attributes. 3. Gem Mine from CHARACTER level 125 – yes. 4. Start on every character that reaches level 25 (TestChar1 will be first).

## Open questions (asked 2026-10-11, answers go to `docs/precedents.md`)
1. Wood and stone come from the Woodcutter's Hut and the Quarry – may the Quarry and other buildings (Treasury, Academy, units, Smithy, Hall of Knights) be upgraded too? The user will answer later.
