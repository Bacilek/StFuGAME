# Witch (unlocks at character level 66)

Source: the user's explanation 2026-10-19 + sf-api 0.4.4 (`gs.witch: Option<Witch>`, `unlockables.rs`), one raw capture. Nothing is implemented yet.

## How it works (user)
- Every day the witch "buys" ONE item type (ring, belt, weapon, armour, …) and pays 2× the gold the shop would pay. Goal: route ALL selling
  of that item type for the day there (equipment swaps, shop spinning, …).
- Enough sacrificed items (probably shared by the whole server, e.g. eu31) → a new enchantment is brewed for 24 h.
- Enchant tab: each item type (gloves, boots, weapon, …) can get ONE specific enchantment. We do not choose it, we only pay gold to apply it
  to the worn piece. The user will list the enchantments (TODO).
- "Juice" fruits into potions: later, once we have fruits (not interesting now).

## sf-api
- `witch.required_item: Option<EquipmentSlot>` = the type the witch wants today (`None` + `cauldron_bubbling = true` when the cauldron is full/brewing).
  So we CAN read today's type.
- `witch.progress` 0–100 (collection progress), `witch.enchantment_price` (silver), `witch.enchantments: EnumMap<Enchantment, Option<EnchantmentIdent>>`
  (None = not unlocked yet).
- Commands: `WitchDropCauldron { item_pos }` (`PlayerWitchSpendItem`), `WitchEnchant { enchantment }` (worn item of that type, `PlayerWitchEnchantItem:<id>/1`),
  `WitchEnchantCompanion`, `WitchBrewPotion` (fruits, ignore). None whitelisted in `safe.rs` yet.
- Enchantments known to sf-api (item type ids): SwordOfVengeance (10), MariosBeard (30), ManyFeetBoots (40), ShadowOfTheCowboy (50),
  AdventurersArchaeologicalAura (60), ThirstyWanderer (70), UnholyAcquisitiveness (80), TheGraveRobbersPrayer (90), RobberBaronRitual (100).

## Capture 2026-10-19 (character above level 66)
`witchshop:1792368000/1792421040/1250000/157821/1/5/51/0/9/91/1792068112/6/61/0/8/81/0/10/101/0/3/31/0/4/41/0/7/71/0/1/11/0/1/1/9/204100`
- idx 33 = 1 ("needs more" = 1 → no item wanted → cauldron bubbling), idx 35 = 204100 silver = 2041 gold enchant price.
- idx 4 = 1 is read by sf-api as the enchantment count, yet nine `x/ (id+1) /flag` triples follow from idx 6 (ids 51, 91, 61, 81, 101, 31, 41, 71, 11);
  one triple has a timestamp (1792068112 = ~3.5 days before the capture). sf-api therefore sees only the first one (ShadowOfTheCowboy) as unlocked
  → VERIFY which enchantments the game shows as unlocked and what the flag/timestamp means.


## Enchantments (user 2026-10-19; "quests" also means expeditions)
| Item | Enchantment | Effect |
|---|---|---|
| Helmet | Adventurer's Archaeological Aura | +10 % XP on quests |
| Armor | Mario's Beard | +50 % mushrooms on quests |
| Gloves | Shadow of the Cowboy | you attack first (random if the enemy has it too) |
| Boots | "36963-Feet boots" (user confirmed the name; sf-api: ManyFeetBoots) | quests 30 s shorter (thirst unchanged, only time) |
| Weapon | Sword of Vengeance | +5 % crit damage of this weapon |
| Amulet | Unholy Acquisitiveness | +10 % chance to find items on quests |
| Belt | Thirsty Wanderer | first beer of the day free, beer cap 11 a day |
| Ring | The Grave Robber's Prayer | +10 % gold from quests |
| Talisman | Robber Baron Ritual | +20 gold from the Arena |
- Ring example seen in the game: "+10 gold bonus on expeditions" (the user's wording of the ring enchant's action text).
- The enchant stays on the ITEM; a swap of equipment needs a new enchant (gold; price probably grows with level).
- Gems cannot be removed from an item. Epic items never go into the cauldron, except the forced free-slot sale (see the end of this file).
- Belt (Thirsty Wanderer): ONE free beer a day is ALLOWED (user 2026-10-19), but only if the belt was already worn with the enchant at the START of the day; a belt equipped during the day does not count. Needs a mushroom-exception entry in CLAUDE.md + safe.rs when implemented (only the free beer, verify via mushroom count that it costs none).

