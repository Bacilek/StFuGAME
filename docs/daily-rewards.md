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

- Lucky coins: only for a Gleeman/event Wheel of Fortune task when a chest needs it (`docs/tasks.md`, user 2026-10-07).

## Verification status
| What | Status |
|---|---|
| The calendar gets collected | ⏳ 2026-10-07 already collected (next 08.10. 00:00), verify tomorrow |
| Free Wheel of Fortune spin | ✅ 2026-10-07 17:14: +492 xp, mushrooms unchanged; next free 08.10. 00:00 (once a day) |
| Welcome pack claimed correctly, right content, no mushroom loss | ⏳ not verified yet, 2026-10-08 |

## Free deal at the Mushroom Dealer (`daily::claim_welcome_pack`, implemented 2026-10-08)
- The user 2026-10-07: for new accounts a free Deal appears at the Mushroom Dealer after some time (mushrooms + resources for free).
  The user claimed it manually on TestChar1 at first; captured live on a fresh challenge character 2026-10-08.
- Not one of sf-api's typed commands – `Command::Custom` + base64 params, same mechanism as the guild list
  (`guild.rs`), captured straight from a browser Network tab:
  - `ShopCatalog` (args `["1", "", "1"]` → params `1//1`): returns the whole shop catalog as plain JSON
    (`{"success":true,"catalog":{"categories":[…],"articles":[…]}}`, no `&key:value` tail).
  - `ShopCheckout` (args `["1", identifier, ""]` → e.g. params `1/starterpacks_item_2/`): claims one article by its
    `identifier`. Response is that same JSON shape (`{"success":true,"completed":true,"id":…,"orderId":"FREE_…"}`)
    **followed by** `&resources:…&achievement(…):…&characterstatus:…&dailytasklist:…&…` – neither pure JSON nor
    sf-api's usual format, so `GameState::update` cannot be trusted to parse it
    (`SimpleSession::send_raw_only`/`safe::send_raw_only`: skip updating the game state for these two commands
    entirely; the caller always follows up with a normal `Command::Update`, which refreshes it and re-triggers the
    mushroom watchdog against the state from before the checkout).
  - The catalog's free item as of 2026-10-08: `identifier: "starterpacks_item_2"`, `internalIdentifier:
    "welcomepack_1"`, `sku: "FREE"`, `price: {"amount": 0, "currency": null}`, `content`: Gold 10, Coins(=mushrooms)
    10, Hourglasses 100, Lucky coins 10. Gated by `ruleset` `tutorialProgress: ["2:200"]` (presumably why it only
    shows up "after some time"). The other `StarterPacks` item (`starterpacks_item_1`) costs real money (129 CZK)
    – confirms the rest of this shop is paid, only ever check out an `identifier` whose catalog `price.amount`
    was just confirmed to be exactly `0`.
- Safety: `safe.rs`'s whitelist only pins the shop id (`"1"`) and the request shape; the actual "is this free"
  check happens in `daily::claim_welcome_pack` itself, re-fetching the catalog and checking `price.amount == 0`
  fresh every time (never a hardcoded identifier) before ever calling `ShopCheckout`.
- **Not yet verified live** – written from a capture, not yet seen running against the real server end to end.
