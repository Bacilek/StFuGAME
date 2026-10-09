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
| 15 | Companion **big/crit** hit | necromancer wolf (2×, dmg 15241/21607 vs its normal ~850-6500) |
| 17 | Plague Doctor special attack (seen once, result was `3` = blocked) | plague-doctor fight |
| 18 | Plague Doctor poison bolt/application (deals direct damage too) | plague-doctor fight |
| 19 | Plague Doctor poison DoT tick (smaller) | plague-doctor fight |
| 20 | Plague Doctor poison DoT tick (bigger) | plague-doctor fight |
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
- **Revive** (Demon Hunter): would show as our life reading `≤ 0` in some row, then positive again in a later row
  while the fight (and our turns) continue. Never observed live (no Demon Hunter opponent found this session) –
  implemented from first principles off the life-sequence data, not yet confirmed against a real revive.
- **Druid bear form / swoop**: never observed live either (no Druid opponent found). Covered only by the generic
  crit-streak / big-hit signals, not a dedicated detector.

## Scoring (`src/arena_highlights.rs::evaluate`)

Only runs on a **won** fight (the user's examples were all about winning, see `docs/precedents.md`). Additive
point system, current weights (all **unverified**, tune after seeing real scores in `logs/progress.log`):

| Signal | Threshold | Points |
|---|---|---|
| Crit streak (own) | ≥ 3 in a row | 3 |
| Block streak (opponent's hits on us) | ≥ 3 in a row | 2 |
| Evade streak (opponent's hits on us) | ≥ 3 in a row | 2 |
| Block+heal count | ≥ 3 in the fight | 2 |
| Companion summoned | ≥ 2 times | 1 |
| Companion big hit (`type=15`) | ≥ 2 times | 2 |
| Longest same-actor run (own side) | ≥ 3 | 3 |
| Mid-fight revive | happened | 4 |
| Survived at ≤ 10 % max life (excl. the final blow) | happened | 3 |
| Opponent ≥ 5 levels higher | — | 2 |
| Opponent ~50 %+ "stronger" (`arena::strength()` ratio) | — | 3 |

`MARK_THRESHOLD = 5`. Every evaluated fight (even below threshold) logs its score and reasons
(`[arena] Highlight score N: …`) so the thresholds can be tuned from real data without needing a rebuild loop.

## Mechanism

1. `arena::run` sends `Fight` via `safe::send_raw` (not `safe::send`) so both the parsed `GameState` *and* the
   raw response string are available – `arena_highlights::evaluate` needs the raw string for `fight.r`; nothing
   else in `arena.rs` changed.
2. `arena_highlights::maybe_mark` scores the fight, then looks for a matching entry in `gs.mail.combat_log`
   (`CombatLogEntry`, matched by opponent name + most recent within 5 min) to get the `msg_id` to mark.
3. If found and the score clears the threshold, sends `Command::Custom { "PlayerCombatLogMark", [msg_id, "1"] }` –
   the exact command/params captured live from the user's own browser Network tab (`params` = base64 of
   `"<msg_id>/1"`, which sf-api's session layer already base64-encodes automatically for every `Custom` command –
   no manual encoding needed). Whitelisted in `safe.rs::custom_allowed` only for an id already present in our own
   combat log.

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
| Demon Hunter revive | ⏳ not observed live, logic untested against a real case |
| Druid bear form/swoop | ⏳ not observed live, no dedicated detection |
| `PlayerCombatLogMark` actually marks the fight server-side | ⏳ not sent live yet |
| `gs.mail.combat_log` populated right after a `Fight` response without a separate fetch | ⏳ unverified assumption |
| Point thresholds reasonable (mark too much / too little) | ⏳ pending real scores in the log |

## Open questions

- Should a close loss with something impressive (e.g. a long crit streak, dying at the very last blow to a much
  stronger opponent) ever be marked too, or strictly wins only, as implemented? Current behaviour: wins only.
- If more class samples turn up (Demon Hunter revive, Druid bear form/swoop, a cleaner Plague Doctor poison
  sequence), update the tables above and `src/arena_highlights.rs` accordingly.
