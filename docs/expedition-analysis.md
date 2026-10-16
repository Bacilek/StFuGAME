# Expedition analysis (min-max workbook)

Goal: reach 40 heroism (= 3 stars, the maximum reward) as often as possible, with as little wasted effort as possible.
This file is the place to compare "what the data says" with "what the bot does" and to collect improvement ideas.
Rules and encounter tables: `docs/expeditions.md`; strategy code: `src/tavern.rs` (`choose_encounter`, `future_value`).

## How to refresh the numbers
```
node scripts/expedition_stats.js                         # whole report to the terminal
node scripts/expedition_stats.js --since 2026-10-17      # only newer runs (e.g. after a strategy change)
node scripts/expedition_stats.js --out roster/expedition-report.md --failures   # file + every failed run with its picks
```
It reads `logs/expeditions.jsonl` + `roster/*/logs/expeditions.jsonl` (all characters, local only). Sections: per mission,
failed runs, options per round, value of the round after a Cake/CupCake, chain follow-up, encounter frequency,
keys/chests, rewards, where heroism comes from. Needs only Node (no Python on this machine).
After a change of strategy: rerun with `--since <date of the release>` and compare to the baseline below.

## Source check of the encounter table (user's screenshot of the in-game FAQ, 2026-10-16)
All 16 cycles in `my_input/expeditions.png` match `src/missions.rs` / `docs/expeditions.md`: points, limits, "after that only the last
encounter" (Sword Trial, Extinguished Fire, Toxic Fountain Cure), bounty "can be redeemed 3× per individual encounter",
chicken drumstick → 2 options next, suckling pig → 1 option next, key → treasure chest (limit 2×, gold always, sometimes
hourglasses, event resources), toilet paper limit 3×. Cycles NOT on the screenshot (from the user / server only): Running Dry,
Plenty of Men in the Sea, Beak to Tail, Gone With the Wind.

## Baseline (journals up to 2026-10-16, 1117 expeditions, 13 journals, strategy = greedy score in `choose_encounter`)
- **88 % reach 40** (987 of 1117). Of the 130 failures, 110 end at 30–39 (near misses), only ~20 are real disasters.
- Heroism histogram (final): below 35: 54, 35–39: 76, 40–44: 285, 45–49: 323, 50–54: 243, 55+: 136. 63 % of all runs end at 45+, so in most
  runs the points are there – the failures are runs where the target bonus or the chain did not come together.
- Per mission, ok rate: Podium 100 %, Hot Carnal Craving (pig) 99 %, Dragon 98 %, Unicorn 93 %, Build A Friend 93 %,
  Toxic Fountain 91 %, Beak to Tail 86 %, Sanitary 85 %, Gone With the Wind 85 %, Running Dry 84 %, Fish 84 %, Fire 83 %,
  Revealing Lady 83 %, Sword Trial 80 %, **Bewitched Stew 74 %** (worst).
  This matches the user's tier list (S: podium/dragon, A: pig/unicorn/Klaus, B: the rest) – tier choice in `choose_expedition` is justified.
- Rewards after the boss: gold 80 %, mushrooms 17 % (the bot takes them whenever offered), hourglass 3 %.

## How the crossroads works (measured)
- Round 1 always offers 3 options. Later rounds: 3 options ~83–87 %, 2 options ~5–9 %, 1 option ~7–9 % (in rounds without a Cake/CupCake before).
- **After a suckling pig (Cake) the next round has exactly 1 option (712 of 712), after a chicken drumstick (CupCake) exactly 2.** Otherwise 3.
- Slots (share of everything offered): Dummy2 21 %, Key 14 %, CupCake 8 %, Cake 5 %, DummyBounty 5 %, Dummy1 4 %, Dummy3 4 %,
  Bait 3 %, rest ≤ 2 % each. Per expedition (10 rounds, ~27 slots): Dummy2 5.6×, Key 3.7×, CupCake 2.2×, Cake 1.5×, DummyBounty 1.4×,
  Dummy1 1.2×, Dummy3 1.2×. Key → chest: key limit 2×, but a key is on offer 3.7× per run; only 52 % of keys picked became a chest
  (Suitcase follows a key only 47 % in the next round, 59 % within 2 rounds, so a key is a gamble).
- **A chain's next step is NOT guaranteed.** After taking step k, step k+1 is on offer in the next round ~65–70 % of the time, within
  2 rounds ~80–89 % (worst: fish 53 % / Key→chest 47 %). Chains with a heavy upfront cost are therefore risky: Build A Friend
  (Hand/Feet/Body = −15 before Klaus +35) ended between −15 and +12 in 5 of 68 runs when Klaus never came.
- The next step is as likely whether or not it is the main mission (e.g. Hand→Feet 69 % when Klaus is the target); the main mission
  does not seem to be favoured in the offers, only the bonus makes it worth following.
