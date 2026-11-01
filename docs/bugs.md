# Bug hall of fame

Funny, character-specific misadventures, for the end of the challenge (user 2026-10-18). Rules for entries:
- Only bugs that hit ONE to THREE named characters. No generic/global bugs, no pure code fixes.
- Only write-worthy ones, so few entries. The funnier and more concrete the better, with the consequence in numbers.
- Short: a headline, a line or two on what it did and why. Newest first. In English like the rest of the repo.

## Květoš and Filminy, the weapon-swap champions (2026-10-17/18)
Because of one gem ignored in the item comparison, Květoš swapped two weapons ~2450 times (~300 an hour) from 03:57 to 12:29. Every
swap ate the inventory's action limit, so each Tavern round waited 5 min and a 15-min expedition took 1 h 40 min. Filminy did the
same the day before (~2700 swaps between an epic weapon and a Luck one) and stopped only after a release.

## Mrožik swapped +43 epic gloves with a +75 CON gem for +46 gloves with no slot (2026-10-18)
Same blind spot: the gem in the gloves was worth nothing in the comparison, so the "upgrade" was a downgrade.

## Mimimimi11 left a better guild for a worse one (2026-10-17)
She read the own guild's totals from a stale cache, saw the new guild as better, and moved. Now the totals are read fresh first.

## PajaRizz skipped a dungeon because the backpack was full of gems (2026-10-16)
Four loose Luck gems and an epic filled all seven slots, and a dungeon fight needs a free one. Hence "one slot always stays free".

## Pagan swapped two weapons 1361 times in one day (2026-11-01)
A strong plain weapon (119-257) and a weaker one with an All gem (105-175) each beat the other depending on which was worn, because the damage gain was divided by the WORN weapon's damage. One swap every ~10 s from 00:00; the loop ate every inventory pass, so loot piled up, his backpack looked full and the Tavern reward alert fired. Now both are judged against the mean damage (`inventory::compare`, test `two_weapons_do_not_swap_forever`).

## Pagan dismantled 13 epics he was saving for the toilet (2026-11-01)
The same weapon loop kept the junk in his backpack unsold, so "one slot must stay free" dismantled an epic every 5 minutes between 00:14 and 01:20 (12 epics in that hour plus 1 on 30 Oct, about 4 300 splinters in total). The loop is fixed, so the junk is sold and epics go only when no slot is free.
