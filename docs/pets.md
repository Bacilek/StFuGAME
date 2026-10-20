# Pets (briefing 2026-10-20, user; NOTHING implemented yet)

Source: the user's memory of a 2023 tutorial video (never played Pets himself, everything below is an assumption until verified)
+ https://en.sfporadnik.pl/pets.php. Unlock: level 75 + one finished Tavern expedition (Egg Basket reward). Main character
reaches it after midnight 2026-10-21.

## Mechanics
- 5 habitats (elements), 20 pets each (ids 1..=100: Shadow 1-20, Light 21-40, Earth 41-60, Fire 61-80, Water 81-100).
- Element advantage chain: Water > Fire > Earth > Light > Shadow > Water (disadvantage ~ -10 % stats/dmg, unverified, and
  whether the other side gets a bonus is unknown).
- Each collected pet: +1 % to its habitat's attribute (Water STR, Light DEX, Earth INT, Shadow CON, Fire LCK); pets at level
  100/150/200 add +0.5/0.75/1 %. More pets of a habitat -> stronger pets of that habitat (pack bonus +5 % per caught pet in sim).
- Each habitat has its own fruit (5 kinds). A pet can be fed 3x a day (`Pet.fruits_today`), +1 level per fruit. Level cap 100
  (200 after all habitats are cleared).
- Fruits come from Tavern expeditions (reward `RewardType::Fruit(habitat)`), PvP/PvE pet fights, tasks. They are scarce.
- Pet classes Mage/Warrior/Scout: Warrior weakest, Scout best, so an earlier Scout can beat a later Warrior.
  Base stats scale by position in the habitat (sf-api `PET_BASE_STAT_ARRAY`: 10 .. 160), class per position in
  `PET_CLASS_LOOKUP` (sf-api, feature `simulation`).
- Habitat dungeon ("pseudo dungeon"): fight pets 1 -> 20 in order; beating one makes it catchable. Catching needs a condition:
  mostly an expedition LOCATION + time of day (day 6-18, night 18-6) or season/holiday/event (e.g. mushroom event). At the
  start the 3 weakest pets of every element are available.
- Trainers: attacks on trainers (5/day, one per element) also give fruits; exact mechanic unknown.
- PvP: 5 a day, one per habitat; defender habitat rotates daily Water -> Fire -> Earth -> Light -> Shadow.

## Plan: the main pet of a habitat changes as better ones are caught
Fruits go to the current main pet of the habitat (or the one we need at a given level to beat the next chain target). Levels
are chosen so we can skip many pets and beat the farthest, usable (mostly Scout) one. A level target need not be reached if
the next chain pet falls earlier -> switch main pet at once. Chains (pet name (position 1-20) [level to feed to]):
- Shadow: Toothey (3) [45] -> Petdacat (9) [70] -> Pinklynx (14) [100] -> Poisnake (20) [100]
- Light: Tinck (3) [82] -> Buckfoxion (12) [93] -> Knilight (18) [80] -> Unikor (20) [100]
- Earth: Smaponyck (3) [100] -> Lilbeatzup (14) [100] -> Mouthrexor (20) [100]
- Fire: Gullps (2) [28] -> Kokofire (7) [100] -> Dragopyre (15) [100] -> Devastor (20) [100]
  Stipulation: Mantiflame may replace Dragopyre, whichever we find first.