- A bounty (wanted poster) for an item appears only for items in the current expedition (see `expeditions.md`), +10 once per poster,
  max 3× per individual encounter.

## Why Cake / CupCake are not free points (the main finding so far)
The round after a restricted pick is worth less, measured over all journals (best option on offer, in points):

| pick | points now | next round: options | next round: best option | 2-round total | compare: Dummy3 (3) + normal round (3.7) |
|---|---|---|---|---|---|
| suckling pig (Cake) | +5 | 1 (forced) | 1.7 | **6.7** | 6.7 |
| chicken drumstick (CupCake) | +3 | 2 | 2.6 | **5.6** | 6.7 |
| Dummy3 / any other | +3 | 3 | 3.7 | 6.7 | – |

- Pig ≈ break-even with a Dummy3 on points, and it costs control: the forced single option can be a bad one (Hand −5, Bait −2, Key
  without a chest) – 15 % of forced rounds are another Cake (a chain), 13 % a Key, 17 % Dummy2.
- **Drumstick is worse than a Dummy3 by ~1.1 points over two rounds and equal to a Dummy2** (+2 and then 3.7 = 5.7). The bot picked
  CupCake 875× out of 2314 offers (44 times while a Dummy3 was on the same screen, a clear loss).
- Both reduce the chance to see a chain step / chest / poster item in the next round. In failing runs the typical picture is
  `Dummy2, CupCake, Dummy2 …` after the chain is done – 35–39 final heroism.
- The bot's score is greedy on `immediate_gain + future_value`; it does not model "the next round has fewer options". Idea list below.

## Where the heroism comes from (successful runs, points shown on the pick only)
Cake 19 %, Dummy2 14 %, CupCake 10 %, Klaus 10 %, Dragon 9 %, Podium 8 %, Dummy3 7 %, Gone With the Wind steps 4 %, Prince 4 %,
Couple 4 %, witch's brew 3 %. Task bonuses (+5 per dragon, +10 per podium, completion +5/+10/+20 …) come on top and are not in this table.
Average needed: 4 points per round over 10 rounds; the average best option of a normal round is 3.7 – so the target mission's bonuses
are what makes 40 realistic, and a run without a finished target almost always ends at 30–39.

## What is done well (keep)
- Target mission first: among missions with a completion bonus (857 runs) 89.6 % reach 40 when the target was completed (avg heroism 47.0)
  but only 68.4 % when it was not (avg 38.0; 57 runs). Per-item missions (dragon, podium, pig, fire, sword, fountain) have no "done" state.
- Poster logic (+10) and "stop at 40 and farm key → chest" work (63 % of runs end at 45+; overflow is cheap).
- Tier-based expedition choice (since 2026-10-16) aligns with measured ok rates.

## Gaps / suspected losses (ordered by expected gain)
1. **Gone With the Wind (leaf blower +4 → leaf swirl +2 → Leafbold +6, +10 on completion, −10 on failure) is not mapped** (`Unknown`
   in the journal, 72 of 1117 runs = 6.4 %). The completion bonus is verified (55 journal mismatches of exactly +10, 15 × +20 with a poster):
   sf-api has no ids, so the bot sees `Unknown(+4)`, `Unknown(+2)`, `Unknown(+6)`. Needs a points-based mapping (see todo).
   It treats them with the generic `UnknownTarget` guess; ok rate 85 %, in line with other B missions.
2. **CupCake (drumstick) is overvalued**: take it only when nothing better; never over a Dummy3; it blocks chain/key visibility.
3. **Pig/drumstick mid-chain**: when a chain step is expected next round (we just took step k, the next one shows up ~70 %), a
   Cake/CupCake pick shrinks the next offer to 1–2 options and can hide step k+1. Prefer a normal point item in such a round.
4. **Feasibility constants** (`feasibility`: 0.7 / 0.35) can be replaced by measured values: next step offered next round ~0.65–0.70,
   within two rounds ~0.85, so P(complete a chain of n steps in the remaining rounds) is better modelled per step.
5. **Key / chest value**: chest only 52 % of keys. When 40 is secured, a key is a gamble worth taking only with ≥ 2 rounds left;
   verify what the chest brings (gold only, `chests` in the journal, reward value is not yet separated from the boss reward).
6. **Build A Friend**: −15 upfront for +35 +5; 5 / 68 runs finished between −15 and +12 without Klaus. Consider stopping the chain when ≤ 3
   rounds are left and Klaus has not been seen, or only starting it with ≥ 6 rounds left (see `feasibility`).
7. **Bewitched Stew** (worst ok rate 74 %): cauldron +2, witch −5, brew +15, completion +5; the −5 step makes runs fragile; check if
   taking the witch only when brew follows is realistic (needs the follow-up probability above: 65 %).

