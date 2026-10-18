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

## Open questions (asked 2026-10-19)
See `docs/status.md`.
