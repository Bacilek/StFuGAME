# Game of Dice (Gambler, Tavern)

Status: IMPLEMENTED 2026-11-06 (`src/dice.rs`, gates in `src/safe.rs`), waiting for the first real run (⏳ below). Nothing has been played by the bot yet.

## Rules (user 2026-11-06)
- Unlocks together with the Tower (character level 125 AND Light Dungeon 1–9 cleared).
- One free game every hour (cooldown like a dungeon), at most 10 games a day (20 during Lucky Day); the server's `remaining` decides.
- A game = ONE roll of 5 dice, then we may keep any one type and ONE re-roll re-throws the rest. Only 3+ equal dice pay out. The re-roll is free (no hourglass re-rolls exist).
- Skipping the wait: `RollDicePrice::Hourglass`, only on the first roll. From 23:00, when the day's games left do not fit into the free ones before midnight, one hourglass
  per game is paid so all of the day's games get played. No hourglass reserve (user: "na rezervu kašlem"). NEVER `RollDicePrice::Mushrooms`.
- Event Epic Good Luck: an epic can show up in the game and is worth more than everything (user). How it shows is unknown (see below).
- Other events change the payout: Glorious Gold Galore (5× gold), Fantastic Fortress Festivity (2× wood/stone), Days of Doomed Souls (2× souls),
  Forge Frenzy (2× arcane splinters), Witch Talisman enchant (+10 % arcane splinters and dice).

## What the bot does (`dice::run`, once per main-loop pass, after the toilet)
- Unlocked = the server sent a `dice` key (`remaining > 0` or a next-free time); no level check.
- Free first roll as soon as `next_free + 30 s` has passed (`Cooldown::Dice` in `safe.rs`), then the re-roll holding ONE face: an unknown face (the epic guess) first,
  otherwise the most frequent face, ties by value Arcane > Souls > Hourglass > Silver > Stone > Wood (provisional, only "wood uninteresting, gold little" is from the user).
- A face sf-api has no name for (> 6) is sent as `Command::Custom("RollDice")` with the same wire shape (`safe::custom_allowed`, payment 0 or 2 only) and raises a bell alert ("neznámá kostka").
- From 23:00 `dice::skip_with_hourglass`: games left > free games that still fit before midnight (cooldown assumed 3600 s) and hourglasses > 0 → hourglass first roll.
- Each game is logged `[dice] Rolled: … | held … | now … | won N Type | n games left` to progress.log and `inventory.log`. A failed game pauses dice for 15 min.
- `schedule.rs` wakes the character for the next free time and for 23:00.

## Protocol (sf-api, captured 2026-11-06)
- `RollDice:<payment>/<d1>..<d5>`; payment 0 free, 1 mushrooms, 2 hourglass; die 0 = re-roll, else hold that type.
- Faces (`DiceType`): ReRoll 0, Silver 1, Stone 2, Wood 3, Souls 4, Arcane 5, Hourglass 6.
- Response keys: `dicestatus:a/b/c/d/e`, `dice:<next_free_ts>/<games_left>`, `dicereward:<face+1>/<amount>`, `resources:`.
- Capture 1 (first roll paid with an hourglass): `dicestatus:6/1/6/3/1` = Hourglass, Silver, Hourglass, Wood, Silver; `dice:1793926883/19` with `timestamp:1793923882` (3001 s to the next free game, 19 left on a Lucky Day).
- Capture 2 (held both hourglasses, re-rolled 3): `dicestatus:6/3/6/3/3`, `dicereward:4/4968` = 3× Wood, +4968 wood (`resources` idx 5 1514078 → 1519046), `dice:…/18`. The reward arrives in the re-roll response; there is no separate "take" step.
- Capture 3 (user 2026-11-06, Forge Frenzy active): first roll 2× Stone, 1× Wood, 2× Arcane; held the 2 Arcane, re-rolled 3 -> `dicestatus:6/6/5/5/5`, `dicereward:6/778` = 3× Arcane splinters = **778** (2× from Forge Frenzy, so ~389 base); `dice:…/16` (16 left, Lucky Day). Our hold rule picks exactly this (pair of Arcane beats the pair of Stone).
- `resources` idx: 1 mushrooms, 2 gold, 3 lucky coins, 4 hourglasses, 5 wood, 7 stone, 9 metal, 10 arcane, 11 souls, 12–16 fruits.

## Verification status
| Thing | State |
|---|---|
| `dicestatus`, `dice`, `dicereward` parsing | ✅ captures above (unit test) |
| Reward comes with the re-roll response, no "take" step | ✅ capture 2 (assumed the client sent payment 0 for the re-roll ⏳) |
| Re-roll payment value the client sends | ⏳ guessed 0 (Free); check the first `[dice]` lines and the alerts |
| Cooldown between free games 3600 s | ⏳ (capture 1 showed 3001 s ≈ 50 min left) |
| Hourglass skip price (1 per game?) | ⏳ compare `resources` idx 4 before/after |
| Unlock detection (`dice` key absent before the Tower) | ⏳ |
| Epic face id / how the epic shows up | ⏳ watch for the "neznámá kostka" alert |
| Payout table (3/4/5 of a kind per type) | ⏳ collect from the logs |
