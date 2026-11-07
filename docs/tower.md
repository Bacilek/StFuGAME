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
