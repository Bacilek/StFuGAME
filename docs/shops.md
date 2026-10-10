# Shops (Weapon Shop, Magic Shop)

## Rules (from the user, 2026-10-07)
- NEVER buy for mushrooms, only for gold. Never `RefreshShop` (a new offer costs a mushroom).
- Shopping only after the whole Thirst for Adventure is used up (Tavern done), so the best items are already in.
- An item that improves the equipment even a little (1 stat point is enough) → buy it and equip it. Epic items are still never sold.
- Spare gold → "spin" the shop: buy items for gold even when they are worse, sell them right away and look at the new offer.
  Continue until all items in the shops cost mushrooms.
- Spare gold = above a dynamic reserve: the price of the most expensive gold item for our class seen that day
  (so an upgrade that shows up after a spin can always be bought). Upgrades may use all the gold.

## Implementation (src/shops.rs)
- Once a day (`Day` with the date), called in the main loop when the Tavern is done, before City Guard.
- Gold item = `mushroom_price == 0`, real equipment (`equipment_slot`), not unique.
- Upgrade = the gold item with the biggest gain over the equipped one by `inventory::value` (same as the backpack), affordable.
  Bought into a free backpack slot, then `inventory::manage` equips it and sells the old one (unless epic).
- Spin = the cheapest non-epic gold item, only if gold − price ≥ reserve; `inventory::manage` sells it. Cost logged
  as `[shops] Spin cost` (purchase − sale). Limit 60 spins a day.
- Stops when: no gold items in either shop, not enough gold above the reserve, the backpack is full,
  or the bought shop slot did not change after `Update` (stale shop data, needs verification).
- `safe.rs`: `BuyShop` passes only when the item at `shop_pos` has `mushroom_price == 0`, matches `item_ident`
  (it contains both prices; the server also rejects a changed item) and we have the gold. Plus the mushroom watchdog.

## Shop refresh for an ad (2026-10-08: do not implement, see below)
- The user: once a day each shop can be rerolled for watching an ad. Wanted: do it after everything costs mushrooms.
- sf-api 0.4.4 has no command for it (only `RefreshShop` = 1 mushroom, forbidden). The server sends a `skipvideo` key, sf-api ignores it.
- Captured by the user 2026-10-08 (Weapon Shop): `AdvertisementsCompleted:4` (base64 `NA==`), `PlayerNewWares:1/2`
  (base64 `MS8y`). Confirms the pattern: ad id is per-shop (Weapon 4, Magic 5), second `PlayerNewWares` argument
  is always `2` (paid by ad). The ad itself plays via a 3rd-party SDK (`www.ayetstudios.com` `sdk_event` calls,
  a Google IMA video beacon to `csi.gstatic.com`), not something the bot could trigger on its own even once the
  server commands are known – no video player, no SDK integration.