## Decisions (user 2026-10-19, also in precedents.md)
- Enchant priority: right after better items. Everything of the day's type that is non-epic and unwanted → witch (swaps, spinning, leftovers);
  during spinning buy that type so the sale pays 2×. Never buy for mushrooms because of it.
- Gem items MAY go into the cauldron (user 2026-10-19: gems are cheap to remake; we sell gem items too). Epics still never.

## Raw format of `witchshop` (verified 2026-10-19 against the game)
- From idx 5 on: triples `(slot id, enchant id, unlock timestamp)`, nine of them. Third value 0 = locked, non-zero = unlocked at that time.
  Capture: only the ring triple `9/91/1792068112` has a timestamp; the user had exactly the ring enchant unlocked (a second one unlocks 2026-10-19 14:00).
- sf-api 0.4.4 `Witch::update` is WRONG for this: it trusts idx 4 (= 1) as count and reads the first triple (gloves, 51 → ShadowOfTheCowboy) as unlocked.
  → Parse the raw `witchshop` ourselves (unlocked = third value ≠ 0); never use `witch.enchantments` for the decision. The `EnchantmentIdent` for `WitchEnchant` is `i + 1` of the triple position, but its constructor is crate-private → needs a check of how to send it (maybe `Command::Custom`).
- TODO: after 14:00 capture again and check that a second triple gets a timestamp.

## Capture 2026-10-19 (user, after enchanting the ARMOR): the enchant of a worn item in `ownplayersaveequipment`
Each item is 19 numbers; the third one (index 2) is the enchantment id, which is exactly what sf-api's `Item::parse` reads (`cfpget(2)`), so `Item.enchantment` on worn items IS reliable (only the `witchshop` unlock parsing is wrong, see above). In the capture (character level ~57 average equipment, 10 slots in the order weapon, shield-slot… by item type id: 1 weapon, 2 shield, 3 breast plate, 4 boots, 5 gloves, 6 helmet, 7 belt, 8 amulet, 9 ring, 10 talisman):
- type 3 (BreastPlate): index 2 = **31** = Mario's Beard (+50 % mushrooms on quests) ✅ armor enchant confirmed.
- type 9 (Ring): index 2 = **91** = The Grave Robber's Prayer ✅ (the earlier ring enchant).
- all other worn items: 0 (not enchanted).
So: "already enchanted?" can be read from `item.enchantment == Some(slot's enchantment)` for the worn piece; a new piece comes without it (the enchant stays on the item).

## Phase 1 implemented: read-only (2026-10-19, user: "yes, implement the Witch")
`src/witch.rs`: `session.rs` keeps the raw `witchshop` numbers of the latest response (`SimpleSession::witch_shop`), `witch::parse` reads them (layout above: idx 2/3 progress, idx 5.. nine `(item type, enchant id, unlock timestamp)` triples, idx 33 = 1 → wants nothing, else idx 34 = wanted item type, idx 35 = price), `witch::observe(session)` runs once per main-loop pass from level 66 on: writes `roster/<character>/witch.json` and logs `[witch] wants …, enchant price …, unlocked N/9: …` when it changes. Sends NOTHING (no command is whitelisted). Test `witch::tests::parses_the_capture` (the 2026-10-19 capture). Verify live: the line appears for TestChar1 (level 67) with the right unlocked enchants (ring + armor + whatever unlocked at 14:00) and the wanted item type; check whether the `witchshop` ever arrives after the login or only with it (then the data is as old as the login).
Next phases (each separately, user tests): 2) drop the day's wanted non-epic items into the cauldron (`PlayerWitchSpendItem`, whitelisted only for the wanted type, never an epic); 3) enchant worn items (`PlayerWitchEnchantItem:<ident>/1` via `Command::Custom` because `EnchantmentIdent` cannot be built; price ~2041 g, priority right after better items); 4) route item selling (swaps, shop spinning) to the witch; 5) Thirsty Wanderer free beer.

