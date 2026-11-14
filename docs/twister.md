# Dungeon Twister (user 2026-11-07; NOT implemented, the bot skips it)

- Unlocks at character level 130 (the user believes that is the only requirement). A special (shadow-list) dungeon with 1000 floors.
- Rule from the user: whenever we beat the opponent with MORE THAN 90 % of our HP left, the next floor can be fought for free (no mushroom, no waiting), so the fights chain.
  Rewards seen by the user (floor 37 on the main character): mostly common items or nothing, some XP.
- sf-api 0.4.4 (vendored): `ShadowDungeon::Twister` (index 14, floor limit 1000), `Dungeon::is_with_companions` = false (solo fight), the fight command is rendered as
  `PlayerDungeonBattle:<LightDungeon::Tower as u32 + 1>/<use_mushroom>` = `PlayerDungeonBattle:15/0` (consistent with the capture below: Twister is shadow index 14 → 15).
- Until it is mapped `dungeons::open_dungeons` excludes the Twister, so the normal "lowest enemy level" choice never picks it (its floor enemies would otherwise look like the lowest-level opponent every time).

## Capture 2026-11-14 (user, Bacilek31, level 199, floor 43 done) – analysed
- Response keys: `dungeonprogressshadow(37)` index 14 = `43` (floors done); `dungeonenemiesshadow(11)` lists the floor monsters as `id/dungeon/?` triples with dungeon `15` (= Twister) for ids 29, 518, 544, 491, 225;
  `currentdungeonenemiesshadow(3)` third triple `544/15/208/2/0/229` = the NEXT floor's enemy (level 208). The fight just fought was monster 518 (`lastfightstore`: `74635424,518,1,25,1794680762,0`, type 25 = Twister, 1 = win).
- Fight: our HP 9061425, enemy level 207 HP 5461040. Winner us; our HP at the end 8279868 = **91.4 %** (> 90 %) → free chain.
- **The free-chain signal is `characterstatus[21]` (= `dungeons.next_free_fight`)**: it was `1794680743`, i.e. 19 s BEFORE the fight (1794680762) – the timer was NOT pushed 1 h ahead as after a normal dungeon fight.
  So after a >90 % win the server leaves the old (expired) timer, `UpdateDungeons` still reports "free". After a <90 % win presumably the timer is set to fight time + 1 h (not captured yet).
  (Other timestamps in `characterstatus`: [3] 1794685211 = current action end, [9] = calendar next = midnight.)
- Reward in the sample: `battlereward 1/1/6634810/1332282…` (xp 1332282 as a level-199 character, gold 66 348.10).

## Why it is not just switched on (design notes for implementation)
- `safe::send` lets a second `FightDungeon` through only after the server has sent a NEW cooldown end later than our fight (`docs/dungeons.md` Safeguards). In a free chain the timer stays old, so the chain would be blocked.
  Needed: a Twister-specific rule – after a Twister win compute `hp_left / hp_max` from the fight (`last_fight` / the `fight.r` rounds) and allow the next Twister fight when > 90 % AND the timer is still in the past (an on-cooldown fight would cost a mushroom, so the timer must be re-read with `UpdateDungeons` before every floor).
- A fight on cooldown costs a mushroom → the guard must stay strict: Twister only if `dungeon_is_free` is true on a fresh `UpdateDungeons`; stop the chain at the first loss or <90 % win (the timer will then be +1 h).
- Open: does the first Twister fight of the hour share the 1 h dungeon cooldown with the light dungeons (the chain-break case)? Does a Twister fight at <90 % set the shared timer?
- Floor enemies are chosen by the server (current floor), nothing to pick; `Candidate` for the Twister = the `currentdungeonenemiesshadow` entry with dungeon 15.

## Needed from the user
- One capture of a Twister fight that ends with < 90 % HP left (does `characterstatus[21]` jump to fight time + 1 h?) and the request line (expected `PlayerDungeonBattle:15/0`).
- Whether the user wants the bot to chain the Twister fully (all floors while > 90 %) – risk: each chain fight takes real time and fills the backpack with common items (inventory logic would have to sell them).
