# Blacksmith (unlocks at character level 90)

Source: the user 2026-10-24 + sf-api 0.4.4 (`Command::Blacksmith`, `Item::dismantle_reward`, `gs.blacksmith`) + one raw capture of a
dismantle (a ring: several hundred Metal and 12 Arcane Splinters). Code: `src/smith.rs`; whitelist `safe.rs` (`dismantle_ok`), tests
`safe::blacksmith_tests`, `smith::tests`, `inventory::tests::epic_is_sold_only_from_a_full_backpack`.

## Phase 1 (implemented 2026-10-24, NOT in a release yet): dismantling only, resources are just collected
Rules (user):
1. A PLAIN (non-epic) item that is to be got rid of (it used to be sold: worse than the worn one, other class, bought for shop
   spinning, loot from the dungeons / expeditions, ex-equipment, ...) goes:
   - to the witch if she wants that item type today (she goes first, even for items that would pay splinters),
   - else to the Blacksmith if its dismantling pays ARCANE SPLINTERS (`smith::splinters` = `Item::dismantle_reward().arcane > 0`),
   - else (Metal only) to the shop as before.
2. EPICS stay never-sold. New from level 90: in the one situation where an epic used to be sold (`inventory::last_resort_epic`: the
   backpack is full, one slot must stay free) the epic with the MOST Arcane Splinters in the BACKPACK (never a worn one) is
   dismantled instead of the most expensive one sold. Without dismantles left it falls back to the old payout rule (witch 2x / shop).
3. Dismantles are limited per day: 5, 15 during the Forge Frenzy Festival (`docs/events.md`). The server decides: `gs.blacksmith.
   dismantle_left`; an unknown counter (never seen) or one from an earlier day counts as "try"; a refused dismantle (any error that
   is not a lost session) stops dismantling for the rest of the day (`smith::REFUSED_ON`) and the item is sold instead.
4. Metal and Arcane Splinters are only collected; nothing is spent yet (no item upgrades, no socket upgrades, no gem extraction).

Commands: `Command::Blacksmith { item_pos, action: Dismantle (201), item_ident }` = `PlayerItemMove:<pos>/201/-1/<ident>`. Only
`Dismantle` is whitelisted (`SocketUpgrade`, `GemExtract`, `Upgrade` and every `...WithMushrooms` stay forbidden), only for a backpack
equipment item (never worn, never unique), level 90+.

## Capture (user 2026-10-24, response of a dismantle)
`timestamp:<t>&resources:...&smith:5/<t>&characterstatus:...&backpack:...&dailytasklist:...` - the response carries `resources` (idx 9 =
Metal, idx 10 = Arcane Splinters in sf-api), `smith:<dismantles left>/<time of the last dismantle>`, the new backpack and the daily
task list (the `BlacksmithDismantle` task, type 9, counts 5 points per dismantle; 15 in the Forge Frenzy Festival).

## Verification status
| Item | Status |
|---|---|
| Dismantle command (`201`) accepted by the server | user captured it by hand, the bot has NOT sent it yet |
| `dismantle_reward` (sf-api port of the community simulator) matches the game | UNVERIFIED - every dismantle logs `[smith] Dismantled ...: Metal +a (predicted b), Arcane Splinters +c (predicted d) - as predicted / MISMATCH` (progress.log + `inventory.log`); fix the formula on a MISMATCH |
| `smith:` counter is part of every `Update`, not only of the dismantle answer | UNVERIFIED (the code tolerates both) |
| Daily limit really 5 (15 in the event); does `dismantle_left` count down from the cap or up | UNVERIFIED (capture shows `5` after one dismantle during the event) |
| A socketed gem is lost with the item (dismantle or shop alike) | assumed, UNVERIFIED |

## Ideas for later phases (nothing agreed)
- Spend Metal/Splinters: `Item::upgrade_costs` (+3 % of the highest attribute per upgrade, max 20 upgrades) on WORN items; needs an
  upgrade capture and the user's rules (which items first, reserve for the next dismantle-payback, ...).
- Socket upgrade / gem extraction (`SocketUpgrade`, `GemExtract`): needs captures.
- Forge Frenzy Festival: dismantle every day up to the 15 (task points), splinters from expeditions (reward order unknown).

## Waiting for midnight instead of selling an epic (user 2026-10-24, implemented, NOT in a release yet)
Level 90+, backpack full, only epics left and NO dismantle left today (`smith::waiting` = unlocked but not `available`, includes "refused
today"): `inventory::last_resort_epic` returns nothing, so no epic is sold. The full backpack stops the Dungeons (`[dungeons] Backpack
full of epics and no dismantles left today: waiting for midnight, no fights`) because nothing new could be picked up anyway; at the new
day the stale counter counts as "try" and the epic with the most splinters is dismantled. Below level 90 the old sale rule stays.

## Capture: upgrading a worn item (user 2026-10-24, armour level 1 -> 2; NOT automated, syntax only)
Command `Command::Blacksmith { item_pos, action: Upgrade (204), item_ident }` = `PlayerItemMove:<pos>/204/-1/<ident>` (same shape as
the dismantle `201`). The response carries `ownplayersavecharacter`, `resources` (Metal idx 9, Arcane Splinters idx 10: 20078 / 8354
after, the previous capture said 21787 / 8035, so other things happened in between and the exact price is not isolated),
`ownplayersavepotions`, `ownplayersaveequipment` (the upgraded worn item), `ownequipmentaveragelevel`, `itemlevelshop`, `deeds`. Price
per upgrade: `Item::upgrade_costs()` (sf-api); the item's `upgrade_count` goes up by one. Still forbidden in `safe.rs`.
