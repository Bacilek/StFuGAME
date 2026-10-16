# Arena

See also `docs/arena-highlights.md`: after a win, scores how "cool" the fight was from the raw combat log and
marks standout ones via `PlayerCombatLogMark` (shows up in Quarter → Mail). Not yet verified live.

## Rules (from the user)
- Fight whenever the Arena is **off cooldown**, even during an expedition (the Tavern does not get in the way).
- **At most 10 wins per day**, after that the Arena gives no rewards. Driven by the server counter `fights_for_xp`
  (per the user = today's wins for XP, 0–10), so the server resets the day.
  On cooldown a fight would cost a mushroom: the server ignores the `use_mushroom` flag and always fights (sf-api docs on `Command::Fight`).
- Challenge whichever of the 3 offered opponents sf-api's own battle simulator gives us the best simulated win
  chance against (`sf_api::simulate::simulate_battle`, 300 iterations – the same simulator `tournament.rs` uses
  for the daily duels, just fewer iterations since this runs every ~10 min per character instead of once a day).
  Changed 2026-10-08 (user): the previous coarse attribute-only formula ("weakest of 3" by `power()` below)
  led to a long real losing streak on TestChar1 (32 losses out of 41 fights on 2026-10-08) because it ignores
  weapon damage, crit/block chance and class matchups entirely – a battle simulation accounts for all of that.
  Not yet verified live (pending: does the win rate actually improve?); if it doesn't help, easy to revert to the
  old formula (`git log` has it).
- The old formula is kept as `arena::power()`/`arena::total()` for `hunt.rs`'s Hall of Fame search (there it
  would mean a full battle simulation per candidate scanned, far too many `ViewPlayer` calls): opponent strength
  = 100 % main attribute + 80 % Constitution (CON) + 27 % Luck (LCK) + 10 % each secondary attribute.
  E.g. Scout: 100 % DEX + 80 % CON + 27 % LCK + 10 % STR + 10 % INT.
  Attribute = base + equipment bonuses + pet bonus (`attribute_basis + attribute_additions + attribute_pet_bonus`).
- Main attribute by class (`Class::main_attribute` in sf-api): STR Warrior, Paladin, Battle Mage, Berserker;
  DEX Scout, Assassin, Demon Hunter, Plague Doctor; INT Mage, Druid, Bard, Necromancer.
- Losses are fine (user): the opponents rotate and our honor drops, so we get weaker opponents and can win for XP later.

## Safeguards (src/safe.rs)
- `Fight` only with `use_mushroom: false` and only when `next_free_fight` + a 30 s margin has passed.
- After our fight another one may only happen once the server has sent a NEW cooldown end (later than our fight).
- Mushroom watchdog: if mushrooms decrease after any command, the bot exits immediately (exit 2).

## Log `logs/arena.jsonl` (user request)
One line per fight: `date`, `fight_of_day` (which fight of that day), `opponent`, `won`, `honor`, `gold`, `xp`.
No time, no opponent strength, no `fights_for_xp`.

## Known bug (minor, cosmetic)
`fight_of_day` in the journal can glitch to `1` right after the bot restarts (one observed case on TestChar1,
2026-10-08: ...39, 1, 41 – coincided exactly with a release rebuild/restart). `fights_today()` reads
`arena.jsonl` fresh each time; a transient read failure right after restart yields 0 lines → `fight_of_day = 1`.
Only mislabels that one journal entry, does not lose data or affect the real server-side win count. Not fixed
(low priority, rare, harmless) – revisit only if it starts happening more often or the user wants a retry added.

## Flow (src/arena.rs, loop in src/main.rs)
- Loop: Arena (when possible) → one expedition → again. During an expedition the Arena is tried between steps
  and the waiting is cut short when the Arena becomes free earlier. With nothing to do the bot waits until the cooldown ends (+30–120 s),
  after 10 wins 30 min.
- The bot runs until it is stopped.

- Fights started from the Hall of Fame (`Fight` by name) do not count towards `fights_for_xp` (user 2026-10-07).
  Used only for Gleeman fight tasks (`src/hunt.rs`).

