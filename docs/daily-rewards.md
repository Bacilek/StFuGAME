# Daily rewards

## Rules (from the user)
- Spin the Wheel of Fortune once a day, **free spin only**. Never for mushrooms or lucky coins.
- Collect the daily login bonus (calendar).
- Nothing else (task chests etc. not for now).

## Implementation (src/daily.rs)
- Calendar: `CollectCalendar` once `specials.calendar.next_possible` has passed.
- Wheel: `SpinWheelOfFortune { payment: FreeTurn }`, only when `specials.wheel.next_free_spin` is known and has passed (+30 s).
  Only `FreeTurn` is whitelisted; after a spin the next one needs a new time from the server (same safeguard as Arena/Dungeons).
- A failed action is retried at the earliest after 30 min.

## Verification status
| What | Status |
|---|---|
| The calendar gets collected | ⏳ 2026-10-07 already collected (next 08.10. 00:00), verify tomorrow |
| Free Wheel of Fortune spin | ✅ 2026-10-07 17:14: +492 xp, mushrooms unchanged; next free 08.10. 00:00 (once a day) |
