# Once-a-day battles + the Belt's free beer (user 2026-10-31)

Code: `src/battles.rs` (called from the main loop right after `guild::battles`), `tasks::free_beer_ok` / `tasks::free_beer`, whitelist in `safe.rs`.

| What | Command | Condition (also re-checked in `safe::send_raw`) |
|---|---|---|
| Demon Portal (personal; does NOT start the Dungeons cooldown) | `PlayerPortalBattle:` (`FightPortal`) | level >= 99 and `dungeons.portal.can_fight` |
| Guild Portal | `GroupPortalBattle:` (`GuildPortalBattle`) | level >= 99, in a guild, own member entry has not fought the portal today |
| Guild pet vs. hydra | `GroupPetBattle:0` (`GuildPetBattle { use_mushroom: false }`, NEVER true) | in a guild, `hydra.last_battle` not today (the leader must have set a pet: a refusal in other guilds is expected) |

One attempt per kind and day (`roster/<character>/battles_day.txt`), a failure is only logged (`[battles] ... refused or failed`). Level 99 is assumed from sf-api
for the portals, the pet battle has no known requirement. Captures from the user: `docs/` has none saved; the responses carry `fightresult.battlereward`.
Verification (2026-11-01 18:39, first live pass): Demon Portal and guild Portal fight fine ("done"); guild pet is refused in guilds without a pet ("group need a pet") or after a fight ("come back tomorrow") – expected.

## Free beer (Thirsty Wanderer, Belt enchant 71)
The first beer of the day (`gs.tavern.beer_drunk == 0`) is free while the worn Belt carries enchant 71 (user verified 2026-10-31, also when the
belt was enchanted during the day, `PlayerBeerBuy` response without any mushroom change). `tasks::free_beer`: drinks it once 20 min of Thirst for
Adventure fit under the 100 min cap (or from 21:00 when anything fits). `safe.rs` allows `BuyBeer` for it and gives the mushroom watchdog a
spend allowance of **0**, so a wrong assumption exits the app instead of burning a mushroom. Later beers keep the old rules (Exceptional XP event only).
Supersedes the "belt worn at the start of the day" condition of 2026-10-19.

## Retries (user 2026-11-08)
Květoš never fought the guild Portal (task `FightGuildPortal 0/1`) although it was open: the single midnight attempt (marker `battles_day.txt` written
before the send) was spent, the log line was gone. Now a kind is marked done only when the command went through AND the game state no longer offers
the fight; otherwise `<kind>-fail <date> <unix time> <count>` is written and the fight is retried after an hour, at most 4 times a day.