## Implemented 2026-10-16 (⏳ verify live after the next release build; compare with `--since`)
- A+B: `tavern::restriction_cost` subtracts from the score of a CupCake (1.1, +0.5 mid-chain) and a Cake (0, +1.0 mid-chain: a held non-last chain step or some of a counted target found). Zero in the last round and when the pig is the target (Hot Carnal Craving). Tests: `drumstick_is_worse_than_dummy3_pig_is_not`, `restriction_cost_grows_mid_chain`.
- Raw ids of unknown encounters in the journal (`src/rawid.rs`). Not done: D (needs the ids first), C, F, key-with-lead (the user: 40 beats chests, chests give no XP).
- E (≤ 2 rounds left, 40 not secured: points only): `feasibility` already returns 0 when steps > floors left, so future value vanishes by itself; nothing extra needed.

## Foreign quest items (e.g. Prince +8 while the mission is Klaus)
Taken for the immediate points only (future value of a foreign chain is 0). The permanent last encounter that a started cycle leaves behind is a risk, measured:
after taking a Prince in a non-fountain mission (60 runs) the polluted fountain (RoyalFrog −4) was offered 2.7× per run, **picked 0.13× and forced (the only option) 0.05× per run**,
i.e. ≈ −0.5 expected points against +8 gained. Sword in stone → broken sword −4: offered 0.65×, picked 0.03×, forced 0.02× per run; campfire → burnt campfire (0): harmless.
Verdict: grab the foreign +8 whenever offered; no penalty needed (a forced −4 after a pig is ~2 % per run).

## Candidate tactics to test (one change at a time, then rerun the report with `--since`)
- A. Lower the score of CupCake by ~1.1 (opportunity cost of the next round) and of Cake by ~0 … 1; never take CupCake over a Dummy3.
- B. Model "next round has fewer options" explicitly: penalty = (expected best option with 3) − (with 2 or 1) for the picked item
  (measured: 3.7 vs 2.6 vs 1.7), reduced when the next chain step is already known to come (e.g. we hold a poster).
- C. Replace `feasibility` by the measured per-step probability (0.68 next round, 0.85 within 2).
- D. Map Gone With the Wind by its points (+4 / +2 / +6) so it is a proper chain with bonus +10 / −10.
- E. Late-game rule: with ≤ 2 rounds left and 40 not secured, maximise immediate points (ignore future value).
- F. Compare Cake behaviour when the mission is Hot Carnal Craving (+3 per pig at the end): this mission already ends at 53 avg,
  so Cake there is clearly right; for other missions it is break-even.
Success metric: ok rate (share ≥ 40) by mission and overall, plus the near-miss share (35–39). Baseline: 88.4 % overall.

## Data hygiene notes
- Older journal lines (before 2026-10-12) were written with an incomplete mission table: `Klaus`, `RidingStan`, `Barkeeper`, `Merman`
  show as "bonus unverified" / "unknown mission" and the checks carry mismatches (Klaus +5, Unknown +10 …). The script maps those names
  back to the real missions (`ALIAS` in `scripts/expedition_stats.js`) and uses the final heroism, which is unaffected.
- `Well is -2 in the table` (108 mismatches) is the old cauldron value; the server shows +2 (fixed 2026-10-16).
- Character level differs between journals; expedition difficulty does not seem level-dependent (offers are the same pool), reward size is.
- Per-round heroism IS in the journal, inside `checks` ("heroism after picking X in round N: expected a, server has b"); older runs too.
- The journal's `rewards` is only the boss reward. **The chest (Suitcase) reward is not recorded** – only `chests` (count). Needed before
  judging keys: log what the chest gave (gold amount, hourglass, event resources) – todo.
- "n runs, x %" in the per-mission table: `n` = number of runs of that mission, "ok" = share of them that reached 40. Gone With the Wind: 72 of 1117 runs
  (6.4 % of all expeditions), 61 of 72 ok (85 %), avg final heroism 46.3 → upper B tier (like Beak to Tail 86 %).

## Early lead -> key? (measured 2026-10-16, heroism from `checks`)
Final result by heroism after round 5 / 6 / 7: **≥ 30 after round 5: 131/131 reached 40; 35–39 after round 6: 114/114; 30–34 after round 6: 96 %;
35–39 after round 7: 99 %**; below 30 the rate drops to 77–88 %. So with a lead of 35 after round 6 the points are practically safe, and a
key (and then a chest) costs only a "spare" round. `choose_encounter` today only farms keys when the *projected* heroism is already ≥ 40; it
does not consider "almost certain" leads. Keys picked in round 8–9 turn into a chest only 54 % / 47 % of the time (round 7: 70 %), so an earlier key
(round 5–7) is the better gamble – but the chest value is unknown (see above).
Build A Friend: Hand taken in round 1 → Klaus came 45/46, round 2: 10/11, round 3: 6/8, round 4–5: 2/3; so start the chain only early.
