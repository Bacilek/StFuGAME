# Dungeons

## Rules (from the user)
- Whenever the Dungeons are **off cooldown**, one fight. **Never for mushrooms** (the server ignores `use_mushroom`,
  so a fight on cooldown would always cost a mushroom).
- Runs alongside the Tavern and City Guard.
- The cooldown should be 1 h (user). The bot follows the server time though (`dungeons.next_free_fight`).
- No fight with a full inventory (to be solved later).
- Several unlocked dungeons: pick the enemy with the lowest level, at a similar level (up to +2) the one with weaker stats
  (strength as in the Arena: 100 % main + 80 % CON + 40 % LCK + 10 % secondary).

## Implementation (src/dungeons.rs)
- Always `UpdateDungeons` before deciding: neither `Update` nor a fight refreshes the Dungeons timer (sf-api docs).
- Enemies: `Dungeons::current_enemy` (sf-api data), light and shadow dungeons, except the Tower (it has its own command).
- The "mirror image" (sf-api: a level 0 warrior) counts as our own character (level and stats).
- When no fight happened (full inventory), the next attempt is at the earliest after 5 min.

## Safeguards (src/safe.rs, shared with the Arena)
- `FightDungeon` only with `use_mushroom: false` and only once the cooldown end + 30 s has passed.
- After our fight another one may only happen once the server has sent a NEW cooldown end (later than our fight).
  A stale state therefore never lets through a fight for a mushroom.
- Mushroom watchdog: if mushrooms decrease after any command, the bot exits immediately.

## Verification status
| What | Status |
|---|---|
| Cooldown 1 h | ✅ 2026-10-07: fight 16:59:29, next 17:59:29 |
| `current_enemy` matches the enemy in the game | ✅ 2026-10-07: Training Camp Henry Hobbyhorse (lvl 3) → after the win Broken Brian (lvl 4), fights went through |
| A win is "accepted" by itself (no extra command) | ✅ 2026-10-07: Training Camp, xp +280 |
| Fighting during City Guard works | ✅ 2026-10-07 18:01 (win, xp +360, +1 g 90 s) |
| Fighting during an expedition works | ⏳ not verified |

## Unlocking dungeons (user 2026-10-09)
- Pending unlocks arrive in the `unlockfeature` key (`GameState::pending_unlocks`, pairs `main/sub`) at login and after
  `PlayerDungeonOpen`. The game client sends `UnlockFeature:<main>/<sub>` (command via `GET cmd.php?req=UnlockFeature&params=<base64 "30/1">`)
  when the Dungeons tab is opened with `30/1` pending → Desecrated Catacombs go from `-1` (locked) to `0` (open) in `dungeonprogresslight`.
- The bot does the same in `dungeons::run` (`DUNGEON_UNLOCK_IDENT = 30`), only this ident is whitelisted in `safe.rs`
  (`UnlockFeature { main_ident: 30, .. }`). `9/1`, `5/1`, `40/1` are unknown features (not dungeons as far as we know), never sent.
- Light World unlock conditions (user, from memory, in the game's order): levels 10, 20, 30, 40, 50, 70, 80, 95, 110, then a mix
  of "after dungeon N cleared" (1–9, 10, 11, monster 5 of 11, 12, …) and levels 180, 200, 210, 240, 270, 280, 340, 480, 500, 520,
  540, 560, 580. Keys for further dungeons are found on expeditions (after reaching the level) or in the Gem Mine (Fortress, from level 25).
  Not implemented: only the level-10 unlock (ident 30) is known to work so far.
- Fight reward sample (Mimimimi11, first Catacombs fight, win): `battlereward` 650 gold / 1287 xp.
