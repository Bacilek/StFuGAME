# Tavern expeditions

Sources:
- official FAQ: https://playa-games.helpshift.com/hc/en/4-shakes-fidget-1653988985/faq/281-expeditions---encounters/
  (encounter cycles, step heroism, limits),
- the user's experience (task bonuses from the quest giver),
- verification from bot runs (`logs/expeditions.jsonl`).

The machine-readable version lives in `src/missions.rs`; when something changes, update both.

## Expedition flow
Click the quest giver → choose one of 2 expeditions → 5 rounds (crossroads, no waiting) → boss → choose a reward → waiting/travel
→ "continue" → 5 rounds → boss → waiting → end. Always **10 rounds** in total.

Verified from runs:
- After the waiting the server waits for "continue" (`ExpeditionContinue`). Only then does it send the new crossroads offer.
- After the 2nd boss the bot never saw a reward offer (`rewards` empty). The user says there is a reward choice after both bosses – not verified yet.
- Per-item bonuses (`+5/`) are credited by the server right after the pick in round 10, before the boss
  (Sword Trial: 19 → 15 for a sword −4, then +4 × 8 = 47).

## Rules
- There is always exactly one main mission (the quest giver's task). Items from other cycles still show up at the crossroads.
- **Items of foreign cycles** are only taken for their immediate heroism (e.g. dragon mission, offer campfire +3 / dummy +1 → campfire).
  They are not taken for future steps of a foreign cycle – a foreign task gives nothing at the end (user rule).
- Every encounter adds its heroism right when picked. Every encounter belongs to some cycle (chain).
- **40 heroism = maximum reward.** More adds nothing. Goal: secure 40, then farm keys and chests.
- Limit = how many times a cycle can repeat during one expedition.
- "Permanent end" = the last step stays on offer even after the cycle is complete.
- Task bonus notation: `+5/` = per target item, credited at the end; `+10` = once, on completion.

## Encounter cycles

| Cycle | Limit | Steps (heroism) | sf-api names | Task bonus |
|---|---|---|---|---|
| Dummies | – | Dummy 1.0 (1), 2.0 (2), 3.0 (3) | `Dummy1..3` | – |
| Key and Treasure Chest | 2× | key (0) → treasure chest (0, gold / resources depending on the event) | `Key` → `Suitcase` | – |
| Hot Carnal Craving | – | chicken drumstick (3, next crossroads only 2 options), suckling pig (5, next crossroads only 1 option) | `CupCake`, `Cake` | +3 / piece (pig) |
| Bounties | 3× per type | wanted poster (0), the wanted encounter then gives +10 once (the poster is used up) | `*Bounty` | – |
| Dragon Taming | 2× | bait/princess (−2) → dragon (10) | `Bait` → `Dragon` | +5 / piece |
| Sanitary Emergency | 3× | toilet paper (0), only when the quest giver asks for it; 3 needed | `ToiletPaper` | +20, failure −5 ⚠ |
| Extinguished Fire | 1×, permanent end | campfire (3) → phoenix (5) → burnt-out campfire (0) | `CampFire` → `Phoenix` → `BurntCampfire` | +4 / piece |
| Unicorn Whisperer | 1× | horn (1) → donkey (3) → rainbow (5) → unicorn (7) | `UnicornHorn` → `Donkey` → `Rainbow` → `Unicorn` | +10 |
| Podium Climber | 2× | small hurdle (−1) → big hurdle (−2) → winner's podium (15) | `SmallHurdle` → `BigHurdle` → `WinnersPodium` | +10 / piece |
| Revealing Lady | 1× | socks (0) → cloth pile (0) → couple (12) | `Socks` → `ClothPile` → `RevealingCouple` | ? |
| The Sword Trial | 1×, permanent end | sword in stone (6) → bent sword (3) → broken sword (−4) ✱ | `SwordInStone` → `BentSword` → `BrokenSword` | +8 / piece |
| Bewitched Stew | 1× | cauldron (−2) → witch (−5) → witch's brew (15) (user 2026-10-10; brew +15 server-verified) | `Well` → `Girl` → `Balloons` | +5, failure −5 |
| Running Dry | 1× | mugs (0) → draft beer (6) → tapping bartender (6) (user 2026-10-10) | `Mugs` → `DraftBeer` → `Barkeeper` | +5, failure −5 |
| Toxic Fountain Cure | 1×, permanent end | fairy fountain (8) → polluted fairy fountain (−4) | `Prince` → `RoyalFrog` | +8 / piece (polluted fountain keeps appearing to the end, net +4 each), failure 0 (user 2026-10-10, not yet verified) |
| Build A Friend | 1× | hand (−5) → feet (−5) → body (−5) → Klaus (35) | `Hand` → `Feet` → `Body` → `Klaus` | ? |

Sanitary: the FAQ says "at the end +5 if you have 3 toilet papers, otherwise −5". The user is sure about +20 / −5; their version applies (not yet verified in a run).

Task bonuses cannot be checked against the FAQ (it does not list them); they rely on the user's data and are verified during runs.

✱ Verified on the server, the FAQ says 5 / 2 / −5.

Bounties exist only for: dummy, dragon, burnt-out campfire, unicorn, winner's podium, couple, broken sword, witch's brew,
polluted fairy fountain and Klaus. Only those whose target is in the current expedition appear.

## Verification status (from bot runs)
The bot verifies during runs: encounter points vs. the table, the heroism change after every pick and the change after the last boss.
Results are in the journal (`checks`), mismatches are reported as `[check] MISMATCH`. After runs, copy what was confirmed here.

| What | Status | Evidence |
|---|---|---|
| Poster +10 to the wanted encounter | ✅ | 2026-10-07: dummy with a poster 25 → 37 |
| Unicorn Whisperer +10 right on completion | ✅ | 2026-10-07: unicorn 8 → 25 |
| Donkey +3 (the user wrote +2) | ✅ FAQ | the server showed `Donkey(+3)` |
| Chicken drumstick +3, next crossroads only 2 options | ✅ | 2026-10-07: 37 → 40, next round 2 options |
| Dragon Taming +5 / piece at the end | ✅ | 2026-10-07: 36 → 46 for 2 dragons |
| Treasure chest appears after the key | ✅ | 2026-10-07: Barkeeper round 1 key, round 2 chest |
| Hot Carnal Craving: pig → next crossroads 1 option, drumstick → 2 | ✅ | 2026-10-07: Barkeeper rounds 6 and 7 |
| Sword Trial: 6 / 3 / −4 (FAQ 5 / 2 / −5, the user −4 for the broken one) | ✅ server | 2026-10-07: offer and credited heroism |
| Revealing Lady: couple +12 | ❌ mismatch | 2026-10-07 (Barkeeper, couple not the target, no poster): the server showed +12, heroism 16 → 23 (+7) |
| Revealing Lady: task bonus | ⏳ hypothesis +10 | 2026-10-07 (target, with a poster): 3 → 30 (+27). Fits "couple really +7, poster +10, bonus +10"; the alternative "couple +12, bonus +5" does not fit the Barkeeper run |
| Sword Trial +8 / piece (user) | ✅ | 2026-10-07: 4 broken swords, +32 after the pick in round 10 |
| A poster works only once (used up) | ✅ | 2026-10-07: the second broken sword got no +10 |
| Bewitched Stew: witch −5, brew +15 | ✅ | 2026-10-07 server; user confirmed 2026-10-10 (cauldron −2 per user, not yet verified) |
| Other task bonuses, Sanitary +20 / −5 | ⏳ | pending |

## Unmapped
- Gone With the Wind (user 2026-10-10: +10 on completion, −10 on failure; leaf blower +4 → leaf swirl +2 → Leafbold +6): sf-api has no ids for it, so it is not in `missions.rs`; add it once its ids show up in the journal `unmapped`.
- Missions from sf-api the FAQ does not know: merman (`FishingRod` → `FishingBait` → `Merman`),
  riding (`Chicken` → `Tiger` → `RidingStan`), lovebirds (`Cupid` → `LovestruckShakes` → `LoveBirds`).
- Missing task bonuses: Revealing Lady, Build A Friend (the bot estimates +5 on completion for now).

The bot does not stop on unknown things. It prints them and logs them to the journal (`unmapped`).
For an unmapped mission it treats items from the same tens range as the target as chain steps (e.g. 151 → 152 → 153).

## Rewards after a boss
- Normally: mushrooms > gold > hourglasses.
- Leftover expedition (started with at most 3 min of Thirst for Adventure, i.e. the rest of the day): mushrooms > hourglasses > gold.
  It gives little gold (2026-10-07: 9 silver) but the same hourglasses. After a bot restart mid-expedition it is treated as a full one.
- Hourglasses are never used.

## Choosing an expedition
1. The shortest one (least Thirst for Adventure).
2. At equal length: an unmapped mission or one with an unverified bonus, otherwise the highest ease.
   Ease = (step heroism + bonus + avoided penalty) / number of rounds. A rough estimate, tune from the journal.

## Journal
The bot writes every expedition to `logs/expeditions.jsonl` (picks, heroism, keys, chests, verdict).
Verdict: below 40 = failure, above 45 with a declined key/chest = probably too much heroism instead of chests.
Tuning constants: `OPPORTUNITY_COST` and `feasibility()` in `src/tavern.rs`.
