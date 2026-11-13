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
`last_resort_epic`'s sale/dismantle. There is NO toilet key (user 2026-11-02): it opens by itself at level 100. Pet Ungere appears at aura 20.

## IMPLEMENTED 2026-10-25 (user, NOT in a release yet, never seen live): `src/toilet.rs`
- Runs in the main loop right after `inventory::manage` (before potions). Needs `gs.tavern.toilet` (`arcanetoilet` level > 0); the Open-with-key step is not automated.
- **Sacrifices (user):** outside the event 1 a day, preferably a gem (an epic only with a full backpack); during Tidy Toilet Time 2 a day, preferably epics (gem as fallback). `toilet::candidate`; epic = cheapest backpack epic equipment (not unique, not legendary), gem = `inventory::spare_gem`. Potions and plain items are never thrown (`toilet::throwable`, also enforced in `safe.rs::toilet_drop_ok`).
- **Counting:** the server's `sacrifices_left` is only refreshed by responses that carry `toiletstate`, so the day's own count lives in `roster/<char>/toilet_day.txt` (`date n`, 99 = refused); a day with no throw yet is always tried once. After the throw the item must be gone from its slot, else "refused for today".
- **Flush** whenever `mana_currently >= mana_total` (the cap/cost can change) and a backpack slot is free (`safe.rs::toilet_flush_ok`); max 20 per pass. The received item goes through the normal inventory pass: equip if better, else witch (her type of the day) / Blacksmith (pays Arcane Splinters) / shop; a received epic stays for the next sacrifice.
- **Epics wait for the toilet:** with the toilet unlocked `inventory::last_resort_epic` returns nothing (no sale / dismantle / enchant for a slot); a full backpack with the day's sacrifice used simply waits until the next day (Dungeons pause like with the Blacksmith).
- Logs: `[toilet] Threw a ... in: <state> -> <state>`, `[toilet] Flushed: ... got ...` (also in `inventory.log`). Live checks: that `sacrifices_left` / mana show up in the state at login, the day file, that `last_resort_epic` no longer sells epics.
- Wash: not implemented (no use, user).

## BUG fixed 2026-10-26: a locked toilet counted as unlocked
sf-api creates `gs.tavern.toilet` from `toiletstate` (sent to locked characters too); only `arcanetoilet` with a level > 0 fills `mana_total`. `toilet::owns_epics`/`candidate`/`run` and `safe::toilet_drop_ok` used `toilet.is_some()`, so TestChar1 (level 81) kept 14 epics in a full backpack ("the toilet owns them", nothing sold, Dungeons stopped; found via the bell's first alert). Now `toilet::unlocked(gs)` = `mana_total > 0`. The user's rule stands: below level 90 (no Blacksmith) and without a toilet the most expensive epic is sold when the backpack is full.

## Check 2026-11-01
Only TestChar1 (lvl 116) has the toilet open (flushes, aura 7). Chlamydie/Filminy/Novotne/Mimimimi11 (lvl 105-117) report `mana_total` 0 = locked: the bot never calls `ToiletOpen` (needs a ToiletKey item), so their epics pile up. Open question for the user: open it by hand, or automate `ToiletOpen`?
- 2026-11-02: room for a Tavern reward (2 slots): loose gem -> potion -> kept item -> socket swap -> TOILET epic (within the daily limit) -> only then the last-resort epic (never with an unlocked toilet).

## 2026-11-04: the toilet unlock is 6/1 on some characters
Five level 105-125 characters waited with pending unlocks `3/1, 6/1`; the user accepted 6/1 by hand (PajaRizz) and got `arcanetoilet:1/0/0/60`. `toilet::run` now sends 32/1 and 6/1 (`safe.rs` whitelists both). 3/1 is still unknown and not sent.
- Live 2026-11-04 (release 21:38): the `6/1` unlock was sent (pending unlock gone), but afterwards `state` stayed aura 0, mana 0/0: neither `UnlockFeature` nor `Update` returns `arcanetoilet`, only a fresh login does. `toilet::run` now returns `SessionLost` right after an unlock that did not bring it (relogin at once; before it waited 2 h). Characters already unlocked get it with the next login (a bot restart).
- **CORRECTION 2026-11-05 (user: the toilet has no key, it unlocks with a message after level 100):** the unlock is the pending `3/1` at level 100+, NOT `6/1`. Evidence: PajaRizz's login capture after the user accepted the news had only `6/1` pending (3/1 gone) and `arcanetoilet:1/0/0/60`. The 6/1 sent on 2026-11-04 (release 22:04) was a mistake (what 6/1 is: unknown, it was consumed on Mrožik, Wecros, Květoš, Sanek, Pagan, Novotné; the user accepted it by hand on PajaRizz anyway, no harm seen). `toilet::run` sends 32/1 and 3/1 (level 100+), `safe.rs` whitelists 32 and 3/1 only.

## 2026-11-07: something goes in EVERY day (user)
- Order of the sacrifice: epic (event, or a full backpack) -> loose gem (weakest by `gem_effect` main+other, i.e. a secondary/Luck gem first, a worse main-stat gem when the sockets are full) -> from 20:00 the CHEAPEST PLAIN equipment item (`toilet::spare_plain`, never the item kept for its gem slot). Potions are still never thrown. The event day needs 2, other days 1.
- Loose secondary/Luck gems used to be sold at once (`gems::sell_secondary_gems`), so the toilet never saw them: while a sacrifice is owed today the weakest `toilet::gems_to_keep` of them are held back (event: fewer when spare epics cover the quota).
- The plain fallback needs an item to exist: from 20:00, with a sacrifice owed and no epic/gem, `inventory::next_action` does not sell the cheapest plain item (`toilet::plain_reserve`); `toilet::run` throws it right after. `safe.rs::toilet_drop_ok` / `throwable` now accept plain equipment (not unique/legendary, not potions).
- Not live yet (built dev only): watch for `[toilet] Threw a plain ...` after 20:00 and a gem held back at `[gems]`.

## Epic surplus rule (user 2026-11-13, replaces the full-backpack and tight-day (2026-11-08) rules)
An epic is the preferred sacrifice only during Tidy Toilet Time, when the Tavern needs room (`need_room`), or when the backpack holds
MORE than 6 throwable epics (`toilet::epic_surplus`; one event weekend uses at most 2 x 3 = 6). Otherwise a gem (from 20:00 a plain
item). A full backpack or a tight day alone no longer triggers it; `toilet_tight.txt` is no longer written.
