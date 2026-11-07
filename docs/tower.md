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
