# Weekly events (Friday 00:01 – Sunday 23:59)

Since 2026-10-16 (user): a wave of game events runs EVERY WEEK from Friday 00:01 to Sunday 23:59 (server time). The user lists the
events and says how to treat them; this file is the rule book. NOTHING here is implemented yet (2026-10-16: rules recorded,
the user is still listing further events) – when coding, extend `tasks::beer_justified` / `safe.rs` (see the mushroom rule in CLAUDE.md).

Currency that matters to the user: **XP** (the only real currency). Gold matters less – the higher the level, the more gold
comes anyway, so a gold event fades in value quickly.

## Active events (2026-10-16)
| Event | Effect | Rule |
|---|---|---|
| Exceptional XP | 2× XP from expeditions, daily rewards, Arena fights, Academy, Wheel of Fortune; special pet Luchtablong can be found | **Drink 10 beers in the Tavern** (user explicitly allows mushrooms for this): 10 beers ≈ 200 extra Thirst for Adventure, i.e. as many expeditions as possible at double XP. |
| Glorious Gold Galore | 5× gold from expeditions, City Guard shifts, Game of Dice (not unlocked yet), Gold Pit (not unlocked yet), Wheel of Fortune; pet Armoruck can be found | Gold alone does NOT justify beers. It is only a bonus on top of the XP event: with both running, the 10 beers/day (Fri+Sat+Sun = 30 mushrooms) are drunk anyway. If the Gold event ran alone: no beers. |

## Rules
- Beer budget in these events: up to 10 beers a day (the game's daily limit), Friday–Sunday, ONLY while the Exceptional XP event is active.
  This is a new exception to the "never spend mushrooms" rule (recorded in CLAUDE.md and `docs/precedents.md`); the old narrow
  Gleeman/quest beer exceptions stay as they were.
- The trigger is the XP event, not the gold one. The gold event only makes the beers more profitable.
- Everything else (never other mushroom spending, hourglasses, skips) is unchanged.

## Open questions (asked 2026-10-16, see status.md)
- When to drink: all 10 at once at the start of the day (00:01), or one by one whenever Thirst for Adventure runs out? Beers only help if the
  expeditions actually fit into the day (an expedition takes time; 10 beers ≈ 200 min of extra Thirst).
- Detecting the event: from the server (event list in the game state) – to be checked in sf-api when coding.
