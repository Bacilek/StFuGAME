# Arena fight highlights

Saves an impressive won Arena fight (`PlayerCombatLogMark`) so it shows up in Quarter → Mail, like manually
clicking "save" on a fight in-game. User's idea (2026-10-09): detect things like crit/block/evade streaks, a
paladin's block-and-heal, winning against a much stronger opponent, surviving at very low HP, and class-specific
flourishes (berserker rampage, necromancer/druid summons going off, assassin's double weapon, demon hunter revive).

## Why this needed reverse engineering

sf-api (0.4.4) does **not** parse the Arena's per-round combat log at all for the server's current format:
`SingleFight::update_rounds` (`gamestate/arena.rs`) has `if fight_version > 1 { // TODO: Actually parse this;
return Ok(()); }`, and the live server sends `fightversion:2`. So `GameState.last_fight.fights[0].actions` is
always empty – nothing to build on. The fighters themselves (level/life/attributes/class, `fighter_a`/`fighter_b`)
*are* parsed fine (`update_fighters` has no such guard), so those came from `GameState` as normal; only the
round-by-round `fight.r` string needed decoding by hand.

The client itself is **Unity WebGL** (`cdn.sfgame.net/res/sfgame3/Build/…/*.wasm.gz`), not readable JavaScript –
so no shortcut via browser DevTools source search; the format below was reconstructed purely by matching 9 of the
user's real captured fights against their own blow-by-blow description of what happened in each one (2026-10-09
session). See `docs/precedents.md` for the session log of which fight proved which code.

## Raw format (`fight.r`)

Example (necromancer fight, wolf summoned 3×):
```
fight.r:21252/0/11/0/0/15180/41107/1/2/1/3/0/21252/0/12/0/0/15180/40263/…
```
Split on `/` into integers. Each round is **7 fixed fields**, followed by a variable-length tail we don't use
(looks like an internal effect/duration counter, not needed for scoring):

| # | Field | Meaning |
|---|---|---|
| 0 | `actor` | Player id of whoever acts this round |
| 1 | `stance` | Active class buff/stance marker – `20`/`21` = Paladin Defensive Stance, `30` = Berserker Rage. Not used for scoring (informational only). |
| 2 | `type` | What kind of action (table below) |
| 3 | `result` | Outcome on the **defending** side (table below) |
| 4 | *(f4)* | Not decoded, unused |
| 5 | `own_life` | Life of the *actor* at this point |
| 6 | `target_life` | Life of the *defender* after this action |

The next round starts wherever `actor` (field 0) is one of the two fighter ids again – the tail length varies
(2 to 5 extra values) depending on what happened, so rounds can't be split by fixed width.

### `type` – confirmed values

| Value | Meaning | Confirmed in |
|---|---|---|
| 0 | Normal attack | every fight |
| 1 | Critical hit | every fight |
| 10 | Battle Mage Blast (big single hit) | battle-mage fight, round 1 |
| 11 | Summon companion (no damage) | necromancer (golem, skeleton, wolf) |
| 12 | Companion attack (normal) | necromancer, plague-doctor skeleton |
| 14 | Demon Hunter **revive** (no damage; `own_life` = life after reviving) | DH opponent, 2× in one fight |
| 15 | Companion **big/crit** hit | necromancer wolf (2×, dmg 15241/21607 vs its normal ~850-6500) |
| 17 | Plague Doctor special attack (seen once, result was `3` = blocked) | plague-doctor fight |
| 18 | Plague Doctor poison bolt/application (deals direct damage too) | plague-doctor fight |
| 19 | Plague Doctor poison DoT tick (smaller) | plague-doctor fight |
| 20 | Plague Doctor poison DoT tick (bigger) | plague-doctor fight |
| 13 | Druid **swoop** (eagle/sweep) attack; the same actor attacks again right after with a regular `type=0` row (free attack) | Druid opponent, 2× in one fight (attack #4 and the last) |
| 100 | Assassin's second weapon attack (always right after a `type=0`/`1` first-weapon action, same actor) | assassin fight, every one of their turns |

`17`–`20` (Plague Doctor poison) are **not fully disambiguated** – which is the "crit tick" vs. plain tick wasn't
100% pinned down round-by-round against the user's narration. Not needed for scoring (poison damage shows up via
the generic "big hit" / revive / streak signals anyway), so left as-is; revisit if a cleaner sample turns up.

Bard's "notes" mechanic does **not** get its own `type` code – it just inflates the damage of a normal `type=0`/
`1` action and changes the tail values' shape (`[1,1,2,2,0]` instead of the usual `[1,2,3,X,0]`). Not decoded
further; not needed (generic "big hit as % of max life" covers it).

### `result` – confirmed values

| Value | Meaning | Confirmed in |
|---|---|---|
| 0 | Normal (hit landed) | every fight |
| 3 | **Blocked** – `target_life` unchanged | necromancer (golem), plague-doctor (skeleton), battle-mage, assassin |
| 4 | **Evaded** – `target_life` unchanged | plague-doctor fight (3× in a row), assassin fight |
| 6 | **Blocked + healed** (Paladin shield) – `target_life` **increases** instead of staying flat | berserker fight, battle-mage fight, necromancer×2, assassin fight |