- Also captured: `PlayerPollScrapbook` fires when opening the Magic Shop (scrapbook/legendary bitmask for the
  shop's collection %), unrelated to ad availability. A plain `Poll` response is just a timestamp – the ad
  availability flag is not in it, must be in some other, fuller response.
- Both Weapon Shop responses captured 2026-10-08: `AdvertisementsCompleted:4` → `trust_counter:4&resources:…`;
  `PlayerNewWares:1/2` → `timestamp:…&resources:…&characterstatus:…&subscriptionstatus:&storeitemsshakes:…&itemlevelshop:16`.
  Neither contains an "ad still available today" flag – that hunt is abandoned (see decision below).
- **Decision (user + Claude, 2026-10-08): do not implement this.** `AdvertisementsCompleted`'s response includes a
  `trust_counter` field, almost certainly an anti-fraud/bot-detection metric from the ad network (ayetstudios) or
  the game's own ad-reward system, not a simple "ads remaining" counter. Spoofing `AdvertisementsCompleted` without
  ever actually watching the ad through that SDK risks more than a failed call – this mechanism looks purpose-built
  to catch exactly this kind of automation, with consequences that could reach beyond one rejected command. Not
  worth it for a minor convenience (saving one mushroom's worth of shop reroll). Left here for reference only;
  do not revisit unless the risk picture changes.
- Captured by the user 2026-10-07 (browser, DevTools), one shop (probably the Magic Shop):
  1. `AdvertisementsCompleted:5` (params base64 `NQ==`), response 71 B,
  2. `PlayerNewWares:2/2` (params `Mi8y`), response 520 B (the new offer).
  sf-api's `RefreshShop` sends `PlayerNewWares:<shop − 2>` (Weapon = 1, Magic = 2) without the second argument = the mushroom
  refresh. The second argument `2` is most likely "paid by ad"; `5` in `AdvertisementsCompleted` is probably the ad type for this shop.
- Unknown: the Weapon Shop ad id, the response bodies, how to tell that today's ad is still available, and whether
  `PlayerNewWares:x/2` without a completed ad would charge a mushroom (must be known before implementing).
- The user 2026-10-07: in the client, an ad that is still available shows a TV icon; after it is used the icon is gone.
  The "New Goods" button for a mushroom is always there. So the server tells the client about availability somewhere
  (field not known yet, sf-api ignores it) – find it from the response bodies before implementing.
- Waiting for the user: Response bodies of `AdvertisementsCompleted`/`PlayerNewWares` (not just sizes), and – more
  importantly – a capture of whatever request/response carries the "ad still available today" flag (not `Poll`,
  not `PlayerPollScrapbook`): open the shop with the TV icon still showing and look for any other request in
  Network, or diff a fuller state response from before vs. after using the ad.
- The SDK/video-player integration (ayetstudios.com) means the bot cannot actually play the ad itself even with
  the right server commands – at best it could call `AdvertisementsCompleted` without ever watching anything,
  which may or may not be accepted by the server; needs testing once the availability flag is known, very
  carefully (mushroom watchdog stays on).

## Hourglasses (user 2026-10-07)
- Hourglasses (`ItemType::QuickSandGlass`) sometimes cost gold in the shop. Buying them for gold is fine for spinning
  (cheapest gold item); they are saved, never used. The event task "Buy hourglasses" uses them too.
- Potions are bought here as well (after equipment upgrades, before tasks and spinning), see `docs/potions.md`.

## Verification status
| What | Status |
|---|---|
| Gold-only items have `mushroom_price == 0` | ✅ 2026-10-08 (many purchases, mushroom watchdog never tripped) |
| `BuyShop` costs only gold, mushrooms unchanged | ✅ 2026-10-08 (mushrooms stayed at 18 across many buys) |
| After a purchase the shop slot gets a new item (and `Update` shows it) | ✅ 2026-10-08 (repeated buys in the same shop succeed, no "did not change" abort) |
| Spin cost (purchase vs sale price) | ✅ seen (e.g. 2026-10-07 spin cost 1.03–6.59 g) |
| A bought hourglass: backpack item or straight to the hourglass counter? | ⏳ not verified |
| Shopping works during City Guard | ✅ 2026-10-07 per the user (everything works during a shift); bot run not seen yet |

## Reserve survives restarts (2026-10-09, user)
Bug found: the reserve (most expensive gold item for our class seen today) lived only in memory and a new day/restart started at 0. After the 18:44
restart all 12 characters ran the shop with offers that were already spun empty ("All items cost mushrooms"), so the reserve stayed 0 and
`tasks::buy_surplus_attributes` (floor = 5 × reserve = 0) spent almost all gold on attributes (Květoš 7.5 g, Filminy ~25 g, Mimimimi11 ~27 g, PajaRizz ~46 g left).
Now: `roster/<character>/shop_reserve.json` (`{date, seen, carry}`) is loaded on start; a new day starts from yesterday's `seen` as `carry`
(effective reserve = max(seen today, carry)); and when no gold item was ever seen, the gold price of the class's mushroom-priced equipment is the estimate.
Not run live yet. The `need more gold` error from the surplus loop (price estimate = last price) is separate and still there.

## Ad-refresh experiment (2026-10-10, user: "try it on the test user")
- Code: `src/adtest.rs`, hooked into the main loop before `shops::run`. Runs ONLY for `TestChar1` and ONLY while the flag
  file `roster/TestChar1/ad_test` exists (content `weapon` = ad 4 / shop 1, `magic` = ad 5 / shop 2, `probe` = just log).
  The file is removed afterwards; at most once per process.
- Flow: log the ad-related keys of the login response (`skipvideo` …, `SimpleSession::login_ad`), log mushrooms/gold/offer, wait
  11–16 s like a real ad, `AdvertisementsCompleted:<ad>`, 1.5–3.5 s later `PlayerNewWares:<shop>/2`, `Update`, log again.
  Everything → `roster/TestChar1/logs/ad_test.log` (session keys redacted). Whitelisted by `adtest::custom_allowed` (exactly
  these two commands for the chosen shop); the mushroom watchdog allows a decrease of 1 for that `PlayerNewWares` only.
- Verification (⏳ after the first run): does the server accept the claim without an SDK? is the refresh free (mushrooms
  unchanged)? does the offer change? what does `skipvideo` say before/after? any `trust_counter` change or warning?
- **Result 2026-10-10 10:07 (TestChar1, Weapon Shop): it WORKS and is FREE.** Login response had `skipvideo:1` (ad-available flag, very
  likely; `0` expected once used – not yet seen). After an 11 s simulated watch: `AdvertisementsCompleted:4` →
  `trust_counter:4&resources:…`, 3 s later `PlayerNewWares:1/2` → full new offer (the whole Weapon Shop changed), mushrooms
  37 → 37, silver unchanged; the normal shop pass then bought an upgrade from the new offer. No error, no warning.
  Still open: does `skipvideo` flip to `0` after use (check the next login), does the second call the same day get rejected
  (and for free), Magic Shop (`magic`: ad 5, shop 2) not tried, `trust_counter` meaning (4 = ?, compare with a later value).
- **Result 2026-10-10 10:11 (TestChar1, Magic Shop, after a bot restart): also works and is free.** `AdvertisementsCompleted:5` →
  `trust_counter:3`, `PlayerNewWares:2/2` → new offer, mushrooms 37 → 37. NOTE: the fresh login still showed `skipvideo:1`
  although the Weapon Shop ad had been used 3 min earlier, so `skipvideo` is NOT a reliable per-shop "ad still available"
  flag (maybe "ads exist at all", or the client-side TV icon comes from elsewhere). `trust_counter` was 4 for ad 4 and 3
  for ad 5 → looks like a per-ad-id value, not a global count. Still open: a second use of the same shop's ad the same day
  (rejected? free?), whether the ads reset at midnight, and how a bot would know an ad is still available (probably just try
  once a day per shop and treat an error/unchanged offer as "used").
- **Decision (user 2026-10-10): the "second use the same day" test is cancelled.** Regular characters get exactly 1 ad refresh per shop per
  day (resets at midnight); more attempts could look like abuse, so the bot must never try a repeat. If automated later, track
  "used today" locally per shop and never rely on probing the server. Automation itself is not decided yet (ask the user).
- **Automated for TestChar1 only (2026-10-10, user):** `adtest::daily_refresh`, called from `shops::shop` when spinning has run
  dry (no gold item left to spin, or gold below the reserve). Weapon Shop first, then Magic Shop, one per call (the loop looks at the
  new offer and buys/spins again before the next one). At most one ad per shop per day, marker `roster/TestChar1/ad_refresh.json`
  (`{date, weapon, magic}`, written before sending, so no retry after an error; today's two manual uses were pre-filled).
  The manual flag file `ad_test` still works (and also marks the shop as used). Other characters: `NotApplicable`, nothing sent.
  First automatic run expected 2026-10-11 (needs a release rebuild + restart); check `roster/TestChar1/logs/ad_test.log`.


## Late spin round (2026-10-10, user)
The shop pass runs once a day (after the Tavern), but gold can arrive afterwards (Gleeman chests are opened after the shops). `shops::respin` runs whenever the pass is done and gold ≥ cheapest non-epic spin item + reserve: spin only, nothing else, no ad refresh. Not verified live.

## Lucky coin ad – the flying TV (2026-10-10, user: "try it on TestChar1")
The TV with wings sometimes appears in the Tavern or at Dr. Abawuwu; clicking it plays an ad and gives 3 lucky coins. The user's capture (`my_input/tv.png`): `Poll`s and ayetstudios `sdk_event`s while the ad plays, then ONE `AdvertisementsCompleted` with params `MQ==` (= `1`), then `Poll`s again – no follow-up command. Implemented as the one-shot flag experiment `roster/TestChar1/ad_test` = `lucky` (`adtest::lucky_ad`, TestChar1 only): logs the login ad keys, lucky coins + mushrooms before/after, the raw response, in `roster/TestChar1/logs/ad_test.log`. Put the flag file in place while the TV is visible in the game (availability is not visible to the bot – whether the server rejects the call without a TV is exactly what is tested). NOT run yet; needs a release rebuild. No automation until the user has seen the result (how often the TV appears, any `trust_counter`).
