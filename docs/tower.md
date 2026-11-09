# Tower companions (design from the user, 2026-11-07; NOTHING implemented yet)

Nobody has the Tower yet (needs character level 125 + Light Dungeons 1-9 cleared; closest 2026-11-07: TestChar1, Mimimimi11, Novotné, last Pyramids of Madness).
Tower fight = 4 vs 1 boss: the main character + 3 companions (Bert = warrior without shield, Mark = mage, Kunigunde = scout/archer). They have the
character's level and NO equipment; we have to supply it.

## Rule (user 2026-11-07)
- The main character is ALWAYS offered gems and equipment first (unchanged). Only what we would otherwise get rid of goes to the companions.
- Disposal order for any item/gem we would get rid of: **companion > wash > toilet (TTT) > toilet (non-TTT) > blacksmith > witch > sell**.
- Companion offer: compare with the companion's current item of the same slot using the SAME power formula as the main character (Arena formula on
  the item's attributes, weapons by damage); only items of the same class are ever compared, so class-specific damage/stat ratios do not matter.
  Better -> equip, the old item falls out into the backpack and goes through the same chain (companion offer is moot for it, then wash, ...).
- Wash (`PlayerToilettWash`): free (no mushrooms), unlimited a day, once per item (`Item::is_washed`). Same item type (glove stays glove), another class
  (if it has one) with other stats; gems stay. Tried on items we would sell/throw and on the item just taken off. Epics are washed too (washed epics can be thrown into the toilet).
- Dug-out gems (luck / secondary; also main attribute, CON, black, legendary) are checked for a companion that would benefit, after the main character.
- Amulet, Talisman, Ring are class independent (magic shop sources); the rest of the armor is class specific.

## Protocol captures (user 2026-11-07, main character with Tower)
- Equip = `PlayerItemMove:<from_pos>/<101+companion>/<slot>/<ident>` (sf-api `Command::EquipCompanion`). Companion order in `companionequipment`: warrior, mage, hunter
  (same as `warriorcompanionaverageequiplevel`, `magecompanion…`, `huntercompanion…`). Response keys: `ownplayersavecharacter`, `owntower.towerSave`, `backpack`,
  `companionequipment`, `*companionaverageequiplevel`, `itemlevelshop`. Epic weapon swapped for epic on Bert, non-epic for epic on Mark: both fine, the old item lands in the backpack.
- Wrong class: `error:class cannot use this item` (wizard glove onto Kunigunde) -> check `Item::can_be_equipped_by_companion` first.
- Wash: `PlayerToilettWash:` response is just `toiletstate:0/1/0` + `backpack:` (no resources -> free). Ring 344 CON -> 344 CON (coincidence); epic heavy armor -> medium armor, same stat total.
- sf-api 0.4.4 has `EquipCompanion`, `WitchEnchantCompanion`, `Item::is_washed`, `can_be_equipped_by_companion`; no Wash command (needs `Command::Custom`, allowed in `safe.rs`).

## Open
- Full request line of `PlayerToilettWash` (arguments).
- Gem sockets on companions' items, how to read companion stats for the power formula.

## Wash probe (2026-11-07, `src/wash_probe.rs`, NOT in a release yet)
- Captured request is a bare `PlayerToilettWash:` (no payload) right after a `Poll`; in the client the item is dragged onto the "wash cycle" tub, which sends nothing else.
  The item washes in place in its backpack slot. So how the server picks the item is unknown.
- One-shot probe, runs when `roster/<character>/wash_probe.todo` exists (deleted first): A) bare Wash; B) if nothing washed, `ToiletDrop` (`PlayerToilettLoad:<pos>`) of an
  unwashed backpack epic + bare Wash. Logs `[wash-probe]` lines with the answers. `safe.rs` allows `PlayerToilettWash` with no arguments only when the toilet is unlocked.
  Marker created for TestChar1 (user agreed 2026-11-07; the toilet is already used up for the day, so nothing can be flushed by accident).

## Tower fight (user 2026-11-07)
- `PlayerTowerBattle:<current_level>/<use_mush>` (sf-api `Command::FightTower`; `use_mush` always 0). The Tower fight uses the SAME hourly dungeon cooldown as the
  normal dungeons (it takes the slot of one dungeon fight), so a lost Tower fight costs a dungeon fight.
- The Tower boss is far stronger than the same-level light dungeon enemy: the user beats a level 200 normal dungeon enemy alone easily, but in the Tower 4 v 1 (level 200) it lost.
  Capture (loss): boss level 200, 16,019,700 HP, damage/attributes ~4.2k/1.7k/1.7k/15.9k; response has 4 fights in sequence (`fightheader1..3` = the companions, `fightheader4` = the character),
  the boss HP carries over from fight to fight (16,019,700 -> 14,084,360 -> 13,072,184 -> 11,853,432), `winnerid*.s:0` = the boss won all four, `fightresult.battlereward` all zeros.
