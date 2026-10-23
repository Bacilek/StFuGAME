# Pets (briefing 2026-10-20, user; NOTHING implemented yet)

Source: the user's memory of a 2023 tutorial video (never played Pets himself, everything below is an assumption until verified)
+ https://en.sfporadnik.pl/pets.php. Unlock: level 75, then the first finished Tavern expedition unlocks Pets (user 2026-10-23: no Egg Basket needed; my earlier "Egg Basket" note was wrong). Main character
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

## Captures 2026-10-21 (user, main character, lvl 76) – what is decoded
Starting pets (one per habitat, level 1): Shadow Slurp (#1), Light Tinck (#3), Earth Monkoragge (#2), Fire Pyrobhibus (#3), Water Goldy (#1). 13 fruits of every habitat at the start.
- **Fruits** are in the `resources:` key, indexes 12..16 = Shadow, Light, Earth, Fire, Water (13/13/13/13/13; one lemon (Light) -> 12). Fruit kinds: Shadow = blackberries, Light = lemons, Earth = apples, Fire = strawberries, Water = plums.
- **Feeding** (`PlayerPetFeed:<pet_id>/<fruit_idx>`; Tinck = pet_id 23): level of Tinck 1 -> 2 (`petsSave` index 24), Light fruits 13 -> 12, a `ownpetsstats.petsStats:` line comes back (`23/2/20/2/18/18/36/36/27/0/0/1/1/1/13/13/0` = id, level, armor, class, 5 attributes, 5 bonus attributes, min/max damage, element). The `fruit_idx` used by the game is still unknown (our guess: habitat index, Light = 1 or 2); first call must be tested on one pet. (Later the same pet showed level 4 with Light fruits 11 after the dungeon fight below, so a fight or a second feed gave more than +1 – unclear.)
- **`petsSave` tail (0-based), decoded from the 3 captures:** 242 = pet PvP opponent id (5743 = Doba87, lvl 76, Light pets), 243 = next free battle (0 = free), 244 = our pet rank (3383 -> 3384 after the feed -> 3387 after the fight), 245 = pet honor (100), 246 = opponent pet count (2), 247 = opponent level total (2), 248 = timestamp (opponent chosen/reroll time), 249..253 = per-habitat explored counter (1,1,1,1,1; Light became 3 after beating Shaggyll #1), 261..265 = five values (2257/305/305/1091/307, +1 on Light/Dex after the feed; probably attribute bonuses, part of the character save too), 275 = counter (5 -> 6 after the feed -> 8 after the fight). So sf-api's pets block is shifted by +11 (opponent..reroll) and more for the rest; sf-api's `rank` index 233 is 0, so `gs.pets` is None on the bot. `src/pets.rs` reads the raw numbers instead.
- **PvP view** (`PlayerLookAt`-like `otherplayer...` keys, includes `otherplayerpetbonus.petbonus:5743/1/2/2/1/2` = the opponent's per-habitat pet bonus %?) shows the opponent's pets in the UI; under it five numbers = the sum of our pet levels per habitat (1,1,1,1,1).
- **Habitat dungeon fight** (`PetsDungeonFight`): Tinck (lvl 4) beat Shaggyll (Light #1) and the response `fightresult.battlereward:1/1/119920/240784/…` gave **gold +1199.20 g (119920) and XP +240,784** (the character's XP went 62,351 -> 303,135 of 1,203,920 needed for the next level = **20 % of a level 76 per fight**) plus an item (`battlerewarditem`), cooldown **1 h, shared by all habitats** (user). The user's plan: beat the weakest unopened pet, then catch it with an expedition at the right place/time ("Shaggyll: Plains of Oz'Kor during the day, i.e. after 06:00").
- **This is worth automating early:** one free fight an hour (if it really is free of mushrooms) is a lot of XP and gold; the open points are which habitat/pet to send (chain targets, `FightPetDungeon{use_mush:false, habitat, enemy_pos = explored+1, player_pet_id}`; Light = habitat 2 in the command), win chance (a fought level-4 Tinck beat pos 1), and the exact cooldown field (probably in the tail; compare the 1 h timer).

### Clarified by the user (2026-10-21)
- The item of a habitat dungeon fight is a fruit of that habitat (lemon for Light): Light fruits 13 - 3 feeds + 1 won = 11, and Tinck 1 + 3 feeds = level 4. So the "unclear extra level" is solved: the user fed Tinck the daily maximum of 3 (he is the Light chain's first target, `Tinck (3) [82]`).
- The habitat dungeon is free once an hour, like the normal Dungeons (never for mushrooms). Together with the 24 possible fights a day this is also a fruit source (1 fruit of the fought habitat per win), besides expeditions.
- `fruit_idx` very likely selects one of the 5 fruit types (the user's guess); still to be tested on one pet.
- Shadow (user 2026-10-21): his level-1 Slurp (#1) lost a habitat dungeon fight against the level-1 Slurp of the dungeon (a coin-flip fight at equal level; he does not want to level Slurp since it is not on the Shadow chain). Plan: get through the first pets until #3 Toothey (Shadow chain target `Toothey (3) [45]`), and feed only him. Open: does a lost fight still start the 1 h cooldown and give anything (rewards, fruit)? Capture the response of a loss; the bot must treat a loss as normal (no retry loop within the hour).
- Confirmed (user 2026-10-21): a LOST habitat fight also starts the 1 h cooldown and gives no reward, like the normal Dungeons. Each habitat has one "active" pet the user can select; the bot should always fight with the strongest pet of that habitat that it feeds (the active one). Shadow only has the weakest pet for now, so Shadow fights are coin flips until Toothey is caught.

## Catch conditions per pet (sfporadnik.pl/pets.php via a summarising fetch 2026-10-21 – **UNRELIABLE, DO NOT USE**)
**Superseded by `docs/pet-conditions.md` (user-verified, filled habitat by habitat).**
The user checked in game: Tinck is in Maerwynn (table says Erogenion), Smaponyck in Maerwynn (table: Erogenion), Ocodile in Black Water Swamp
(table: Evernight Forest), Gullps in Magmaron (table: Flooded Caldwell). Locations/times below are wrong or shifted; real ones come from the
game (user) and must be verified, then this table fixed.
Format: position name: expedition location, time. D = daytime (06-18), N = night (18-06). Positions 1-14 common (100 %), 15-18 rare, 19-20 epic.
- **Shadow:** 1 Slurp Nevermoor D; 2 Digmol Maerwynn N; 3 Toothey Sprawling Jungle N; 4 Okultacle Split Canyon N; 5 Spidor Black Forest Mon N;
  6 Jackobu Black Water Swamp 00-01; 7 Shrimpfly Evernight Forest summer D; 8 Reaprim Skull Island winter D; 9 Petdacat Shadowrock Mountain Fri N;
  10 Mykon Moldy Forest fall D; 11 Fishorr Flooded Caldwell Mon N; 12 Cuckooly Tusk Mountain Tue N; 13 Battlutter Erogenion Wed N; 14 Pinklynx Stumble Steppe Thu D;
  15 Pharamumm Black Forest (Halloween event); 16 Ninstarr Gnarogrim (HoF top 1000 / 50k honor); 17 Luchtablong Busted Lands (Exceptional XP event);
  18 Angrack Rotten Lands (Friday 13th); 19 Devilsatt (Shadow World dungeon 13 floor 10; Plains of Oz'Kurr); 20 Poisnake after the last Shadow pet, Sunburn Desert.
- **Light:** 1 Shaggyll Stumble Steppe D; 2 Jellclops Moldy Forest N; 3 Tinck Erogenion D; 4 Cloudning Shadowrock Mountain D; 5 Nevorfull Sprawling Jungle spring N;
  6 Plutoid Nevermoor Sat D; 7 Djinntonic Sunburn Desert summer D; 8 Blaxta Flooded Caldwell fall D; 9 Lampcess Skull Island Mon N; 10 Teslarr Busted Lands Tue D;
  11 Sunnya Tusk Mountain Sun D; 12 Buckfoxion Northrunt Wed N; 13 Birdychirp Evernight Forest Thu D; 14 Eyeorwhat Black Water Swamp Fri D;
  15 Mesmerit Erogenion (Guild HoF); 16 Antlar Northrunt December; 17 Liphant Split Canyon April 1st; 18 Knilight Erogenion (Epic Extravaganza event);
  19 Heraldon after the Tower, Rotten Lands; 20 Unikor after the last Light pet, Plains of Oz'Kurr.
- **Earth:** 1 Mamoton Stumble Steppe D; 2 Monkorrage Gnarogrim N; 3 Smaponyck Erogenion D; 4 Bittnutz Black Forest D; 5 Roarear Northrunt N; 6 Muscudon Tusk Mountain summer D;
  7 Apstick Black Water Swamp fall D; 8 Horrnington Split Canyon winter D; 9 Boaringg Magmaron spring D; 10 Mameloth Nevermoor Sun N; 11 Rheynooh Maerwynn Mon D;
  12 Rockastonn Busted Lands Tue N; 13 Redwoofox Evernight Forest Wed D; 14 Lilbeatzup Rotten Lands Thu D; 15 Forror Maerwynn Easter; 16 Nipprabs Sprawling Jungle Pentecost;
  17 Armoruck Plains of Oz'Kurr (Glorious Galore event); 18 Canocle Stumble Steppe (Fortress HoF); 19 Tricerawood Gem Mine lvl 20; 20 Mouthrexor after the last Earth pet, Moldy Forest.
- **Fire:** 1 Firimp Nevermoor D; 2 Gullps Flooded Caldwell N; 3 Pyrophibus Black Water Swamp N; 4 Flamechirr Tusk Mountain D; 5 Tectospit Busted Lands Tue N;
  6 Pyroplant Moldy Forest fall N; 7 Kokofire Split Canyon Mon D; 8 Peppryon Sunburn Desert spring D; 9 Boomywoomy Plains of Oz'Kurr winter D; 10 Tikiricky Sprawling Jungle Wed N;
  11 Matchlit Stumble Steppe Thu D; 12 Birblazey Black Forest summer D; 13 Infernox Magmaron Fri D; 14 Humbuzzish Evernight Forest Sat N (rare); 15 Dragopyr weapon shop;
  16 Mantiflame Maerwynn (Tidy Toilet Time event); 17 Finnettle Flooded Caldwell Valentine's; 18 Etrock Evernight Forest New Year; 19 Blazingtongues after Demon Portal, Skull Island;
  20 Devastator after the last Fire pet, Gnarogrim.
- **Water:** 1 Goldy Magmaron D; 2 Orcahle Skull Island D; 3 Ocodile Evernight Forest N; 4 Penguwater Northrunt D; 5 Walrophin Rotten Lands Mon D; 6 Colsnail Tusk Mountain Tue N;
  7 Aquaphant Split Canyon Wed D; 8 Naar Flooded Caldwell winter D; 9 Octoboss Moldy Forest Thu N; 10 Ewilgryn Skull Island Fri D; 11 Seapard Black Water Swamp Sat D;
  12 Shellzy Erogenion fall N; 13 Mingho Sprawling Jungle summer D; 14 Angbite Shadowrock Mountain Sun N; 15 Mermoid Skull Island (Pets HoF); 16 Watnake magic shop;
  17 Cannoturtle Skull Island (birthday event 22.06); 18 Unhere Arcane Toilet (Aura 20); 19 Tritosting after Easteros, Busted Lands; 20 Hydrospir after the last Water pet, Nevermoor.
- The page's names differ slightly from the game client (client: Monkoragge, Pyrobhibus) – match by position, not name.

## User finding (2026-10-21, corrected the same day): only the first 3 pets of a habitat are pre-unlocked
- MANY more than 3 pets per habitat drop in expeditions, but a pet can only be found once it is unlocked. The first 3 pets of every habitat are
  unlocked from the start (as if already beaten in the habitat dungeon); all later ones are "shadowed" until beaten in the habitat dungeon.
  So chain targets at position <= 3 (Shadow Toothey, Light Tinck, Earth Smaponyck, Water Ocodile, Fire Gullps #2) can be caught straight from
  expeditions: no dungeon wins, no fruits spent on weak pets. Deeper targets need the dungeon wins first.
- Every character has its own random starter per habitat (user: Tinck is his starter, other characters may not have it).
- Example Shadow: starter Slurp #1, target Toothey #3 -> find Toothey in an expedition (location/time from the game), feed only him.
- Verified locations (user, in game): Tinck Maerwynn, Smaponyck Maerwynn, Ocodile Black Water Swamp, Gullps Magmaron. Times of day still unknown.
- Tavern idea (not implemented, needs the user's decision): prefer an expedition passing a wanted pet's location at the right time.

## Capture 2026-10-21 after feeding Gullps (user, lvl 76; `petsSave` counted from 0 = player id 23994)
- Levels sit at index = pet id + 1: Shadow #3 (id 3) lvl 4 (idx 4), Light Tinck (id 23) lvl 4 (idx 24), Earth #2 (id 42) lvl 1 (idx 43), **Fire Gullps (id 62) lvl 2 (idx 63)**, Fire #3 (id 63) lvl 1 (idx 64), Water #1 (id 81) lvl 1 (idx 82).
- 103 = pets collected (7), 104..108 = per-habitat collected (Shadow 2, Light 1, Earth 1, Fire 2, Water 1).
- **Fruits fed today** = index 109 + pet id: idx 112 = 3 (Shadow #3), 132 = 3 (Tinck), **171 = 1 (Gullps)**. So the daily 3-feed limit is per pet and is readable here.
- Tail in this numbering (10 lower than the earlier note): 232 opponent id (10160), 233 timestamp, 234 pet rank (3376), 235 honor (253), 236/237 opponent pet count/level total (2/2), 238 timestamp, 239/240 = 6/6, 251..255 = attribute bonuses 2083/305/305/1468/313 (equal to the character save), 265 = counter (14).
- Fruits after the feed (`resources` 12..16): Shadow 12, Light 13, Earth 14, Fire 13, Water 13 (total 65 = the start total, so wins refilled what feeding used). A capture from BEFORE the feed is needed to see which stack `PlayerPetFeed:62/<idx>` consumed; still open.
- Confirmed (user 2026-10-21): a WIN in pet PvP (`FightPetOpponent`) also refills 1 fruit, like a habitat dungeon win. Fruit sources: expeditions, habitat dungeon wins (1 h cooldown), pet PvP wins (5 a day, one per habitat).

## Egg rewards in expeditions (user 2026-10-21)
- Going on e.g. a night Black Water Swamp expedition to find Ocodile (Water #3): the mid-expedition reward choice (3 offered, after the 5th encounter) can contain an **Egg** (the pet is found this way).
- Reward order (precedents 2026-10-21): hard-to-get chain pet egg (only rare conditions/events) > mushroom > other pet egg > fruit > gold > rest. An easy chain pet loses to a mushroom.
- Capture while on such an expedition: `expeditionstate:5/21/4/193/101/44/0/1/1/0/1000/0/0/25/1/1792619611/1792619986/10/-10/0/1` (not decoded; compare with sf-api `ExpeditionStage`; how to tell an Egg reward and its pet/habitat is open - needs a capture of the reward screen).

## Pet PvP (user 2026-10-21; strategy is a DRAFT, nothing implemented)
- Up to 5 fights a day, the opponents are offered one at a time (the next is visible only after the previous win/loss). The defender's habitat is the day's element, cycling Water -> Fire -> Earth -> Light -> Shadow -> Water (2026-10-21 = Light). The opponent fights with ALL his pets of that element.
- We choose our side: each of the 5 fights must use a DIFFERENT habitat of ours (one per habitat per day), and all our pets of that habitat fight.
- We see the opponent's pet count and the sum of pet levels of the day's element (`petsSave` 236/237, ie. 2 pets / 2 levels). Only a rough signal: fewer, higher pets beat many low ones with the same sum (3 pets with one lvl 19 + two lvl 1 vs 3 x lvl 7 = 21 levels each; the concentrated one is stronger), and every pet scales differently.
- Strategy idea (corrected below): hopeless -> sacrifice our worst habitat (or the disadvantaged one), keep the best habitats' pets for as long as possible and avoid pointless losses elsewhere. The last fight of the day: fight even if a loss is certain (a loss lowers honor/rank, so tomorrow's opponents are weaker). Win = +1 fruit (see above).
- Open: exact type bonus numbers, how to estimate opponent strength better (`ViewPet` per pet? the `otherplayerpetbonus` key), whether the opponent can be rerolled, what the 5 fights cost/cooldown, honor gain/loss per fight. Needs captures of a win and a loss.

### Correction (user 2026-10-21): the allocation rule
- Type advantage is for FIGHTS WHERE IT WOULD DECIDE the result (close fights), not for easy ones.
- Clearly winnable fight -> send our WEAKEST habitat that still has a high chance to win (sacrifice the weakest sufficient element, keep the strong ones).
- Hopeless fight -> our worst / disadvantaged element (as before).
- Type bonus is probably ~10 % stats or damage (user will check in game; unverified, maybe only one side gets it).
- The user did not see a way to view the opponent's pet collection in the UI; only the pet count and level sum of the day's element are shown (`ViewPet` / `otherplayerpetbonus` were only my guesses, unverified). Honor gain/loss: not interesting, dropped from the open questions.
- **Type advantage = +25 % damage in pet fights** (user 2026-10-21, read in the game; replaces the "~10 %" guess above; unknown whether the other side gets a penalty).

## Phase 2 (2026-10-23, user: "everything correctly implemented is allowed for pets") – feeding + hourly habitat fight
User rules: feeding goes FIRST each day (strongest pets today). Feed only the main chain pet of a habitat, only up to its target level; if the
target is reached before the day limit, the remaining fruits are kept (fruits are a commodity). Pets outside the chain get nothing. A
habitat dungeon coin flip is fine (retry in an hour). Pet PvP vs a player: feed the main pet to the max first (unless it would exceed its level
cap); with no fruit left, wait until all expeditions finished (a fruit may arrive) before attacking. Tavern idea (NOT implemented): fruit above a
random non-chain egg; prefer expeditions to the right location at the right time of day for a pet we lack, and do not burn the whole Thirst
for Adventure at night when all Night pets are caught and a Day pet is missing (wait until 06:00).
Captures 2026-10-23 (Ocodile, Water id 83): feeding 1 plum -> lvl 7 -> 8, "feed max" (event: up to 9 a day) fed 8 at once (8 -> 16), Water
fruits 10 -> 2, so one fruit per level and the fruit_idx addresses the habitat's own stack; fed today at idx `109 + id` = 9. The `petsSave`
layout now matches sf-api: levels at id+1, explored 210+h (next enemy = explored + 1), opponent 231, next free pet PvP 232, rank 233, honor
234, enemy count/level total 235/236, reroll 237, enemy level of the next dungeon enemy per habitat 238+h (Earth 6, Water 14 ...),
attribute bonuses 250+. Habitat dungeon win with Ocodile lvl 16 vs Penguwater lvl 10 (#4): +1785.10 g, +358,411 XP and +3 Water fruits
(event day; `battlerewarditem` type 16 = fruit). The hourly timer is `characterstatus` index 20 (`gs.pets.next_free_exploration`), dungeon timer 21.
Code: `src/pets.rs` (`CHAINS`, `main_pet`, `feed`, `fight`), levels per chain from the user's list above. Level cap = `max(100, maxpetlevel)`.
Verification status: exact `fruit_idx` (0- or 1-based) UNVERIFIED in the bot (self-checking on the first feed, see status.md); daily feed limit learned by a refused feed.

## Provisional restriction + first PvP capture (2026-10-23, user)
- Habitat dungeon fights ONLY while the "Assembly of Awesome Animals" event is active (3x fruits from pet fights; `pets::fights_allowed`, server
  event list via `tasks::event_active`). Feeding is not tied to the event, but the game limits it to 3 fruits per pet per day, 9 during the event (user 2026-10-23). Habitat fights are tied to the event on purpose (user: "we count on it").
- A pet cannot be fed a fruit of another element (user), so the "FEED MISMATCH" guard should never fire; a wrong `fruit_idx` would just be refused.
  Open: the exact `fruit_idx` -> ask the user for the request line of one feed from the browser's Network tab (`PlayerPetFeed:<id>/<idx>`).
- Pet PvP capture (`PetsPvPFight`): Ocodile lvl 16 + Goldy lvl 1 vs a lvl-80 opponent with 3 Fire pets (level total 18). The response holds
  one `fightN.r` per pet duel (4 here, `winneridN.s` = winner of each; ours lost the first duel (Goldy lvl 1), the lvl-16 Ocodile then won the
  remaining three on the damage carried over: the HP left of the previous winner (`19035`) carries on), final result a WIN, `battlereward` +
  `battlerewarditem:16/.../3` = 3 fruits (event day), the pet honor/rank is in `petsSave` 233..234 (rank 3022, honor 505), opponent count/level
  total at 235/236 (3/9 after the fight). So ALL pets of the habitat fight in turn and a strong one can clean up after weak ones. Not implemented.

## STATE OF THE PETS IMPLEMENTATION + WHAT IS NEEDED FROM THE USER (2026-10-23, work paused: the user must first list the weekly events and how they work)
Done (phase 1+2, committed, NOT in a release yet, never seen live): raw `petsSave` logging; feeding of the chain main pet (`pets::feed`,
self-verifying `fruit_idx`, offsets 1 then 0, learned value in `roster/<char>/pets_feed_idx.txt`); hourly habitat dungeon fight ONLY during
the Assembly of Awesome Animals event (`pets::fights_allowed`); whitelist + cooldown guard in `safe.rs`; wake-up in `schedule.rs`.
NOT done: (1) Tavern pet rules, (2) pet PvP, (3) catching logic (what exactly must happen after the egg), (4) habitat fights outside the event.
User answers 2026-10-23:
- Day/night hours are the same for all pets (day 06-18, night 18-06).
- Tavern: an expedition that gives a NEW pet/egg (location + time of day/event/season matches a pet we do not own and can find) always beats the
  other offer; if BOTH offers have such a pet, fall back to the existing `choose_expedition` rules. When only a Day pet is missing, wait until
  06:00 before burning Thirst for Adventure (Night pets all caught). Fruit ranks above an egg of a non-chain pet (precedents 2026-10-21/23).
- Pet PvP: opponent's pet count and level total are always visible; the opponent is re-rolled only by fighting (never on its own, no reroll
  command needed); cooldown between pet PvP fights is 15 min (5 a day, one per habitat).
Needed from the user: (a) `docs/pet-conditions.md` for Light/Earth/Fire/Water (promised "during the day"); (b) captures of an Egg reward
screen (`expeditionstate` before/after the pick) – promised; (c) a capture of a pet PvP LOSS; (d) after the first live feeding: the
`[pets] Fed` lines + `pets_feed_idx.txt`; (e) after the first event fight: the `[pets] Habitat dungeon fight` line; (f) check the chain target
levels in `CHAINS` (`src/pets.rs`) against the game; (g) optional: the `PlayerPetFeed:<id>/<idx>` request line from the browser's Network tab.
Suggested next steps in order: Tavern pet preference (needs (a)+(b)), PvP (needs (c)), then habitat fights outside the event.

## Egg pick capture (2026-10-23, user; need (b) of the list above)
`ExpeditionProceed` after picking the Earth egg as the mid-expedition reward:
`&unlockfeature:21/41/9/1/&expeditionstate:5/16/4/153/191/91/0/0/1/1000/192/152/0/12/0/1792762890/1792763032/5/-5/0/14`
- `unlockfeature` = pairs `main/sub`: `21/41` (unknown, not accepted by the bot) and `9/1` (pending right after the egg pick -> ident 9 is
  PROBABLY the Pets unlock, not only the "beginners task list" guessed on 2026-10-09; `dungeons::ACCEPTED_UNLOCK_IDENTS` already contains 9 and
  `safe.rs` allows it, so `dungeons::run` sends it, but only when it runs - not while the guild-only rule pauses Dungeons).
- The egg itself is not visible in `expeditionstate` (compare the 2026-10-21 capture above: same 21-number shape); the egg reward is picked via
  the normal `ExpeditionPickReward { pos }`. Which reward slot holds an egg is still open.
- Hypothesis for "no `[pets]` on TestChar1/Novotne": the pets unlock was never sent (guildless pause or `dungeons::run` not reached). The login probe
  (`[pets] login response has ...`, 2026-10-23) shows what the server sends; check it after the next start.
- **Second egg capture (2026-10-23, user; blue = Water egg, picked as the FIRST of 3 rewards):**
  `&unlockfeature:21/82/9/1/&expeditionstate:5/2/4/153/112/161/0/1/1/1000/0/0/0/27/1/1792763478/1792763620/5/-5/0/1`
  Together with the Earth capture (`21/41`): `unlockfeature` ident **21 = a new pet, sub = the pet id (1..=100)**. Earth 41 = Earth #1 and
  Water 82 = Water #2, both pets the account did not own yet (it started with Earth #2 Monkoragge and Water #1 Goldy), so the match is
  consistent (2 of 2, not yet more). The element of the egg is therefore readable AFTER the pick from the response, not from the offer.
  CORRECTION: `9/1` is in both captures and also in the very first pets capture, so it is NOT specific to the egg/pets unlock (the earlier
  guess above that ident 9 = the pets unlock is withdrawn; 9 stays "beginners task list"). The unlock of Pets itself is still unexplained.
  Whether `21/<id>` must be sent as `UnlockFeature` to actually get the pet is unknown (the bot does not send ident 21; not whitelisted).