`result` describes the outcome for whoever is defending in that round, independent of `type` – e.g. a companion's
big hit (`type=15`) can itself be blocked+healed (`result=6`) if the defender has that mechanic; seen in the
necromancer "wolf" fight (round 19: `type=12`, `result=6`).

### Other confirmed mechanics (no dedicated code, just row patterns)

- **"Several actions in one turn"** (berserker rampage, golem/wolf attacking right after its owner, assassin's
  second weapon): shows up as **multiple consecutive rows with the same `actor` id**. Generic across classes –
  detect by counting the longest run of same-actor rows, no class-specific logic needed.
- **Revive** (Demon Hunter) – **confirmed 2026-10-09** (user's fight vs a DH opponent who revived 2×): its own row,
  `type=14`, `actor` = the reviving fighter, `own_life` = life it comes back with (11700 then 10400 of 13000, i.e.
  90 % then 80 %), `target_life` = the other side's life unchanged; the previous row ends with the reviver's
  `target_life` ≤ 0 (−4173, −1238), and the reviver acts again right after (same actor twice in a row). Counted by
  `type=14` per actor, no longer from the life sequence.
- **Druid** – **confirmed 2026-10-09** (user's fight vs a Druid, narrated attack by attack): bear form = the Druid's `stance` field is `11` (`10` = normal form; our rows then carry the same value in f4), e.g. attack #1 was a bear crit. Swoop = `type=13`, immediately followed by a free regular attack of the same actor (attack #4 and the last one). Counted for OUR Druid as `swoops` and `bear_crits`. User's reading of the mechanic (2026-10-09): swoop is a random proc, it can come on the round right after a bear attack but never in the same round as one; each swoop raises the chance of the next swoop, capped at ~50 % (user, from memory – not verified, and it does not grant a dodge) (in the sample both swoops have stance 10, i.e. not bear – consistent, not verified further).

## Scoring: per-character, per-category personal bests (`src/arena_highlights.rs::measure`)

**Changed 2026-10-09 (user, after seeing the first design):** not an additive score. The user does not want to
re-review a pile of "decent" fights per character at the end of the challenge – thresholds should be high (a
genuinely rare fight, not just a good one), and at most **one saved fight per character per category**: a
paladin can have one record for its best block streak *and* a separate one for its best heal count; a plague
doctor one for evades and a separate one for poison damage; records are never compared **across** characters
(Filminy evading 7× in a row and some other character's assassin evading 6× are two unrelated records – one per
character, in `roster/<character>/arena_highlights.json`).

Only runs on a **won** fight (the user's examples were all about winning). Every category below is measured on
every win; a fight gets marked only if it **beats this character's own previous best** in at least one category
(or is the first time that category's threshold was ever reached by this character) – a later, better fight of
the same category also **un-marks** the earlier, inferior one (`PlayerCombatLogMark <id>/0`), so only the single
best fight per category stays saved in-game.

| Category | What it measures | Threshold to even qualify | Direction |
|---|---|---|---|
| `crit_streak` | Longest run of our own consecutive crits | ≥ 4 | bigger better |
| `block_streak` | Longest run of the opponent's hits on us blocked (incl. blocked+healed) | ≥ 5 | bigger better |
| `evade_streak` | Longest run of the opponent's hits on us evaded | ≥ 5 | bigger better |
| `heal_blocks` | Count of blocked-and-healed hits (paladin shield) in the fight | ≥ 4 | bigger better |
| `summons` | Count of companion summons (necromancer/druid) in the fight | ≥ 3 | bigger better |
| `companion_big_hits` | Count of companion crit hits (`type=15`) in the fight | ≥ 2 | bigger better |
| `combo_run` | Longest run of consecutive actions by us in one go (berserker rage, dual weapon, summon+attack) | ≥ 4 | bigger better |
| `swoops` | Our Druid swoop attacks (`type=13`) | ≥ 3 | bigger better |
| `bear_crits` | Our crits while in bear form (stance 11) | ≥ 3 | bigger better |
| `revives` | How many times WE revived mid-fight (we are a demon hunter; `type=14` rows of our id) | ≥ 1 | bigger better |
| `opp_revives` | How many times the opponent (a demon hunter) revived before we still won | ≥ 2 | bigger better |
| `low_hp` | Lowest our life got relative to max, excl. the final (kill) round | ≤ 5 % | **smaller** better |
| `level_gap` | Opponent's level minus ours | ≥ 6 | bigger better |
| `strength_ratio` | Opponent's `arena::strength()` ÷ ours | ≥ 1.6 (60 %+ stronger) | bigger better |
| `big_hit` | Biggest single hit we landed, as a fraction of the opponent's max life (covers bard notes, battle mage blast, mage/assassin one-shots – no dedicated code needed, see below) | ≥ 35 % | bigger better |

All thresholds are **first guesses, unverified** – raise/lower them once real fights start producing records (or
too many/too few). Every new record logs `[arena] New personal best: …`; nothing is logged for an ordinary fight
that doesn't beat a record (kept deliberately quiet, per the project's general log-noise precedent).

## Mechanism

1. `arena::run` sends `Fight` via `safe::send_raw` (not `safe::send`) so both the parsed `GameState` *and* the
   raw response string are available – `arena_highlights::measure` needs the raw string for `fight.r`; nothing
   else in `arena.rs` changed.
2. `arena_highlights::maybe_mark` measures every category and loads `roster/<character>/arena_highlights.json`
   – this character's record per category so far, stored as `{value, msg_id}` (the `msg_id` of the fight that
   currently holds that record). Keeps only the categories that both clear their threshold and beat (or are new
   compared to) the stored record.
3. Looks for a matching entry in `gs.mail.combat_log` (`CombatLogEntry`, matched by opponent name + most recent
   within 5 min) to get the `msg_id` of *this* fight to mark. If not found, logs the miss and leaves the stored
   records untouched (so a later fight beating the same old record gets a fresh chance).
4. For every beaten category, un-marks the old record's fight (`PlayerCombatLogMark <old_msg_id>/0`) – deduped,
   since several categories can share the same old record. Then marks the new fight
   (`PlayerCombatLogMark <msg_id>/1`). Both are `Command::Custom` + base64-params, the exact mechanism captured
   live from the user's own browser Network tab (sf-api's session layer base64-encodes the joined args
   automatically for every `Custom` command – no manual encoding needed). Whitelisted in
   `safe.rs::custom_allowed` only for an id already present in our own combat log, flag `0` or `1`.
5. Only after that does it update and save `arena_highlights.json` with the new `{value, msg_id}` per beaten
   category.

## Verification status

| What | Status |
|---|---|
| `fight.r` round format (7 fixed fields + variable tail) | ✅ matched against 9 real fights, 7 classes |
| `type` 0/1/11/12/15 (necromancer summon/companion/crit) | ✅ 2026-10-09, 2 separate fights |
| `type` 10 (Battle Mage Blast) | ✅ 2026-10-09, 1 fight |
| `type` 100 (Assassin second weapon) | ✅ 2026-10-09, 1 fight, every opponent turn |
| `type` 17/18/19/20 (Plague Doctor poison) | ⚠️ codes seen, exact tick-vs-crit distinction not fully nailed down |
| `result` 3/4/6 (blocked/evaded/blocked+healed) | ✅ confirmed across 5+ fights |
| "several actions in one turn" = consecutive same-actor rows | ✅ berserker (5 in a row), assassin (dual weapon), necromancer (summon+attack) |
| Demon Hunter revive | ✅ format confirmed from a saved fight (unit test `counts_opponent_demon_hunter_revives`); not yet seen live in the bot |
| Druid bear form (stance 11) / swoop (type 13 + free attack) | ✅ format confirmed from a saved fight (unit test `decodes_druid_bear_and_swoop`); thresholds for `swoops`/`bear_crits` are guesses |
| `PlayerCombatLogMark <id>/1` marks a fight (sets `combatloglist`'s last field to `3`, settling to `2`) | ✅ 2026-10-09, both via our own API call and the user's in-game click, same effect |
| `PlayerCombatLogMark <id>/0` un-marks a fight (sets last field to `0`) | ✅ 2026-10-09, confirmed by diffing the user's own pin/unpin in-game clicks (clean single-row `3→0`) – inferred from the resulting list, the raw unmark request itself wasn't captured, but high confidence |
| `gs.mail.combat_log` populated right after a `Fight` response without a separate fetch | ⏳ unverified assumption, still untested by our own code sending `Fight` |
| Point thresholds reasonable (mark too much / too little) | ⏳ pending real scores in the log |

## Open questions

- Should a close loss with something impressive (e.g. a long crit streak, dying at the very last blow to a much
  stronger opponent) ever be marked too, or strictly wins only, as implemented? Current behaviour: wins only.
- If more class samples turn up (Demon Hunter revive, Druid bear form/swoop, a cleaner Plague Doctor poison
  sequence), update the tables above and `src/arena_highlights.rs` accordingly.
- **Un-marking the old, beaten record (2026-10-09, resolved):** user confirmed `/0` un-marks by pinning then
  un-pinning a fight in-game and sending both `combatloglist` dumps – a clean single-row `3→0` diff, nothing else
  changed. `maybe_mark` now sends `PlayerCombatLogMark <old_msg_id>/0` for every record it supersedes, right
  before marking the new one with `/1`, so only the single best fight per category should stay saved. Still
  worth double-checking the very first time this actually fires live (own API call, not an in-game click) that
  the old fight really disappears from Quarter → Mail.

## Text log of saved fights (user 2026-10-09)
`roster/arena_highlights.txt` lists every fight the bot currently has saved, one per line: `time | character | vs opponent |
msg id | category: why; category: why`. Beaten categories are removed from a line, the line is deleted with its last one
(`update_log`/`strip_beaten`, unit-tested). An un-mark in the game happens only when a fight holds no record any more.
