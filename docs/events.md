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
| Assembly of Awesome Animals | guaranteed fruits from expeditions; pets can be fed 9 fruits a day; shorter travel with premium mounts; 3× fruits from the Wheel of Fortune | We have no pets unlocked, so maybe no fruits (a bonus for the future). Never a reason for beers on its own – the XP-event beers cover it ("two birds with one stone"). Faster expeditions: the loop waits for the server's real end time (`busy_until`), so nothing to change; watch the logged duration of the first expedition. |
| Fantastic Fortress Festivity | wood and stone in expeditions; only BIG gems in the Gem Mine; unit training time halved; 2× wood/stone from Game of Dice (not unlocked) and the Wheel | Expedition rewards: unchanged order (we never pick wood/stone). Training halved: the soldiers are looked after every 3 min instead of 5 (`fortress::military`), so raids come faster. Gems: nothing to do. |
| Days of Doomed Souls | souls can be found on expeditions; 2× souls from luring heroes into the underworld, collecting the soul extractor, Game of Dice and the Wheel of Fortune | We have no souls feature; we expect not to find any. If they show up, keep the existing reward order (mushrooms > gold > hourglasses >>> wood/stone). |
| Witches' Dance | all equipment can be thrown into the witches' cauldron for twice its selling value (unlocks at level 66 and after collecting all enchantments) | Do not care yet (not unlocked). |
| Epic Shopping Spree Extravaganza (level 50+) | 5× chance of epics in shops; pet Knilight can be found | Do not care much: epics in shops cost mushrooms (about 15). If an epic ever costs only gold, the normal shop rule applies (better item → buy and equip). Never pay mushrooms for it. |

Academy: during Exceptional XP the Academy's XP stock is assumed to fill twice as fast (`fortress::minutes_to_full`), so it is collected earlier and wakes the loop earlier – collecting is free, an early collect costs nothing.

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
