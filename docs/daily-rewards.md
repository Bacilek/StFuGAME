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
- **Hardened 2026-10-16 (user screenshot `my_input/what.png`: a catalog dump with `sku: BLACKMARKET` items paid in COINS = mushrooms through `cost`, e.g. 799):** `daily::is_free_article` now requires `sku == "FREE"`, `price.amount == 0` AND no `cost` entries, so Black Market / mushroom-priced items can never pass as "free" (unit tests). The Black Market (deeds, trophy shelves, …) is paid with mushrooms – never touch it.

## Expected: Wheel of Fortune 2.0 (user 2026-11-05, NOT seen yet)
Around character level 150, after finishing an expedition, an unlock feature arrives (ident unknown) for "The Wheel of Fortune 2.0": the daily wheel can then also give Arcane Splinters, fruits and souls. The bot does not send unknown `UnlockFeature` idents (`safe.rs`), so it will sit pending. When a character reaches it: capture the `unlockfeature:` value (and the spin response) and ask the user; watch for sf-api failing to parse the new wheel rewards.
Capture 2026-11-05 (user, a spin on the new wheel for 10 lucky coins): `wheelresult:0/3` (type 0 = mushrooms, amount 3), `wheel:2/2/<ts>` after it. The user expects the wheel to upgrade by itself, no unlock command. sf-api (`rewards.rs`, `mod.rs` `wheelresult`) already has the upgraded table (type 1 = Arcane instead of Wood, 3 = PetItem instead of Stone, 10 = Souls) but ASSUMES "upgraded" when level >= 95 AND pets AND underworld are unlocked, so the Underworld is probably the real condition. Risk: if the assumption is wrong, a Stone result (type 3) fails to parse as a PetItem and the whole `Update` can error; watch the first spins of a character that unlocks the Underworld.
Update 2026-11-05 (user): Bacilek got the Underworld right after an expedition (a free beer from the witch enchant gave the Thirst) and the wheel STILL has wood and stone, so sf-api's "upgraded" guess is wrong (the real wheel upgrade is something else, unknown). Fix: sf-api is vendored in `vendor/sf-api` (`[patch.crates-io]` in `Cargo.toml`), one change in `gamestate/rewards.rs`: a type-3 wheel result that is no pet item becomes Stone instead of a parse error. Type 1 may still show as "Arcane" for Wood (only a label). When upgrading sf-api, re-apply the patch (search `PATCHED`).
Captures 2026-11-05 (user, Bacilek, paid spins, the wheel IS the upgraded one): `1/483` = Arcane Splinters (type 1 -> Arcane, matches sf-api upgraded), `6/12420` = WoodXL, `2/108084` = ExperienceXL, `5/1` = Item (a weapon, `backpack:` follows), `0/3` = mushrooms. So the sf-api "upgraded" guess was right for Bacilek (Underworld), the earlier "wood and stone" were probably the segment pictures; the type-3 -> Stone fallback in `vendor/sf-api` stays as a harmless safety net. Not yet seen: fruits, souls (type 10), pet items (type 3).
More captures 2026-11-05 (user, upgraded wheel): `4/16510000` = SilverXL (16.5 M gold), `3/33` = Fruit (`PetItem::Fruit`, amount 31..=35 = habitat 1..5; 1 apple), `8/3996` = StoneXL. The upgraded wheel has NO other pet items, only fruit (user). Full table (upgraded): 0 mushrooms, 1 Arcane Splinters, 2 XP XL, 3 fruit, 4 gold XL, 5 item (1 = equipment), 6 wood XL, 7 XP, 8 stone XL, 9 souls (not seen yet). Everything parses; nothing to do in the bot.
