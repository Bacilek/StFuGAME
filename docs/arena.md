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
  = 100 % main attribute + 80 % Constitution (CON) + 40 % Luck (LCK) + 10 % each secondary attribute.
  E.g. Scout: 100 % DEX + 80 % CON + 40 % LCK + 10 % STR + 10 % INT.
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