- Rule (user 2026-11-07, corrected): the Tower is NOT fought just because it is the lowest level. A lost Tower fight has already used the cooldown, so NOTHING is fought right away
  (that would cost a mushroom, never); the NEXT free slot (next hour) goes to another dungeon, not to the Tower again. Try the Tower again only when our side got stronger (companion gear / levels)
  or the loss was close. Needs a win estimate (sf-api `simulate` supports companions) before spending the slot; not built yet. The loss above was NOT close (user).
- Win reward (user 2026-11-07): +1 % City Guard reward, permanent, and always 1 epic (needs a free backpack slot).

### Wash probe result 2026-11-07 16:56 (TestChar1, Talisman epic at backpack slot 2)
- A bare `PlayerToilettWash:` -> `Server responded with error: server not available`; nothing washed.
- B `ToiletDrop` (`PlayerToilettLoad:`) -> `toilett has been used today` (the day's sacrifices were used up), so the Load+Wash sequence could not be tested today. The second Wash was then
  blocked by `safe.rs` ("not whitelisted", probably the session state was gone after the server error), so it never reached the server.
- The probe now re-arms itself (recreates the marker) when the Load is refused and tries again the next day (after midnight, when the sacrifice is available). It will THROW the epic into
  the toilet (the Load is a real sacrifice, a flush may then burn it) if the Wash is not what takes it out again - accepted by the user (an unused epic).

## Capture 2026-11-08 (user: epic weapon taken off Bert) – REQUEST LINE MISSING
The user pasted only the response (the request line came out as a bare `PlayerItemMove:`); ask for the full request (from/to/slot/ident).
Response keys: `ownplayersavecharacter`, `owntower.towerSave` (companion blocks of 148 numbers from index 3: level at +0, attributes from +4, so the Tower save is already parsed by sf-api),
`characterstatus`, `subscriptionstatus`, `ownplayersavepotions`, `backpack`, `companionequipment` (3 x 10 slots, 12 numbers per item, empty slots all zeros, as in the equip capture),
`warrior/mage/huntercompanionaverageequiplevel` (142/165/170) and `itemlevelshop` (186). Reading: the taken-off item is in the backpack and the companion slot is empty,
so an unequip IS possible without a swap (to verify against the request: the target should be a backpack position).
Decision (user 2026-11-08): implement Tower / Twister / Shadow World "for the characters that get them" – scope and blockers in status.md.

Companion equip request decoded (user 2026-11-08, base64 of `params` in the URL): `2/1/101/9/1/52/6557379/0` = from backpack(2)/position 1 -> companion 101 (Bert) slot 9 (weapon) with item ident `1/52/6557379/0`. The unequip request (companion slot -> backpack) was NOT captured yet (the user only saw the equip one).

Unequip DECODED (user 2026-11-08, base64 `params` `MTAxLzkvMi8xLzEvNTIvNjU1NzM3OS8w`): `PlayerItemMove:101/9/2/1/1/52/6557379/0` = companion 101 (Bert) slot 9 (weapon) -> backpack(2) position 1, ident `1/52/6557379/0`.
So the generic shape is `PlayerItemMove:<from_kind>/<from_pos>/<to_kind>/<to_pos>/<ident>` with kind 2 = backpack, 101/102/103 = companions (equip = `2/<pos>/101/<slot>/<ident>`).
A swap-free unequip works as long as the backpack has a free slot; needs `Command::Custom` (sf-api has no typed unequip) + a `safe.rs` entry.

Wash request DECODED (user 2026-11-08, base64 `params` `Mi8z` = `2/3`): `PlayerToilettWash:2/3` = backpack (2), position 3, item in place, response `toiletstate:0/1/0` + `backpack:`. The bare call of the earlier probe was wrong.
`wash_probe.rs` now sends `PlayerToilettWash:<pos>` (safe.rs: one argument `2/<n>`, unlocked toilet); marker re-armed on TestChar1 (NOT in a release yet). Next: real wash logic in the disposal chain.

## Step 1 IMPLEMENTED 2026-11-08: companion equipment (`src/companions.rs`, NOT in a release yet, no character owns the Tower so it never ran)
`companions::offer` runs at the start of `inventory::manage`: a backpack equipment item that the main character would NOT wear (other class, or `compare` says not better) goes to the companion
of its class when it beats the companion's item in the slot (weapons by average damage, rest by the Arena power formula, +1 % margin; an empty slot always gains; a worn item with a filled gem is left alone).
`Command::EquipCompanion` is whitelisted in `safe.rs` with `companion_equip_ok` (same ident, the class may wear it, never a shield, companions exist). The old item falls into the backpack and is disposed of as usual.
Not done: unequip (not needed for a swap), the wash step of the chain (waits for the probe), dug-out gems for companions, companion enchants. Log lines `[companions] Equipping ...`, journal `companion ...` in inventory.log.

## Decisions 2026-11-08 (user)
- Companion enchants (witch): NOT handled, not planned.
- Gems go to companions only if the main character would not use them (same "main first" rule as for items). Not implemented: needs a capture of socketing a gem into a companion's item (and whether that is possible).
- CORRECTION of the gem rule (user 2026-11-08): a gem also goes to a companion when it would only give the main character a MINIMAL gain (example: worn weakest gem main +300, mined main +301 -> better into an EMPTY companion socket of that main stat than +1 for us). Compare: gain for the main character (`inventory::gem_gain`/`socket_gain`) vs. the value of filling an empty companion socket; "minimal" = FIXED by the user 2026-11-08: main gain below 5 % of the mined gem's value, and the companion socket must be empty. Still not implemented (needs the socket capture).
- Slots (user 2026-11-08): no forbidden slots; Kunigunde has the ordinary scout slots, Mark the mage ones, Bert the warrior ones without a shield (matches `can_be_equipped_by_companion`, no code change).

## Capture 2026-11-08: Tower floor 1 WIN (user, main character level 162, Light dungeons done)
Request `PlayerTowerBattle:1/0` (base64 params `MS8w` = `1/0` = floor 1, use_mush 0; sf-api `FightTower { current_level: 1, use_mush: false }` matches).
Response (keys in order): `fightresult.battlereward`, `battlerewarditem`, `owntower.towerSave`, `dungeonlevel`/`dungeonprogresslight`/`dungeonprogressshadow` (the dungeon lists again),
`ownplayersavecharacter`, `resources`, `fightversion`/`fightheaderN.fighters`/`fightequipmentN`/`fightN.r`/`winneridN.s` x4, `characterstatus`, `backpack`, `companionequipment`, `*companionaverageequiplevel`, `deeds`.
- **Four fights**: 3 companions first (Bert, Mark, Kunigunde: fighters with id -391/-392/-393 are the TOWER enemies, `winnerid1..3.s` = 0 = enemy won / companion lost?), then the main character (`winnerid4.s` = 23994 = OUR player id = WIN). The boss HP (16 019 700) is carried over from fight to fight (`fightheaderN` hp field: 16019700 -> 14646931 -> 13616716 -> 10427967), the main character finishes it (-1 594 063). So "close loss = rest of the boss HP" is readable from the last `fightN.r` / the carried HP.
- `battlereward` = `1/0/26746110/0/...`: index 2 = XP 26 746 110 (character XP 1 776 676 -> 22 204 760 of 22 705 735 needed: ~90 % of a level 162 from ONE floor!). `battlerewarditem` = one item (epic, `5/1/0/1053/...`) that lands in the backpack (first slot).
- `owntower.towerSave`: index 0 = player id, **index 2 = floors cleared (0 -> 1)**, index 3 = character level; companion blocks (148 numbers each) follow; sf-api already parses it.
- `characterstatus` idx 6: 1500 -> 4500 (unknown, maybe tower related), idx 7: 0 -> 10.
- Pending: does a win give the epic every floor (user said "+1 % guard" earlier), cooldown between floors (none seen), what a LOSS returns.
- Tower (user 2026-11-08): the cooldown between floors is the normal dungeon-style cooldown; only a Twister win with 90+ % HP left is free.

## Capture: a gem onto a companion's item (user 2026-11-09, Mark's item at slot 5, an Intelligence gem)
`PlayerItemMove:2/1/102/5/15/22/5703296/0` = backpack position 1 -> companion 102 (Mark), slot 5 (`EquipmentSlot` 5 = Amulet in sf-api numbering; the user
said "head", so the label/slot numbering may differ for companions - to be checked on the first live socket), item ident `15/22/5703296/0` (type 15 = gem).
It is the SAME wire shape as an equip, so `Command::EquipCompanion` carries it. Implemented 2026-11-09: `companions::gem_target` (an item of the
companion with an EMPTY socket and a gem of the class's main attribute; weapon first) is tried in `gems::socket_backpack_gem` when the main
character gains next to nothing (below `companions::MINIMAL_SHARE` = 5 % of the gem's value, or no socket at all); `safe.rs::companion_equip_ok`
accepts a gem only onto an item with an empty socket. NOT in a release yet; nobody owns the Tower companions on the bot's side until a character
unlocks the Tower, so it stays idle until then. Verify `[gems] Socketing ... into <Companion>'s ...`.

## Capture: Tower floor 2 WIN (user 2026-11-09, `PlayerTowerBattle:2/0`, params `Mi8w`)
Same shape as floor 1: four fights (`fightheader1..4`), the three companions lose one after the other (`winnerid1..3.s:0`) and wear the boss down
(HP 6 552 840 -> 5 121 904 -> 4 729 067 -> 4 236 618), the main character wins the last one (`winnerid4.s` = the player id 23994). `battlereward` index 2
= XP 27 958 275, `battlerewarditem` = an epic. `owntower.towerSave` index 2 = 2 (floors cleared). Still missing: a LOSS capture.
