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

## Open questions (asked 2026-10-11, answers go to `docs/precedents.md`)
1. Wood and stone come from the Woodcutter's Hut and the Quarry – may they be upgraded too (needed to pay for the
   Fortress / Laborers' Quarters upgrades)? Treasury (storage), Academy, units, Smithy, Hall of Knights?
2. Gold: upgrades cost silver. Budget against the shop reserve and the attribute buying?
3. Gem Mine: character level 125 – confirmed?
4. Which characters first (level ≥ 25 only; test on one character)? Collecting resources (free) and counter-attacks later.
