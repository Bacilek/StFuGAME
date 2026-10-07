# Arena

## Rules (from the user)
- Fight whenever the Arena is **off cooldown**, even during an expedition (the Tavern does not get in the way).
- **At most 10 wins per day**, after that the Arena gives no rewards. Driven by the server counter `fights_for_xp`
  (per the user = today's wins for XP, 0–10), so the server resets the day. Wins are also counted from `logs/arena.jsonl` as a cross-check.
  On cooldown a fight would cost a mushroom: the server ignores the `use_mushroom` flag and always fights (sf-api docs on `Command::Fight`).
- Challenge the weakest of the 3 offered opponents.
- Opponent strength = 100 % main attribute + 80 % Constitution (CON) + 40 % Luck (LCK) + 10 % each secondary attribute.
  E.g. Scout: 100 % DEX + 80 % CON + 40 % LCK + 10 % STR + 10 % INT.
  Attribute = base + equipment bonuses + pet bonus (`attribute_basis + attribute_additions + attribute_pet_bonus`).
- Main attribute by class (`Class::main_attribute` in sf-api): STR Warrior, Paladin, Battle Mage, Berserker;
  DEX Scout, Assassin, Demon Hunter, Plague Doctor; INT Mage, Druid, Bard, Necromancer.
- Losses are fine (user): the opponents rotate and our honor drops, so we get weaker opponents and can win for XP later.

## Safeguards (src/safe.rs)
- `Fight` only with `use_mushroom: false` and only when `next_free_fight` + a 30 s margin has passed.
- After our fight another one may only happen once the server has sent a NEW cooldown end (later than our fight).
- Mushroom watchdog: if mushrooms decrease after any command, the bot exits immediately (exit 2).

## Flow (src/arena.rs, loop in src/main.rs)
- Loop: Arena (when possible) → one expedition → again. During an expedition the Arena is tried between steps
  and the waiting is cut short when the Arena becomes free earlier. With nothing to do the bot waits until the cooldown ends (+30–120 s),
  after 10 wins 30 min.
- The bot runs until it is stopped.

## Verification status
| What | Status |
|---|---|
| Opponents load from `enemy_ids` (otherwise `CheckArena`) | ✅ 2026-10-07 |
| `ViewPlayer` returns the opponent's stats | ✅ 2026-10-07 |
| Arena cooldown 10 min | ✅ 2026-10-07: fight 16:49:24, next 16:59:24 |
| Fighting during City Guard works | ✅ 2026-10-07 (several fights 17:13–18:14) |
| Fighting during an expedition works | ⏳ not verified |
| `fights_for_xp` = today's wins for XP (user) | ✅ 2026-10-07: after a win 0 → 1, unchanged after a loss |
