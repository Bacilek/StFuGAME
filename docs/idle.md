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

## User decisions 2026-10-25

- Buy the best dollars-per-cost upgrade (confirmed). Prestige timing: see "Prestige rule" below (2026-11-05).
- Dollars are collected by time, automatically (no collect command).
- Merchant sells everything for MUSHROOMS; buyable once a day, then it closes. **Mushroom exception (one time):** exactly ONE
  "speed boost" (25 mushrooms, 2x speed forever on all buildings, survives sacrifice). NEVER time skips or anything else.
- Runes: higher count should improve gear somehow (user does not know yet).

## Capture: merchant purchase (command text `IdleMercantbuy:...`, arguments NOT given)

After it, `idle.idlesave` showed Seat level 3 (earning 3, cost of next 8, money 10) and golden flag idx 53 = 1 (Seat only, the
other buildings are level 0), cycle still 72 s (1792952342 - 1792952270, may be the pre-purchase cycle). `resources` was
`23994/54/20796799/...` (idx 1 = 54; probably mushrooms after paying 25, unverified). So the boost may be a per-building
"golden" flag, not a global one: unclear. sf-api 0.4.4 has no merchant command, so it needs `Command::Custom` and a
`safe.rs` whitelist entry that allows exactly this one purchase with the price 25 (not done).

## CORRECTION 2026-10-25 (user): the merchant purchase was a 2x speed upgrade for the Seat ONLY

The captured `IdleMercantbuy` bought a per-building "golden" upgrade (flag idx 53 = Seat), NOT a global boost. **Do NOT buy it
(or any merchant good) until the user says so again.** The one-time 25-mushroom exception below is REVOKED; the user spent
the mushrooms by hand. A global speed boost may exist under another offer: ask before ever whitelisting anything.

## Implemented 2026-10-31 (user: "TestChar1 is level 109 and still has not used it, the dollars just pile up")
Until now NOTHING was implemented (only documented), that is why the dollars piled up. `src/idle.rs`, called in the main loop after the free beer:
from level 105, if the server sent an idle game (`gs.idle_game`; if not, one `[idle] ... no idle game (unlock pending?)` line per run), every 20 min up to 15
`IdleUpgrade` commands: the building with the cheapest next level, in the biggest affordable batch (100/25/10/1). Simple because the earning per cycle of
unbuilt buildings is unknown (no ratio possible yet). `safe.rs` allows only affordable `IdleUpgrade` batches (`idle::upgrade_ok`); `IdleSacrifice` (prestige),
`IdleUpgradeAmount::Max` and the merchant stay forbidden. NOT seen live: check `[idle]` lines; open: prestige timing (user looks up the optimum), runes.

## Prestige rule (user 2026-11-05, was said earlier but never written down; implemented, NOT in a release yet, never seen live)
Sacrifice (`IdleSacrifice` = `IdlePrestige:0`, resets the buildings, gives runes) as soon as the runes it would give (`sacrifice_runes`) are at least the runes we already have in total (`current_runes`), the first one at 20: 20 -> 40 -> 80 -> ...; when the exact number is missed, the first moment beyond it (22 -> 44+ -> ...). The runes are never spent. `idle::sacrifice_due` / `rune_rule` (unit test), checked at the start of every 20-minute pass, and again in `safe.rs` (`IdleSacrifice` is whitelisted only when due). Log lines: the first pass after a start shows `runes X, a sacrifice would give Y, resets Z`; a sacrifice logs `SACRIFICE ...` and the state after it. To verify live: whether `sacrifice_runes` is really the gain of a sacrifice now (sf-api doc says so) and whether `current_runes` is the running total; check the first sacrifice by hand against the game. Open: what the runes buy (user does not know yet).

## Journal `idle.log` (2026-11-07)
`roster/<character>/logs/idle.log` (not rotated, unlike progress.log): a `state:` line at every 20-minute pass (runes, sacrifice gain, resets, total sacrificed, money) and `SACRIFICE sent` / `after the sacrifice:` lines. Added because the start-up `[idle] ... runes X` line rotated out of the 100-line logs before anyone saw it, so the live check of the prestige rule was impossible. Read it to verify `sacrifice_runes` / `current_runes` semantics against the rule. NOT in a release yet.

## First live data 2026-11-07 13:40 (`idle.log`, the user did the first sacrifice by hand on several characters)
`current_runes` is the running total, `sacrifice_runes` the gain of the next one (matches the rule). Chlamydie / Květoš / Wecros 27 runes (gain 25, resets 1), TestChar1 29 (gain 25, resets 1), Pagan 20 (gain 5, resets 1), PajaRizz 42 (gain 13, resets 2), Sanek 46 (gain 1, resets 2: sacrificed shortly before, money 594k). 42 = 20 + 22 and 46 = ~23 + 23 fit "second sacrifice when gain >= owned", so the bot probably did the second one on PajaRizz and Sanek; no `SACRIFICE` line was captured yet (verify with the next one in `idle.log`). Next due for the 27/29-rune characters when the gain reaches 27/29.

## Breakpoint strategy (user 2026-11-07, replaces "cheapest next level"; NOT in a release yet)
Levels 25, 50, 100, 250, 500, 1000, 2500 of a building double its earning and shorten its cycle, a bigger jump than the cost efficiency of any single upgrade. `idle::STEPS` is the user's order (a staggered wave): Seat 25, Seat 50, Popcorn 25, Seat 100, Popcorn 50, Popcorn 100, Parking 25, Parking 50, Seat 250, Parking 100, Trap 25, Popcorn 250, Trap 50, Trap 100, Drinks 25, Parking 250, Drinks 50, Drinks 100, DeadlyTrap 25, Seat 500, Trap 250, DeadlyTrap 50, Popcorn 500, DeadlyTrap 100, ... (the user adds the rest: VIPSeat, Snacks, StrayingMonsters, Toilet, higher breakpoints; building order = enum order = price order).
Rules (user): (1) go EXACTLY to the breakpoint - never a batch (100/25/10/1) that overshoots the target; when the money does not reach it, get as close as possible; (2) buy as much of the NEXT unreached step as affordable (not necessarily at once) and invest nowhere else while saving for it; (3) the runes' bonus stays after a sacrifice (+5 % dollars per rune), the list simply starts over. Steps whose target is already reached are skipped; past the end of the list the old rule (cheapest next level) applies until the user extends the list. Unit tests: `buys_exactly_up_to_the_breakpoint`, `the_list_is_followed_in_order`. To verify live: `[idle]` lines follow the list, no purchase of an unlisted building while a step is open.
