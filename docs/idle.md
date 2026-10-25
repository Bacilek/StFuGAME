# Arena Manager (idle game)

Unlocked at character level 105 (user 2026-10-25, TestChar-level capture; **no unlock message was seen**, so we do not
know whether an `UnlockFeature` is pending like with Pets). Not implemented, nothing whitelisted in `safe.rs`.

sf-api 0.4.4 knows it as `gamestate::idle::IdleGame` (`gs.idle_game`): money, runes, resets, sacrifice runes, merchant
timer and 10 buildings (`IdleBuildingType`: Seat, PopcornStand, ParkingLot, Trap, Drinks, DeadlyTrap, VIPSeat, Snacks,
StrayingMonsters, Toilet). Commands: `IdleUpgrade { typ, amount: One|Ten|TwentyFive|Hundred|Max }` = `IdleIncrease:<typ>/<amount>`
(`Seat` = 1 ... `Toilet` = 10, amount -1 = max) and `IdleSacrifice` = `IdlePrestige:0` (money -> runes, resets the game).
The in-game currency is "dollars", it is NOT gold and NOT mushrooms.

## Capture 2026-10-25: Seat 1 -> 2 for 6 dollars

Command `IdleIncrease:1/1`, response `idle.idlesave` (118 values, timestamp 1792951774). Decoded (sf-api indexes):

| index | value | meaning |
|---|---|---|
| 1 | 23994 | unknown |
| 2 | 0 | resets (prestiges) |
| 3..12 | 2,0,... | building levels: Seat 2, the rest 0 |
| 13..22 | 2,0,... | earning per cycle: Seat 2 dollars |
| 23..32 | 1792951766 / 0... | cycle start (Seat) |
| 33..42 | 1792951838 / 0... | cycle end (Seat): 72 s cycle = "1 min 12 s" in the UI |
| 53..62 | 0 | golden flags |
| 63 | 1792972801 | merchant new goods time |
| 64 | 1792972799 | unknown (about the same time) |
| 65..70 | 1/23/21/25/25/15 | unknown |
| 72 | 0 | current money (6 spent, 0 left) |
| 74 | 6 | `_current_money_2` (sf-api: "slightly larger than money") |
| 78..87 | 7, 100, 2500, 50000, 1e6, 25e6, 5e8, 1e10, 2.5e11, 5e12 | cost of the NEXT single upgrade per building (Seat 7 after the upgrade; the other nine are their level-0 prices) |
| 88..97 | 115, 1171, 28676, ... | cost of x10 |
| 98..107 | 475, 3835, 91295, ... | cost of x25 |
| 108..117 | 12519, 68933, 1525840, ... | cost of x100 |

Facts: Seat level 1 = 1 dollar / 72 s, level 2 = 2 dollars / 72 s, next level costs 7 (UI: "next upgrade costs 7").
Cost of the first upgrade was 6.

## Open questions (ask the user before building anything)

- Strategy: buy the best dollars-per-cost building continuously? When to `IdleSacrifice` (prestige gives runes, loses all
  progress)? What do runes buy? What does the merchant (new goods at index 63) sell, and does anything cost mushrooms?
- Does collecting the dollars need a command, or are they credited by time only (idx 72 grows by itself)?
- Which commands cost mushrooms (merchant? speed-ups?) - none may be whitelisted until verified.
