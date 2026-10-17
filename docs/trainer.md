# Inventory trainer (user 2026-10-17)

Goal: the user cannot put the inventory rules into words, but can always pick the right move. The app tab **Inventory trainer**
(`src/app.html`, `TR` block) shows generated scenarios; the user clicks the correct move; Claude later derives the rules from the
answers and writes them into the bot (`inventory.rs`, `gems.rs`, `potions.rs`).

## How it works
- Scenario number `n` is generated deterministically from `n` (seeded RNG in JS), so the same number is always the same scenario and
  any answer can be replayed against the bot's logic. Endless; the tab goes to the next unanswered number (progress also in
  `localStorage` key `trainerAnswers`, which may be lost – the file below is the source of truth).
- A scenario: character (class, level, main/CON/LCK, CON as % of main), worn items (9 slots, normal or epic, gem slot none/empty/filled),
  ACTIVE POTIONS WITH DAYS LEFT (a big potion with 1 day left is worth far less than with 10 – user), backpack of 15 slots (epics, loose
  gems, potions, normal items, items waiting for a gem, free slots 0–8), and one NEW thing: a normal item, an epic, a potion, a gem, or
  a gem ready in the mine.
- Moves: equip (sell old / keep old), keep in the backpack, sell, socket / replace a gem into a chosen slot, drink, remove an active potion
  and drink, keep and sell a chosen backpack entry to make room, leave the gem in the mine, "I don't know / depends" + a free note.
- Answers: appended by `app.rs::save_trainer_answer` to **`roster/trainer.jsonl`** (local only): one JSON per line with `n`, `ts`,
  `choice {action, target, targetLabel}`, `note`, and the whole `scenario`.

## Rules heard so far (2026-10-17, to be confirmed by the answers)
- Gems: holding more than 1 gem is pointless when there is no room; with room a stock is fine; with 1 gem held keep mining and swap it
  unless it is a CON/main gem (depends on what is needed right now).
- Potions: most of the game main + CON + Luck potions are worn; Eternal Life is rare and almost never without mushrooms (only gold buys).
  The main kinds (maybe not Luck) should never be sold, least of all the 25 % big ones (found from level 35).
- Not yet a bot change: the potion rework of 2026-10-17 sells a smaller potion of a kind when a bigger one is active – check against the answers.

## Next steps
1. User answers scenarios (the more the better). 2. Claude reads `roster/trainer.jsonl`, groups by scenario family, lists the rules +
contradictions, asks about the contradictions. 3. Rules go to `docs/precedents.md` and the code; optionally a "what the bot would do now"
column to expose differences.
