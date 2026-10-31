# Once-a-day battles + the Belt's free beer (user 2026-10-31)

Code: `src/battles.rs` (called from the main loop right after `guild::battles`), `tasks::free_beer_ok` / `tasks::free_beer`, whitelist in `safe.rs`.

| What | Command | Condition (also re-checked in `safe::send_raw`) |
|---|---|---|
| Demon Portal (personal; does NOT start the Dungeons cooldown) | `PlayerPortalBattle:` (`FightPortal`) | level >= 99 and `dungeons.portal.can_fight` |
| Guild Portal | `GroupPortalBattle:` (`GuildPortalBattle`) | level >= 99, in a guild, own member entry has not fought the portal today |
| Guild pet vs. hydra | `GroupPetBattle:0` (`GuildPetBattle { use_mushroom: false }`, NEVER true) | in a guild, `hydra.last_battle` not today (the leader must have set a pet: a refusal in other guilds is expected) |

One attempt per kind and day (`roster/<character>/battles_day.txt`), a failure is only logged (`[battles] ... refused or failed`). Level 99 is assumed from sf-api
for the portals, the pet battle has no known requirement. Captures from the user: `docs/` has none saved; the responses carry `fightresult.battlereward`.
Verification: NOT seen live yet; check `[battles]` lines and whether the guild Portal/pet battle worked in guilds without a pet.

## Free beer (Thirsty Wanderer, Belt enchant 71)
The first beer of the day (`gs.tavern.beer_drunk == 0`) is free while the worn Belt carries enchant 71 (user verified 2026-10-31, also when the
belt was enchanted during the day, `PlayerBeerBuy` response without any mushroom change). `tasks::free_beer`: drinks it once 20 min of Thirst for
Adventure fit under the 100 min cap (or from 21:00 when anything fits). `safe.rs` allows `BuyBeer` for it and gives the mushroom watchdog a
spend allowance of **0**, so a wrong assumption exits the app instead of burning a mushroom. Later beers keep the old rules (Exceptional XP event only).
Supersedes the "belt worn at the start of the day" condition of 2026-10-19.
