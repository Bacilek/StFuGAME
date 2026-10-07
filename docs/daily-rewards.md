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

## Free deal at the Mushroom Dealer (idea, not implemented)
- The user 2026-10-07: for new accounts a free Deal appears at the Mushroom Dealer after some time (mushrooms + resources for free).
  The user claimed it manually on TestChar1. Wanted: the bot claims such free deals automatically.
- Captured (browser DevTools): `ShopCatalog` (params `1//1`, response ~16 kB = the catalog), then
  `ShopCheckout` (params unreadable from the screenshot, probably `1/starter_packs_item_…/`).
  Response: `{"success":true,"completed":true,"id":29472,"orderId":"FREE_2087052_…"}` + `resources`, `achievement`, …
- This is the real-money shop: the bot may only ever check out an item the catalog shows as free (price 0).
  Needed before implementing: the exact `ShopCheckout` params and the `ShopCatalog` response.
  TestChar1 already claimed it, so capture it on the next new character when the deal shows up.
