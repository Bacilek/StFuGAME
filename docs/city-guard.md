# City Guard

## Rules (from the user)
- When the Tavern is done (Thirst for Adventure used up, no expedition running), go on City Guard duty automatically.
- Length: at most 10 h, but the shift should end in the first hour after midnight (00:00–00:59), because at midnight
  the Thirst for Adventure resets and all tasks start again (Tavern, shopping, …), and the bot should not idle
  without a shift. So min(10, hours until midnight rounded up).
  (Originally "end at midnight at the latest", changed by the user on 2026-10-07.)
- The Arena and Dungeons keep working during the shift.

## Implementation (src/guard.rs, loop in src/main.rs)
- `StartWork { hours }` when the character is idle and the Tavern is done; `FinishWork` (pay) after the shift ends (+10 s).
- `CancelWork` is never used (not whitelisted).
- The main loop wakes up at the end of the shift (pay) and at midnight (Thirst for Adventure reset).
- **Bug found and fixed 2026-10-08:** after a shift ended, the character would immediately start another one without
  ever trying to spend fresh (e.g. post-midnight) Thirst for Adventure first. The "Tavern done" flag was wrongly set
  to true whenever the Tavern was *skipped* because the character was busy with City Guard, not only when it was
  *attempted* from Idle and genuinely found nothing affordable. Fixed in `main.rs`'s main loop
  (`tavern_done = before.0 == CurrentAction::Idle` instead of `!= CurrentAction::Expedition`). Caught live on
  TestChar1: a stale City-Guard-until-yesterday shift got paid out at restart, then a new shift started right away
  even though 100 min of fresh Thirst for Adventure were sitting unused.

## Verification status
| What | Status |
|---|---|
| The shift starts and ends at the expected time | ✅ 2026-10-07: start 17:06 for 6 h, end 23:06 (wage 208 s/h) |
| Pay via `FinishWork` | ✅ 2026-10-08 (`[guard] Shift (6 h) finished, collecting the pay` → `Pay 14.64 g`, new shift started right after) |
| Thirst for Adventure reset at local midnight | ⏳ not verified |
