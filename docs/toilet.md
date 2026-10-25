# Toilet of the Arcane Gods (captures 2026-10-25, NOT implemented)

Source: user captures on a level 105 character during the Tidy Toilet Time event (the toilet opens at level 105 after one
expedition; older docs said 100 - re-check). Nothing in the bot touches it yet; sf-api 0.4.4 has the commands:
`ToiletOpen` (`PlayerToilettOpenWithKey:`), `ToiletDrop { item_pos }` (`PlayerToilettLoad:<pos>`), `ToiletFlush`
(`PlayerToilettFlush:`); the wash (`PlayerToilettWash:`) is NOT in sf-api (would be `Command::Custom`). State: `gs.tavern.toilet`
(`aura`, `mana_currently`, `mana_total`, `sacrifices_left`). None is whitelisted in `safe.rs` (none costs mushrooms in the captures).

## Rules (user 2026-10-25)
- **Throwing:** mana per item: epic 50, gem 30, plain item 25, potion 10 (x2 during Tidy Toilet Time: epic 100). NEVER throw potions or
  plain items; only epics (especially during the event) or gems (practically an unlimited free supply, see `docs/fortress.md`).
- **Daily limit:** `toiletstate` index 2 = `sacrifices_left`. During the event it is 2 a day (1 after the first throw, 0 after the
  second); outside the event probably 1 (UNVERIFIED, the user expects it). So keep >= 2 epics per event day (6 for Fri-Sun).
- **Flushing:** costs 60 mana (`arcanetoilet` field 3), not limited by the daily counter, only by mana; each flush = +1 aura (max 400)
  and a random item (the better the higher the aura, to be seen). Spawn slot `toilettSpawnSlot` is 1-based in the backpack.
- **Purpose:** this is why the epics were kept. It becomes THE way to get rid of epics when the backpack is full: no longer the
  Blacksmith, selling or the witch for epics (these remain for plain items; `docs/blacksmith.md`, `docs/inventory.md`).
- **Wash** (`PlayerToilettWash:`): turns an item of our class into another class (useful from level 125 for the Tower). Capture: the
  item's class field 3 -> 2 and its last field 0 -> 1, `toiletstate:0/1/0` (index 1 = washed flag / washes used). Costs/limits unknown.

## Captures (all 2026-10-25)
| step | response |
|---|---|
| throw epic talisman | `toiletstate:100/0/1`, `arcanetoilet:1/100/<t>/60` (aura 1, mana 100) |
| flush 1 | `arcanetoilet:2/40/<t>/60`, `toilettSpawnSlot:2`, new epic-looking armor |
| throw epic axe (backpack slot 0) | `toiletstate:0/0/0`, `arcanetoilet:2/140/1792948560/60` (+100) |
| flush 2 | `arcanetoilet:3/80/...`, `toilettSpawnSlot:1`, item level shop 110 |
| flush 3 | `arcanetoilet:4/20/...`, `toilettSpawnSlot:1`, item level shop 111 |
| wash | `toiletstate:0/1/0`, one item's class changed |
`arcanetoilet` = aura / mana / time of the last throw (unchanged by flushes) / mana per flush. `itemlevelshop` rises 1 per flush (109 -> 111).

## Next (when the user gives the go-ahead)
Whitelist throw/flush (`ToiletDrop` only for a spare epic or gem, never a worn or better item, never a potion/plain item), a
`toilet::run` step: throw while `sacrifices_left > 0` and a spare epic/gem exists, flush while `mana >= 60`, log `[toilet] aura a -> b,
got <item>`, let the new item go through the normal inventory comparison; epics without backpack room go to the toilet instead of
`last_resort_epic`'s sale/dismantle. `ToiletOpen` needs a `ToiletKey` item (`ItemType::ToiletKey`). Pet Ungere appears at aura 20.
