# Toilet of the Arcane Gods (captures 2026-10-25, NOT implemented)

Source: user captures on a level 105 character (the toilet opens at level 105 after one expedition; `docs/todo.md` and
`inventory.md` assumed level 100 - re-check). Nothing in the bot touches it yet; sf-api 0.4.4 has everything:
`Command::ToiletOpen` (`PlayerToilettOpenWithKey:`), `ToiletDrop { item_pos }` (`PlayerToilettLoad:<item_pos>`),
`ToiletFlush` (`PlayerToilettFlush:`), state in `gs.tavern.toilet` (`aura`, `mana_currently`, `mana_total`, `sacrifices_left`).
None of the three is whitelisted in `safe.rs` yet (they cost no mushrooms, verified only for these two captures).

## Captured flow
1. **Throw** an epic talisman: `PlayerToilettLoad:<pos>` ->
   `toiletstate:100/0/1` and `arcanetoilet:1/100/1792947693/60`.
   - `arcanetoilet` = aura level / mana now / timestamp (of the throw, 55 s before the flush) / mana needed to flush.
     Before the throw it was mana 0 of 60, after it 100 (one epic = 100 mana, more than the 60 needed).
   - `toiletstate` index 2 = `sacrifices_left` (sf-api): **1 after the first throw**, so it started at 2 per day
     (matches the Tidy Toilet Time event text "2 items per day"; so 2 throws/day may be the NORMAL limit - check the value
     on a day without the event). Indexes 0/1 (100, 0) unknown.
2. **Flush**: `PlayerToilettFlush:` -> `arcanetoilet:2/40/1792947693/60`, `toilettSpawnSlot:2`, a new item in the backpack,
   `timestamp:1792947748`. Aura 1 -> 2, mana 100 -> 40 (a flush costs the 60). The reward was an epic-looking warrior heavy
   armor (`3/0/0/8/0/1500/...`, item level ~100, shop level `itemlevelshop:109`) in backpack slot `toilettSpawnSlot`
   (1-based = the second slot, it filled one of two empty slots; 0-based reading not excluded).

## Conclusions / open questions
- Aura goes up per flush; the reward class/quality probably depends on the aura (`docs/pet-conditions.md`: the pet Ungere is found
  in the toilet from aura 20, "Arcane Toilet (Aura 20)" in `docs/pets.md`).
- The mana left over stays (40 now; a second throw adds 100, so the second flush is possible right after the second throw).
  Whether mana decays / regenerates with time (the timestamp field) is unknown.
- Only the number of epics counts for feeding it (`docs/inventory.md`: epics are kept for the toilet). Does a non-epic give less
  mana? Does an equipped item work (`PlayerItemPosition` also takes equipment slots)? Unknown.
- The throw capture does not show which backpack slot was thrown; log `item_pos` format when implementing.
- Tidy Toilet Time event (`docs/events.md`): 2x aura per sacrifice - still to be seen live.

## Next (when the user gives the go-ahead)
Whitelist the three commands (`ToiletDrop` only for an epic we would otherwise sell/dismantle, never a worn or better item),
`ToiletOpen` needs a `ToiletKey` item (`ItemType::ToiletKey`), then a `toilet::run` step: throw while `sacrifices_left > 0`
and an epic is spare, flush when `mana_currently >= mana_total`, log `[toilet] aura a -> b, got <item>` and keep the received
item through the normal inventory comparison.