## Verification status
| What | Status |
|---|---|
| Opponents load from `enemy_ids` (otherwise `CheckArena`) | ✅ 2026-10-07 |
| `ViewPlayer` returns the opponent's stats | ✅ 2026-10-07 |
| Arena cooldown 10 min | ✅ 2026-10-07: fight 16:49:24, next 16:59:24 |
| Fighting during City Guard works | ✅ 2026-10-07 (several fights 17:13–18:14) |
| Fighting during an expedition works | ⏳ not verified |
| `fights_for_xp` = today's wins for XP (user) | ✅ 2026-10-07: after a win 0 → 1, unchanged after a loss |
| Simulated-win-chance opponent picking actually improves the win rate | ⏳ not verified, 2026-10-08 (pending a rebuild + a day of fights) |

## "Attack me" mode (2026-10-12, user; corrected the same day) – `src/attack_me.rs`
Purpose: to LOWER the honor and rank of the user's main `Bacilek31` so he fights among weaker opponents (and can reach the achievement for 25 / 50 / 100 Arena fights won in a row: he takes his weapon off and loses on purpose). It is NOT about the bots' XP wins. A toggle button "Attack me: off/ON" in the app window (next to "Run end of day now", IPC `attack_me`) switches it for ALL bot characters at once; the state is remembered in `roster/attack_me.txt` (file exists = on).
- While on: `hunt::run` puts `Hunt::AttackMe` ahead of every other hunt: the character looks Bacilek31 up in the Hall of Fame (`ViewPlayer`, logs his rank/honor/level) and attacks him by name (`Fight{name}`). These fights use the Arena cooldown but are Hall of Fame fights, so they do NOT count towards the 10 XP wins; while the switch is on they take every free Arena window of the bots, i.e. the XP wins wait (the user accepted "priority"). The character Bacilek31 never attacks itself; the Arena path (`arena::run`) is unchanged.
- ⏳ Unverified live: the lookup/fight by name for this player. The user turns it off when his honor is low enough.

### Test of the attack-me mode (2026-10-12 13:41–13:56, first run after the release)
The mode worked technically: right after the start (13:38) three characters found Bacilek31 (`PlayerLookAt`) and fought him by name – Mrožik LOSS, Květoš WIN, Novotné LOSS (he is a level 27 Paladin with ~89k life, the bots are level 21–22 with ~33k) – then the user switched it off at 13:56. The other characters did not attack in that time: hunts are only reached in passes that do not `continue` out of the Tavern section, and the Arena cooldown applies. Findings: (1) the bots are far too weak to beat him, so a lost bot fight only costs the bot honor; (2) the user himself, attacking Chlamydie with the weapon off (`PlayerArenaFight`), still WON and gained 53 honor (rank 7962 → 7785), i.e. the opposite of what he wants; (3) lowering his honor needs HIM to lose (as the attacker against much stronger players with gear off) – the bots cannot do that for him. The feature stays available (button) but is not useful unless he gets much weaker.
Captures: `PlayerGetHallOfFame` with a name = a list page of 30 players around that name (`Ranklistplayer.r:rank,name,guild,level,honor,class[,lang];…`); a friend's `PlayerLookAt` shows `otherplayerfriendstatus:1`.

## Rumble for Riches (2026-10-16, user)
During the weekly event the choice among the 3 opponents prefers one whose win pays gold: opponent rank ≤ ours + 50 and honor ≥ ours − 955 (`arena::rumble_pays`) with a simulated win chance ≥ 50 %. UNVERIFIED in the real game: check the `[arena] Rumble for Riches` line and the gold in the fight lines.
Answer 2026-10-16 (user): wins pay gold even beyond the 10 XP wins, so during Rumble for Riches the 10-win limit is lifted (`arena::ready`/`secs_until_ready` via `rumble_unlimited`); losses do not matter. Scrapbook hunts (`hunt::scrapbook_due`) yield the cooldown during the event.
REVERSED 2026-10-16 (user: "gold se při prohře v aréně ztrácí"): the 10-win limit applies again during Rumble for Riches (`rumble_unlimited` removed); after the 10 XP wins only Hall of Fame scrapbook hunts. The gold-paying-opponent preference stays for the 10 XP wins.
