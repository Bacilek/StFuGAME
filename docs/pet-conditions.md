# Pet catch conditions (user, read in the game, 2026-10-21)

Source of truth for where/when a pet drops in expeditions (replaces the unreliable sfporadnik table in `docs/pets.md`).
Format: position name: time condition, expedition location. "day" = 06:00-16:00 (user 2026-10-23, was assumed 06-18), "night" = presumably the rest (18-06 assumed, unverified). Season entries (spring/summer/fall/winter) mean whole days of that season, not a time of day.
Positions 15-20 (and seasonal ones) are "hard" conditions: a specific season/event/date/achievement instead of a plain time of day; these are the "hard to get" pets of the egg priority in `precedents.md`.
Only pets unlocked in the habitat dungeon (first 3 from the start) can be found. Filled habitat by habitat; not yet in code.
The last pet (20) of every habitat has only a location condition (no time); it requires having beaten all other pets of the habitat (user 2026-10-23).

## Shadow
1. Slurp: day, Moldy Forest
2. Digmol: night, Rotten Lands
3. Toothey: night, Sprawling Jungle
4. Okultacle: night, Moldy Forest
5. Spidor: Monday night, Rotten Lands
6. Jacobu: 00-01, Black Water Swamp
7. Shrimpfly: summer days, Sprawling Jungle
8. Reaprim: winter days, Rotten Lands
9. Petdacat: Friday night, Shadowrock Mountain
10. Mykon: fall days, Moldy Forest
11. Fishorr: Monday night, Black Water Swamp
12. Cuckooly: Tuesday night, Rotten Lands
13. Battlutter: Wednesday night, Shadowrock Mountain
14. Pinklynx: Thursday day, Shadowrock Mountain
15. Pharamumm: Crazy Mushroom Harvest event, on Halloween, Rotten Lands
16. Ninstarr: top 1000 in the Hall of Fame or 50,000 honor when finishing the quest, Black Water Swamp
17. Luchtablong: Exceptional XP event, Shadowrock Mountain
18. Angrack: 3 days before and after Friday the 13th, Rotten Lands
19. Devilsatt: after clearing floor 10 of dungeon 15 in the Shadow World, Black Water Swamp
20. Poisnake: after beating all pets of the Shadow habitat, Sprawling Jungle

## Light (user 2026-10-23)
1. Shaggyl: day, Plains of Oz'Korr
2. Jellclops: night, Busted Lands
3. Tinck: day, Maerwynn
4. Cloudning: day, Busted Lands
5. Nevorfull: spring nights, Maerwynn
6. Plutoid: Saturday day, Busted Lands
7. Djinntonic: summer days, Sunburst Desert
8. Blaxta: fall days, Plains of Oz'Korr
9. Lampcess: Monday night, Sunburst Desert
10. Teslarr: Tuesday day, Busted Lands
11. Sunnya: Sunday day, Sunburst Desert
12. Buckfoxion: Wednesday night, Maerwynn
13. Birdychirp: Thursday day, Plains of Oz'Korr
14. Eyeorwhat: Friday day, Busted Lands
15. Mesmerit: guild Hall of Fame top 100 or honor 2500, Sunburst Desert
16. Antlar: December, Shadowrock Mountain
17. Liphant: 3 days before and after April Fools' Day, Busted Lands
18. Knilight: Epic Shopping Spree Extravaganza event, Plains of Oz'Korr
19. Heraldon: after clearing the Tower, Maerwynn
20. Unikor: after beating all pets of the Light habitat, Plains of Oz'Korr

## Earth (user 2026-10-23)
Seasons = whole days of the season (user 2026-10-23).
1. Mamoton: day, Gnarogrim
2. Monkorrage: night, Sprawling Jungle
3. Smaponyck: day, Maerwynn
4. Bittnutz: day, Plains of Oz'Korr
5. Roarear: night, Gnarogrim
6. Muscudon: summer, Plains of Oz'Korr
7. Apstick: fall, Sprawling Jungle
8. Horrnington: winter, Gnarogrim
9. Boaringg: spring, Maerwynn
10. Mameloth: Sunday night, Sprawling Jungle
11. Rheynooh: Monday day, Maerwynn
12. Rockastonn: Tuesday night, Gnarogrim
13. Redwoofox: Wednesday day, Maerwynn
14. Lilbeatzup: Thursday day, Plains of Oz'Korr
15. Forror: Easter legendary dungeon, Maerwynn
16. Nipprabs: Pentecost/Whitsun event, Sprawling Jungle
17. Armoruck: Glorious Gold Galore event, Plains of Oz'Korr
18. Canocle: fortress Hall of Fame top 100 or honor 2500, Gnarogrim
19. Tricerawood: found in the Gem Mine after Gem Mine level 20 (not an expedition location)
20. Mouthrexor: after beating all pets of the Earth habitat, Sprawling Jungle

## Fire (user 2026-10-23)
1. Firimp: day, Moldy Forest
2. Gullps: night, Magmaron
3. Pyrophibus: night, Black Water Swamp
4. Flamechirr: day, Sunburst Desert
5. Tectospit: Tuesday night, Magmaron
6. Pyroplant: fall, Moldy Forest
7. Kokofire: Monday day, Magmaron
8. Peppryon: spring, Sunburst Desert
9. Boomywoomy: winter, Magmaron
10. Tikiricky: Wednesday night, Sprawling Jungle
11. Matchlit: Thursday day, Sunburst Desert
12. Birdblazey: summer, Sunburst Desert
13. Infernox: Friday day, Magmaron
14. Hambuzzish: Saturday night, Sprawling Jungle
15. Dragopyr: found in the Weapon Shop (not an expedition location)
16. Mantiflame: Tidy Toilet Time event, Sunburst Desert
17. Finnettle: 3 days before and after Valentine's Day, Magmaron
18. Etrock: New Year's Eve and Day, Plains of Oz'Korr
19. Blazingtongues: after clearing the Demon Portal, Magmaron
20. Devastator: after beating all pets of the Fire habitat, Gnarogrim

## Water (user 2026-10-23)
1. Goldy: day, Skull Island
2. Orcahle: day, Skull Island
3. Ocodile: night, Black Water Swamp
4. Penguwater: day, Shadowrock Mountain
5. Walrophin: Monday day, Shadowrock Mountain
6. Colsnail: Tuesday night, Moldy Forest
7. Aquaphant: Wednesday day, Moldy Forest
8. Naar: winter, Shadowrock Mountain
9. Octoboss: Thursday night, Moldy Forest
10. Ewigryn: Friday day, Skull Island
11. Seapard: Saturday day, Skull Island
12. Shellzy: fall nights, Skull Island
13. Mingho: summer, Black Water Swamp
14. Angbite: Sunday night, Shadowrock Mountain
15. Mermoid: pet Hall of Fame top 100 or 4000 pet honor (no location given)
16. Watsnake: found in the Magic Shop (not an expedition location)
17. Cannoturtle: summer legendary dungeon during the S&F anniversary event, Skull Island
18. Ungere: found in the Arcane Toilet, toilet level 50+ (not an expedition location)
19. Tritosting: after clearing the 22nd Light dungeon, Moldy Forest
20. Hydrospir: after beating all pets of the Water habitat, Black Water Swamp
