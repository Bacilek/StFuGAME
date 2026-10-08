# City Guard

## Rules (from the user)
- When the Tavern is done (Thirst for Adventure used up, no expedition running), go on City Guard duty automatically.
- Length: at most 10 h, but the shift should end in the first hour after midnight (00:00–00:59), because at midnight
  the Thirst for Adventure resets and all tasks start again (Tavern, shopping, …), and the bot should not idle
  without a shift. So min(10, hours until midnight rounded up).
  (Originally "end at midnight at the latest", changed by the user on 2026-10-07.)
- **23:00 checkpoint (user 2026-10-08):** a shift starting before 23:00 is capped there instead of running past
  midnight. Rationale: if a Gleeman-task beer grants bonus Thirst for Adventure while on guard duty, that ALU sits
  unused until the shift ends – and if the shift only ends after midnight, the ALU resets anyway and the bonus
  expedition is lost. Capping at 23:00 gives the Tavern a real window to use it. A shift starting at/after 23:00
  (because nothing was capped, or because this is the shift that starts right after a capped one ends) falls back
  to the original "ends 00:00–00:59" rule – this is what bridges the gap to the new day on a day when beer was
  never needed, with no extra branching: `guard::guard_hours` just applies the same "hours until the next boundary,
  rounded up" formula against whichever boundary (23:00 or midnight) is still ahead. No guaranteed daily pay loss:
  on a no-beer day the two shifts together cover the same span as the old single shift would have.
- The Arena and Dungeons keep working during the shift.

## Implementation (src/guard.rs, loop in src/main.rs)
- `StartWork { hours }` when the character is idle and the Tavern is done; `FinishWork` (pay) after the shift ends (+10 s).
- `CancelWork` is never used (not whitelisted) – shifts are never ended early, only shortened in advance via the
  23:00 checkpoint.
- The main loop wakes up at the end of the shift (pay), at midnight (Thirst for Adventure reset), and otherwise on
  its usual up-to-30-min cadence.
- **Bug found and fixed 2026-10-08:** after a shift ended, the character would immediately start another one without
  ever trying to spend fresh (e.g. post-midnight) Thirst for Adventure first. The "Tavern done" flag was wrongly set
  to true whenever the Tavern was *skipped* because the character was busy with City Guard, not only when it was
  *attempted* from Idle and genuinely found nothing affordable. Fixed in `main.rs`'s main loop
  (`tavern_done = before.0 == CurrentAction::Idle` instead of `!= CurrentAction::Expedition`). Caught live on
  TestChar1: a stale City-Guard-until-yesterday shift got paid out at restart, then a new shift started right away
  even though 100 min of fresh Thirst for Adventure were sitting unused.
- **Same-pass staleness bug found and fixed 2026-10-08 (as part of the checkpoint feature):** `tavern_done` is
  computed once at the top of each main-loop pass and passed by value into `shops::run`/`tasks::run`/`guard::run`
  later in the same pass. If `tasks::run` drank a beer (granting bonus ALU) partway through a pass, `guard::run`
  right after it still saw the *stale* `tavern_done == true` from before the beer, and – with `current_action`
  freshly `Idle` right after `FinishWork` – would start a new shift on top of the just-granted ALU. Fixed by having
  `guard::run` re-check `gs.tavern.thirst_for_adventure_sec` fresh (not the passed-in boolean) before `StartWork`;
  a non-zero value now defers to the next pass's Tavern section instead.
- **Hourglass safety valve (user 2026-10-08, `tavern::should_skip_wait_with_glass`):** even with the checkpoint and
  staleness fix, bonus ALU can still end up stranded with too little real time before midnight (e.g. beer drunk
  during the 23:00→00:0x bridging shift itself, so the Tavern only gets to it once that shift ends). In that case,
  the expedition's `Waiting` stage uses an hourglass (`Command::ExpeditionSkipWait { typ: TimeSkip::Glass }`,
  whitelisted in `safe.rs`) instead of sleeping, whenever normal waiting would leave less than a 15-min safety
  margin before the midnight reset. Hourglasses are a replenishable resource (unlike mushrooms) and this is an
  explicit, narrow exception to "never use hourglasses" – only to avoid *wasting* ALU, never used when not needed.
  **Open/unverified:** what exactly happens to an in-progress expedition right at the midnight reset is not known –
  the 15-min margin is deliberately conservative to stay well clear of finding out the hard way.

## Verification status
| What | Status |
|---|---|
| The shift starts and ends at the expected time | ✅ 2026-10-07: start 17:06 for 6 h, end 23:06 (wage 208 s/h) |
| Pay via `FinishWork` | ✅ 2026-10-08 (`[guard] Shift (6 h) finished, collecting the pay` → `Pay 14.64 g`, new shift started right after) |
| Thirst for Adventure reset at local midnight | ⏳ not verified |
| 23:00 checkpoint actually caps a shift there | ⏳ not verified (needs release rebuild + a live day) |
| Bridging shift (23:00→~00:0x) starts automatically on a no-beer day | ⏳ not verified |
| Beer near the checkpoint correctly lets the Tavern run before a new shift starts (staleness fix) | ⏳ not verified |
| Hourglass skip fires only in genuine crunch cases, `quicksand_glasses` decreases by exactly 1, no mushroom involvement | ⏳ not verified (rare case, needs a live beer-near-midnight occurrence) |
| What happens to an in-progress expedition exactly at the midnight reset | ❓ unknown, not something this feature needs to answer (avoided via the 15-min margin) |
