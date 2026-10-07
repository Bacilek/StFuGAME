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
