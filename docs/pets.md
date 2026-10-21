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
