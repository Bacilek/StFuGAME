# Game of Dice (Gambler, Tavern)

Status: NOT implemented (2026-11-06). Rules from the user + first captures below; strategy questions are open (see the end).

## Rules (user 2026-11-06)
- Unlocks together with the Tower (character level 125 AND Light Dungeon 1–9 cleared).
- One free game every hour (cooldown like a dungeon), at most 10 games a day (20 during Lucky Day, `docs/events.md`).
- Skipping the wait: `RollDicePrice::Hourglass`. User's wish: after 23:00, if games are left that day, pay hourglasses
  so that all of the day's games get played. NEVER `RollDicePrice::Mushrooms` (whitelist only `Free` and `Hourglass`).
- A game: first roll of 5 dice (all `ReRoll`), then one more roll in which we hold some dice and re-roll the rest.
  Only 3+ equal dice pay out. Dice types (`sf_api::command::DiceType`): ReRoll 0, Silver 1, Stone 2, Wood 3, Souls 4, Arcane 5, Hourglass 6.
- Events change the payout: Glorious Gold Galore (5× gold), Fantastic Fortress Festivity (2× wood/stone), Days of Doomed Souls (2× souls),
  Forge Frenzy (2× arcane splinters), Epic Good Luck (epic chance), Witch Talisman enchant (+10 % arcane splinters and dice).

## Protocol (sf-api, captured 2026-11-06 on the user's character)
- Command `RollDice { payment, dices: [DiceType; 5] }` → wire `RollDice:<payment>/<d1>/<d2>/<d3>/<d4>/<d5>`; `0` = re-roll that die, otherwise hold that type.
- Response keys: `dicestatus:a/b/c/d/e` (the 5 dice on the table, `DiceType` numbers), `dice:<next_free_ts>/<games_left>`
  (`tavern.dice_game.next_free`, `.remaining`), `dicereward:<type+1>/<amount>` (`tavern.dice_game.reward`, type = value − 1), `resources:` updated.
- Capture 1 (first roll paid with an hourglass): `dicestatus:6/1/6/3/1` = Hourglass, Silver, Hourglass, Wood, Silver (user: 2× gold, 2× hourglass, 1× wood);
  `dice:1793926883/19` with `timestamp:1793923882` → next free game in 3001 s (~50 min); 19 games left (Lucky Day, 20 a day).
- Capture 2 (held the 2 hourglasses, re-rolled the other 3): `dicestatus:6/3/6/3/3`, `dicereward:4/4968` = 3× Wood, +4968 wood
  (`resources` wood field 1514078 → 1519046), `dice:…/18`. 2 hourglasses are no win, so the user took the wood.
- An `idle.idlesave` response was pasted between the two; it carried no dice keys (probably the Arena Manager updating in the background).
- NOT captured yet: the exact request strings the client sent (how many re-rolls a game has, whether a further re-roll costs an hourglass, how a win is "taken"),
  the payout table (3/4/5 of a kind per type), and the response when no game is left.

## Verification status
| Thing | State |
|---|---|
| `dicestatus`, `dice`, `dicereward` parse in sf-api | ✅ captures above |
| Reward for 3× Wood | ✅ 4968 (with Fortress Festivity? unknown) |
| Re-roll count per game, price of extra re-rolls | ⏳ ask / capture |
| How to take the win | ⏳ capture |
| Payout table | ⏳ collect from journal once implemented |
| Unlock detection (what the server sends before the Tower) | ⏳ |

## Open questions for the user (strategy)
1. Which type to hold when the first roll has several pairs / what is each type worth (Arcane, Souls, Hourglass, Silver, Stone, Wood)?
2. May extra re-rolls cost hourglasses, or only the free ones?
3. Hourglass reserve (do not skip waits below N hourglasses)? Skipping a wait costs how many hourglasses (1?).
4. Play from level 125 only when the Tower is open, or detect by the `dice` key in the response?
