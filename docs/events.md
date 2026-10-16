# Weekly events (Friday 00:01 – Sunday 23:59)

Since 2026-10-16 (user): a wave of game events runs EVERY WEEK from Friday 00:01 to Sunday 23:59 (server time). The user lists the
events and says how to treat them; this file is the rule book. The beer rule is IMPLEMENTED (2026-10-16, `tasks::beer_justified` / `event_beer`, checked again in `safe.rs`); other events are not coded yet (the user is still listing them).

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

## Answers (user 2026-10-16) and implementation
- When: as soon as Thirst for Adventure runs out AND the character is free. A running City Guard shift (5x gold!) is never cancelled; right after it ends (`guard::run` after collecting the pay, before a new shift) the beer is drunk, then the Tavern uses the Thirst.
- Starts the same day (Friday 2026-10-16) after the shift. On Monday it must be off: `tasks::xp_event_active` needs the event in the server's list, an end time in the future AND weekday Fri–Sun.
- Gleeman beers are gone for good (user: "for the Gleeman we never drink beers any more", at most a quest may offer mushrooms): `Means.beers = 0`, the travel/quest beer helpers are dead code kept with their tests.
- All characters drink, even those with few mushrooms (they should all have enough); the only stop is 0 mushrooms.
- One beer per call; condition: idle, Thirst for Adventure below the cheapest offered expedition, `beer_drunk < beer_max`.
- Verification: ⏳ watch `[tasks] Exceptional XP event: drinking beer n/10` in the logs, the mushroom watchdog must accept each −1.