## Findings 2026-10-19 (user enchanted TestChar1, level 67, two captures 3 s apart)
- **Unlocks are server-wide** (shared by all characters), the enchanting is per character and per worn item.
- **Only ONE enchantment is offered at a time:** at first only the armor (Mario's Beard, unlocked second), after paying it the ring (The Grave Robber's Prayer, unlocked first) appeared. So the order of enchanting is the NEWEST unlock that the character does not wear yet first, i.e. the reverse of the unlock order; the next one shows up only after the previous one is applied. → The bot must enchant one item, re-read the `witchshop`, and repeat.
- Evidence in the captures: `ownplayersaveequipment` of the first (after the armor): BreastPlate `3/0/31/…` (enchant 31) and Ring `9/33/0/62`; second (after the ring): Ring `9/33/91/62`. Silver went 2 611 432 → 2 417 432 = **1940 gold** for the ring (price on this character; the earlier capture of another character showed 2041 gold → the price differs per character, read it from idx 35 every time).
- The `EnchantmentIdent` that sf-api would send is the 1-BASED POSITION of the enchantment's triple in the `witchshop` list (`i + 1`, e.g. gloves 51 = 1, ring 91 = 2); `PlayerWitchEnchantItem:<ident>/1`. NOT yet verified against the game; if the user sees the real request (browser Network tab) in phase 3, compare. Since the offer is one at a time, the ident of the offered one must be worked out from the data (which unlocked triple, not yet worn, newest `unlocked_at`).

