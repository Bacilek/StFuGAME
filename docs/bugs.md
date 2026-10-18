# Bug hall of fame (funny / strange behaviour of the characters)

The user wants a list of the strangest things the characters did, to go through at the end of the challenge (user
2026-10-18). **Append an entry whenever a bug, a loop or a silly behaviour is found or fixed** (date, character, what it did,
cause, fix, status). Newest first. Kept in English like the rest of the repo.

## 2026-10-18 – Květoš swapped two weapons ~2450 times
- **What:** from 03:57 to 12:29 the character equipped weapon A, then B, then A, ... every ~10 s (~300 an hour). Every pass of
  the inventory module ran into the "Action limit for one pass" and the Tavern round after it waited ~5 min, so expeditions took
  1 h 40 min instead of 15 min and the day's Thirst for Adventure was still not used up at 12:30.
- **Cause:** a socketed main/CON gem was IGNORED in an item's value but INCLUDED in the baseline build (`raw_totals`), so the worn
  weapon was always punished by its own gem and the other one looked better. Also a weapon's damage used the main attribute total
  of the character WITH THE WORN weapon.
- **Fix:** socketed main/CON gems count fully; damage uses the total with the candidate weapon (`total_with`). Release 12:29.
- Same bug had hit **Filminy** on 2026-10-17 00:00–10:00 (~2700 swaps, between an epic weapon and a Luck weapon); it stopped by
  itself after the 11:56 release.

## 2026-10-18 – Mrožik traded +43 epic gloves with a +75 CON gem for +46 gloves without a slot
- Same root cause as above (gem ignored in the comparison). Fixed by the same change; it should swap back by itself.

## 2026-10-18 – "UsePotion failed 10 times: no potion downgrade" popup (Mimimimi11, PajaRizz, Filminy overnight)
- **What:** with a full backpack the bot tried every few minutes to drink a 10 % potion while a 25 % one of the same kind was
  active; the server refuses downgrades.
- **Cause:** a rule "drink the smaller one when the bigger active has <= 3 days left" that nobody tested against the server.
- **Fix:** a smaller potion replaces the bigger active one only by removing it first, only when it has < 24 h left.

## 2026-10-18 – Gone With the Wind poster showed up as an "unknown situation" popup on 7 characters
- Raw id 1019 (the mission's poster, +0, doubles the completion bonus) was not in `KNOWN_UNNAMED_IDS`.

## 2026-10-17 – Mimimimi11 left a better guild for a worse one
- Own guild totals were read from a stale cache; fixed by reading them fresh (`ViewGuild`) before every switch.

## 2026-10-16 – Helmets "+1100 %" better than what was worn
- The dashboard credited an item with an EMPTY gem slot with the best gem we had, so junk-stat items beat real upgrades.

## 2026-10-16 – PajaRizz skipped a dungeon with a full backpack
- Four loose Luck gems and an epic filled the backpack; the dungeon needs a free slot. Led to "always keep one slot free".

## Smaller oddities noticed (not fixed / not bugs)
- 2026-10-18: the attribute price table drifted at high levels (Mimimimi11 Strength: expected 719 g, paid 724 g) because our own purchase counter was a guess beyond the 216-level table; the server sends the real counter (`attribute_times_bought`), now used (fixed).
- 2026-10-18: most characters sat with a finished gem in the Gem Mine for hours because collecting needed 2 free slots (fixed: 1 slot is enough, an unwanted gem is sold).
- 2026-10-18: "not 24 hours member" guild sign-up errors on almost every character at the same time (retried every 4 h, expected).
- 2026-10-18: Mrožik lost 11 arena fights today (the others 3–7) while winning the capped 10; reason not examined.
