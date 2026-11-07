# Dungeon Twister (user 2026-11-07; NOT implemented, the bot skips it)

- Unlocks at character level 130 (the user believes that is the only requirement). A special (shadow-list) dungeon with 1000 floors.
- Rule from the user: whenever we beat the opponent with MORE THAN 90 % of our HP left, the next floor can be fought for free (no mushroom, no waiting), so the fights chain.
  Rewards seen by the user (floor 37 on the main character): mostly common items or nothing, some XP.
- sf-api 0.4.4 (vendored): `ShadowDungeon::Twister` (floor limit 1000), `Dungeon::is_with_companions` = false (solo fight), the fight command is rendered as
  `PlayerDungeonBattle:<LightDungeon::Tower as u32 + 1>/<use_mushroom>` (odd, verify with a capture).
- Until it is mapped `dungeons::open_dungeons` excludes the Twister, so the normal "lowest enemy level" choice never picks it (its floor enemies would otherwise look like the lowest-level opponent every time).

## Needed from the user
- Captures: one Twister fight request + response (request line, `fightresult`, any HP / "free fight" marker in the response, the dungeon progress key), and one where the next floor
  is free (>90 % HP) vs one where it is not (cooldown time appears). Where the 90 % rule shows (a flag, or only the cooldown value).
- Whether a Twister fight uses the shared hourly dungeon cooldown when the chain is broken.
