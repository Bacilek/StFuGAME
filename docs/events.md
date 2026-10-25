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
| Assembly of Awesome Animals | guaranteed fruits from expeditions; pets can be fed 9 fruits a day; shorter travel with premium mounts; 3× fruits from the Wheel of Fortune | Pets ARE unlocked now (`docs/pets.md`). Hourly habitat dungeon fights run ONLY during this event (user 2026-10-23: "we count on it", `pets::fights_allowed`). Feeding: the daily limit is 3 fruits per pet normally, **9 per pet during the event** (user); the bot learns the limit from a refused feed (`pets::feed`), nothing to configure. Never a reason for beers on its own. Faster expeditions: the loop waits for the server's real end time (`busy_until`), nothing to change. |
| Fantastic Fortress Festivity | wood and stone in expeditions; only BIG gems in the Gem Mine; unit training time halved; 2× wood/stone from Game of Dice (not unlocked) and the Wheel | Expedition rewards: unchanged order (we never pick wood/stone). Soldiers train 2× as fast (user: 1 soldier per 5 min instead of 10): looked after every 3 min instead of 5 (`fortress::military`), so raids come faster. Gem Mine yields ONLY the biggest gems (user) – nothing to do (Gem Mine only from level 125, we don't mine anyway). |
| Days of Doomed Souls | souls can be found on expeditions; 2× souls from luring heroes into the underworld, collecting the soul extractor, Game of Dice and the Wheel of Fortune | We have no souls feature; we expect not to find any. If they show up, keep the existing reward order (mushrooms > gold > hourglasses >>> wood/stone). |
| Witches' Dance | all equipment can be thrown into the witches' cauldron for twice its selling value (unlocks at level 66 and after collecting all enchantments) | Do not care yet (not unlocked). |
| Epic Shopping Spree Extravaganza (level 50+) | 5× chance of epics in shops; pet Knilight can be found | Do not care much: epics in shops cost mushrooms (about 15). If an epic ever costs only gold, the normal shop rule applies (better item → buy and equip). Never pay mushrooms for it. |
| Epic Quest Extravaganza | spending at least 100 and 300 Thirst for Adventure (minutes) on expeditions finds one epic item each | Never aimed at directly. 100 is free every day (every character already found an epic that way); with the 10 XP-event beers (+200) the 300 mark is reached too, so a second epic comes along. Epics are still NEVER sold (inventory space is saved otherwise); the beers' Thirst must actually be spent on expeditions the same day. |
| Epic Good Luck Extravaganza | better chance of an epic from the Wheel of Fortune; from Game of Dice (not unlocked) | Nothing to do (the free daily spin is taken as before). |
| Rumble for Riches | Arena wins pay gold, except when the opponent is more than 50 ranks behind us in the Hall of Fame or has more than 955 less honor; limit depends on level, max 50 million gold a day | CODED (`arena::rumble_pays`): among the 3 opponents the bot prefers one whose win pays gold (rank ≤ ours + 50, honor ≥ ours − 955) when the simulated win chance is ≥ 50 %; otherwise the usual best-win-chance pick (task class pick still wins over it). REVERSED 2026-10-16 (user): a LOST Arena fight costs gold during the event, so no constant fighting – back to the 10 XP wins, then only Hall of Fame hunts for the scrapbook. |
| Gloom Set | chance of finding the epic Gloom Set in the weapon shop, magic shop and on expeditions | Nothing special: epics found are kept, shop epics only if they cost gold. |

## Events not used by the bot yet (user 2026-10-23: only documented, do NOT implement – no calls/responses captured yet)
sf-api has both as `Event::TidyToiletTime` and `Event::ForgeFrenzyFestival`; the bot reads neither.
| Event | Effect (user) | Notes |
|---|---|---|
| Tidy Toilet Time | 2 items can be sacrificed in the toilet every day; sacrifices generate 2× aura; pet Mantiflame can be found during the event | Toilet (`tavern.toilet`, level 100, `docs/todo.md`) is not implemented. When it is: use both daily sacrifices during this event (epics are kept for it, never sold). Mantiflame = pet id 16 (`docs/pets.md`). Toilet captures exist since 2026-10-25 (`docs/toilet.md`), not implemented. |
| Forge Frenzy Festival | metal and arcane splinters can be found on expeditions; 2× Blacksmith resources for dismantling; 15 instead of 5 points for dismantling at the Blacksmith (task `BlacksmithDismantle`); 2× arcane splinters from Game of Dice and the Wheel of Fortune | Dismantling instead of selling is implemented (2026-10-24, `docs/blacksmith.md`; the server's `dismantle_left` decides how many, 15 in the event); no extra dismantles just for the task points yet. Expedition splinters: normal reward order applies (unknown ones get logged). Today the bot does nothing special. |

Academy: NO special handling (2026-10-16). A first attempt assumed the stock fills twice as fast during the XP event; it made Academy 4/5 characters (limit ~520-738, ~500/h) look "full within 45 min" at once, so they collected 1-31 XP on every pass (user noticed the spam). Reverted: the server's own `per_hour` is used; if it turns out the event doubles only the collected amount, nothing changes for overflow.

## Rules
- Beer budget in these events: up to 10 beers a day (the game's daily limit), Friday–Sunday, ONLY while the Exceptional XP event is active.
  This is a new exception to the "never spend mushrooms" rule (recorded in CLAUDE.md and `docs/precedents.md`); the old narrow
  Gleeman/quest beer exceptions stay as they were.
- The trigger is the XP event, not the gold one. The gold event only makes the beers more profitable.
- Everything else (never other mushroom spending, hourglasses, skips) is unchanged.

## Answers (user 2026-10-16) and implementation
- When: as soon as Thirst for Adventure runs out AND the character is free. A running City Guard shift (5x gold!) is never cancelled; right after it ends (`guard::run` after collecting the pay, before a new shift) the beer is drunk, then the Tavern uses the Thirst.
- Starts the same day (Friday 2026-10-16) after the shift. On Monday it must be off: `tasks::xp_event_active` needs the event in the server list and an end time in the future (the weekday check was dropped later the same day, see the end of this file).
- Gleeman beers are gone for good (user: "for the Gleeman we never drink beers any more", at most a quest may offer mushrooms): `Means.beers = 0`, the travel/quest beer helpers are dead code kept with their tests.
- All characters drink, even those with few mushrooms (they should all have enough); the only stop is 0 mushrooms.
- (2026-10-17, user: burst of up to 5 beers at 0 Thirst, `BEER_BURST`) One beer per call before that; condition: idle, Thirst for Adventure below the cheapest offered expedition, `beer_drunk < beer_max`.
- Verification: ⏳ watch `[tasks] Exceptional XP event: drinking beer n/10` in the logs, the mushroom watchdog must accept each −1.

Correction (user 2026-10-16): the Academy fills at exactly the same speed during the XP event; only the COLLECTION pays 2× XP. So the overflow rules and collection timing stay as they are (`fortress::should_collect`, no event logic).

## Server-driven, no manual switch (user 2026-10-16)
An Events tab with manual switches and end times was built and REMOVED the same day (user: the bot reads the events from the server, no tab needed). `tasks::event_active(gs, event)` = the event is in the server list (`gs.specials.events.active`, a bitmask the sf-api parses from the `events` response key) AND the list end time (`ends`, shared by all events, normally Sunday 23:59) is in the future. NO hardcoded weekday any more: the XP beers stop the moment the server stops announcing the event or its end time passes, on whatever day that is (Monday morning included), and would start on any day the server announces it. Beers are re-checked in `safe.rs` before `BuyBeer` (`beer_justified`).
Known limit: the list only changes when a response carries the `events` key; if a long-running session never gets a fresh one, a NEW event is noticed late, while an ended one is still caught by `ends`. Watch the first Friday after the rebuild (`[tasks] Exceptional XP event: drinking beer`) and note whether a session started on Thursday notices Friday 00:01.

## Monday–Thursday task events (Goblin Gleeman event tasks, found 2026-10-12 and 2026-10-19)
Besides the Friday–Sunday wave there is a 4-day event of Gleeman event tasks (Monday 00:00 – Thursday 00:00; the server's `event.end` shows it). They are found under the Goblin Gleeman. Seen so far:
| Week | Name | Tasks (1 point each, 3 chests at 1/2/3 points) |
|---|---|---|
| 2026-10-12 | Ruffian event (name from earlier notes) | win N× against a class, against another class, 5× in a row, 30× in the Hall of Fame |
| 2026-10-19 | **Part-Time Nudist event** (user) | 10 wins with no epics/legendaries (`WinFightsNoEpicsLegendaries`), 10 wins with no chest plate (`WinFightsNoChestplate`), 10 wins with no gear (`WinFightsNoGear`, not mapped), 10 wins bare hands (`WinFightsBareHands`) |
Chest 3 of the Nudist event is beer ×1 (raises the daily beer cap to 11, not free) + ~1.4–2.7k gold: low priority (user 2026-10-19), NoGear/NoEpics only when they fit without effort. See `docs/tasks.md`, `docs/precedents.md`.