## Phase 3 implemented: enchanting (2026-10-19, user: the risk of a wrong ident does not matter as long as the log shows what happened)
`witch::enchant` runs in the main loop right after `shops::run` (better items first, enchanting above the fortress, spinning and attributes). Per command (at most 3 per pass): the unlocked enchantment (timestamp in the past) whose item type is WORN without it, newest `unlocked_at` first; ident = 1-based position of its triple in the `witchshop` list (sf-api's rule, NOT yet verified); sent as `Command::Custom { PlayerWitchEnchantItem, [ident, "1"] }`. `safe.rs` (`custom_allowed`) lets it pass only for the own item (flag 1), ident 1..=9, level 66+, a price the server announced that we can pay and that is at most `WITCH_MAX_PRICE` (10 000 g). Costs gold only; the mushroom watchdog is unchanged.
**Check after every attempt:** the worn item of that type must now carry the expected enchantment id (`Item.enchantment`) AND the silver must have dropped. Otherwise `witch_halt.txt` (today's date) stops the witch for the rest of the day and `[witch] STOPPED for today: …` is logged.
**Log:** `roster/<character>/logs/witch.log` (uncapped): `TRY ident N for the <item>, expected enchant X, price P | before: silver, worn enchants, witch summary`, then `RESULT silver …, worn …, <item> enchant now Y, response keys: …` (the keys show whether the response carries a fresh `witchshop`), `STOPPED …` or `ERROR …`; `progress.log` has `[witch] Enchanting …` / `OK` lines.
Known gaps: the `witchshop` is refreshed only when a response carries it (we do not know yet whether the game sends it during the day), so an enchantment unlocked mid-day may be seen only after the next login; a mid-day unlock timestamp in the future is treated as locked until it passes. Test `safe::witch_tests`.

## Cauldron drop returns no response (2026-10-19, user)
When the user dropped an item into the cauldron by hand, the server sent NO response he could copy. Consequences for the phase 2 code (not written yet): send `WitchDropCauldron` with `session.send_raw_only` (does not touch the game state, so an empty body cannot trip `GameState::update` into an error and a re-login), then `Update`, and judge success by the item having left the backpack and by the witch's progress / wanted type changing in the next `witchshop`. Open: does the item disappear at once or only after a reload, does the progress bar move.

## Phase 2 implemented: the cauldron instead of a shop sale (2026-10-19, user: the item disappears at once and the gold is credited at once; the progress bar moves by tenths/hundredths of a percent)
`inventory::manage`: a `Sell` of a backpack item that `witch::wants` (level 66+, item type = `gs.witch.required_item`, not epic, not unique; gem items fine) is sent as `Command::WitchDropCauldron` instead of `SellShop`, logged as `[inventory] Dropped into the witch's cauldron (the shop would pay X, she pays twice)`. `safe.rs` (`witch_drop_ok`) lets the command through only for a BACKPACK position holding exactly such an item (never a worn one). `required_item` comes from sf-api's own parsing (`witchshop` idx 33/34, which is right; only the unlocked list was misread) and refreshes whenever a response carries a `witchshop`; if the game does not resend it during the day it is yesterday's type until the next login (to be seen in `[witch]` lines). Not yet: shop spinning for the day's type (phase 4), the free belt beer (phase 5). Not verified live; if the response is empty the next `Update` repairs the state.

## Verified live 2026-10-20 (first night after the release)
- **Enchanting works:** TestChar1 00:09:45 `TRY ident 6 for the Breast plate, expected enchant 31, price 194000` → `RESULT silver 612007 (paid 194000), Breast plate enchant now 31`. So the ident IS the 1-based position of the enchantment in the `witchshop` list (breast plate 31 = 6th). The response carries `ownplayersaveequipment` (no new `witchshop`). The breast plate had enchant 0 because a new piece had just been equipped (the enchant stays on the item).
- **Cauldron works:** Novotné (level 66) 00:05:43 dropped its replaced weapon (shop price 421.27 g) into the cauldron (today she wants a Weapon). Whether the gold credited was twice the shop price was not logged; since 2026-10-20 `[inventory] Dropped into the witch's cauldron: the shop would pay X, credited Y` + an `inventory.log` line `cauldron: …` show it.
- Prices per character: 1846 g (Novotné), 1940 g (TestChar1). `witchshop` was seen right after the midnight login; whether it is also resent during the day is still unknown.

## Phase 4 implemented: spin the shops with the witch's type (2026-10-20)
`shops::spin_candidate`: while the witch wants an item type (`gs.witch.required_item`, level 66+), the cheapest non-epic gold item of THAT type is bought first for spinning (affordable under the usual reserve rules); the inventory then sends it to the cauldron (twice the shop price) unless it is an upgrade. Without such an item the cheapest overall as before. Not verified live.
## Phase 5 (free first beer from the belt): waiting
The Thirsty Wanderer (belt) enchantment is not unlocked yet (server-wide unlocks; today only Ring and Breast plate). Nothing to implement until it unlocks and the user says what the free beer looks like in the tavern; the mushroom watchdog stays as the guard.

## Forced epic sale goes to the witch when it pays most (user 2026-10-22)
Exception to "epics never go into the cauldron", ONLY for the sale forced by the free-slot rule (`inventory::last_resort_epic`, `docs/inventory.md`). The epic to sell is the one with the highest payout: `price` normally, `2 x price` when its item type is the witch's type of the day (`gs.witch.required_item`). Example: the most expensive epic is worth 5k, but today the witch buys talismans and our second-most-expensive epic is a 4k talisman -> 8k at the witch -> that one goes to her. Same logic when the day's type is something we hold only cheap epics of: no effect, the plain most expensive is sold in the shop. IMPLEMENTED 2026-10-22: `inventory::payout`, `witch::type_matches`, `potions::execute` sends `WitchDropCauldron` for such an epic, `safe::witch_drop_ok` now allows epics (not unique ones). User confirmed the witch takes epics. Check the log line "payout" and the credited gold after the first live case. Not in a release yet.

## Enchant refused as "not whitelisted" (2026-10-22, Filminy 04:50, Chlamydie 05:58)
`PlayerWitchEnchantItem` for the Breast plate (ident 6) was refused by `safe::custom_allowed` although the same call worked for Sanek 02:25 (`STOPPED for today` in `witch.log`). The check relied on sf-api's `gs.witch.enchantment_price`; the raw `witchshop` (price 1751 g) was fine. Fix (NOT in a release yet): the whitelist accepts the price of the raw `witchshop` too (`safe::raw_witch_price`), so a missing/odd sf-api witch state cannot block enchanting. Open: why sf-api's witch state was missing/zero for those two (the cauldron worked for Filminy at 07:03, so it is not permanent). The cauldron (`witch_drop_ok`, `witch::type_matches`) still reads `gs.witch.required_item`.

## Breast plate first (user 2026-10-22)
`witch::enchant` picks the breast plate (Mario's Beard, +50 % mushrooms on quests) before any other unlocked enchant; the rest by newest unlock. Any order works (Sanek: ring then plate, Novotné: plate then ring). Every worn piece without its unlocked enchant is enchanted as soon as it is worn (already the rule). NOT in a release yet.