- Water: Ocodile (3) [45] -> Octoboss (9) [100] -> Watnake (16) [100] -> Hydrospir (20) [100]
(sfporadnik total fruit for a full habitat: Shadow 200/225, Light 250/259, Earth 192/254, Fire 200/215, Water 200/239/241;
shorter = for mushroom spenders, longer = f2p. The names above are from the user's chain, check against the page.)
Epic catch conditions (sfporadnik): Poisnake after the last Shadow pet (Sunburn Desert), Unikor after last Light (Plains of
Oz'Kurr), Mouthrexor after last Earth (Moldy Forest), Devastator after last Fire (Gnarogrim), Hydrospir after last Water (Nevermoor).

## Priorities
- Expeditions: reward preference mushrooms > fruit > gold > everything else (user 2026-10-20; fruits go before gold).
- Never spend mushrooms on pets (`use_mush` always false; `BrewPotion` at the witch eats 10 fruit, not for now).

## sf-api support (0.4.4, checked 2026-10-20)
`gs.pets` (`Pets`: habitats[HabitatType] with `pets[20]` {id, level, fruits_today, can_be_found, stats}, `exploration`
(`Exploring{fights_won,next_fight_lvl}`/`Finished`), `fruits`, `opponent`, `atr_bonus`). Commands: `ViewPet{pet_id}`,
`PetFeed{pet_id, fruit_idx}` (`PlayerPetFeed:id/idx`), `FightPetDungeon{use_mush, habitat, enemy_pos = explored+1, player_pet_id}`,
`FightPetOpponent{habitat, opponent_id}`, `BrewPotion`. Simulation feature has `get_exploration_enemy` / `pet_to_fighter`
-> could predict the win chance for a level. None of it is in `safe::is_allowed` yet; each command must be verified mushroom-free
first (`use_mush` is ignored by the server per the docs, still send false).

## Open questions (ask / verify after the unlock)
- Is `fruit_idx` the habitat index or a stack index? Does feeding ever cost mushrooms?
- Do dungeon/PvP/trainer fights have a cooldown and does a free one exist (`next_free_battle`, `next_free_exploration`)?
- Real element disadvantage numbers; how a pet is "found" (expedition location + time conditions; command or automatic?).
- Where trainer attacks live (probably Underworld/other command) and what they give.

## Answers from the user (2026-10-20, still assumptions until seen live)
- PvP vs players: 5 a day, one per element.
- Habitat dungeon: probably a ~1 h cooldown (unverified; sf-api has `next_free_exploration`).
- Finding a pet: the condition (e.g. mushroom event + visiting Skull Island on an expedition) must be met, then the chance
  depends on rarity: common (positions 1-14) = 100 %, rare (15-18) = some chance, epic (19-20) = small chance.
  So the chain's late targets (Poisnake/Unikor/Mouthrexor/Devastator/Hydrospir, position 20; Pinklynx/Dragopyre/Knilight/
  Watnake/Lilbeatzup 14-18) may need repeated expeditions after beating them.
- fruit_idx meaning and trainer attacks: unknown, check after the unlock.

## First capture (2026-10-21 00:05, main character, right after the unlock; `ownpets.petsSave`, 0-based indexes)
- The hatch gave 5 pets (one per habitat): Shadow #1, Light #3, Earth #2, Fire #3, Water #1, each level 1 (pet level of pet n at
  index n+1, total collected at 103 = 5; matches sf-api). `unlockfeature:9/1`, `petsHatchIndex:81`.
- The tail differs from sf-api: non-zero values at 242 (5743), 244 (3383), 245 (100), 246/247 (2), 248 (a timestamp 2 h in the
  future: probably the free habitat-dungeon/PvP timer), 249..253 (1 x5), 261..265, 275. sf-api expects explored at 210+e, battled
  223+e, opponent 231..237, next-level 238+e, attribute bonus 250+; with `rank` (233) = 0 it throws the whole pets state away
  (`gamestate/mod.rs` "pets.rank == 0 -> pets = None"), so `gs.pets` may be None on the bot until a rank exists. Layout unresolved.
- Phase 1 is implemented (`src/pets.rs`, `SimpleSession::pets_save`): the raw numbers are written to `roster/<char>/pets_raw.txt` and
  `[pets] collected N: <habitat> [#pos=level …] | tail idx:value …` is logged whenever they change. Use these lines (before/after
  feeding, a dungeon fight, a PvP fight) to decode the tail; nothing is sent to the server yet.
