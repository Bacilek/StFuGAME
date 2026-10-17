# Inventory trainer (user 2026-10-17)

Goal: the user cannot put the inventory rules into words, but can always pick the right move. The app tab **Inventory trainer**
(`src/app.html`, `TR` block) shows generated scenarios; the user clicks the correct move; Claude later derives the rules from the
answers and writes them into the bot (`inventory.rs`, `gems.rs`, `potions.rs`).

## How it works
- Scenario number `n` is generated deterministically from `n` (seeded RNG in JS), so the same number is always the same scenario and
  any answer can be replayed against the bot's logic. Endless; the tab goes to the next unanswered number (progress also in
  `localStorage` key `trainerAnswers`, which may be lost – the file below is the source of truth).
- A scenario: character (class, level, main/CON/LCK, CON as % of main), worn items (9 slots, normal or epic, gem slot none/empty/filled),
  ACTIVE POTIONS WITH DAYS LEFT (a big potion with 1 day left is worth far less than with 10 – user), backpack of 15 slots (epics, loose
  gems, potions, normal items, items waiting for a gem, free slots 0–8), and one NEW thing: a normal item, an epic, a potion, a gem, or
  a gem ready in the mine.
- Moves: equip (sell old / keep old), keep in the backpack, sell, socket / replace a gem into a chosen slot, drink, remove an active potion
  and drink, keep and sell a chosen backpack entry to make room, leave the gem in the mine, "I don't know / depends" + a free note.
- Answers: appended by `app.rs::save_trainer_answer` to **`roster/trainer.jsonl`** (local only): one JSON per line with `n`, `ts`,
  `choice {action, target, targetLabel}`, `note`, and the whole `scenario`.

## Rules heard so far (2026-10-17, to be confirmed by the answers)
- Gems: holding more than 1 gem is pointless when there is no room; with room a stock is fine; with 1 gem held keep mining and swap it
  unless it is a CON/main gem (depends on what is needed right now).
- Potions: most of the game main + CON + Luck potions are worn; Eternal Life is rare and almost never without mushrooms (only gold buys).
  The main kinds (maybe not Luck) should never be sold, least of all the 25 % big ones (found from level 35).
- Not yet a bot change: the potion rework of 2026-10-17 sells a smaller potion of a kind when a bigger one is active – check against the answers.

## Next steps
1. User answers scenarios (the more the better). 2. Claude reads `roster/trainer.jsonl`, groups by scenario family, lists the rules +
contradictions, asks about the contradictions. 3. Rules go to `docs/precedents.md` and the code; optionally a "what the bot would do now"
column to expose differences.

## "What the bot does today" (added 2026-10-17)
After the user picks a move (and a target when needed) a box shows what the bot would do now, green "same" / red "different", with a
one-line reason (so a wrong pick can be explained). It is NOT the Rust code: `botMove` in `app.html` mirrors the documented rules
(potions: kinds main > Eternal Life > CON > Luck, size ≥ active → drink, smaller → sold, days left ignored; gems: secondary sold, empty
socket → socket, else keep only with ≥ 2 free; items: value without gems, only worn Legendary/All gems count, epics never sold, keep
while waiting for a gem). When a log line disagrees with the mirror, fix the mirror. Each saved answer also stores `bot` and `match`.
Scenario mix: some characters have all sockets filled (`emptyP`), so "gem with no socket" cases occur.

## Visual version (2026-10-17, user: "make it visual, like the character tiles")
The tab reuses `charPanel` (paper doll, attributes, potions – potions now also show "N days left") and the backpack grid from the character
card. The NEW item is a big highlighted tile. Moves needing a target (socket / replace a gem, remove an active potion, sell something
from the backpack) are chosen by clicking the real tile, which pulses orange while it is selectable. Fixed: Druid's main attribute is INT
(sf-api `Class::main_attribute`: Warrior/Paladin/BattleMage/Berserker STR; Scout/Assassin/DemonHunter/PlagueDoctor DEX; Mage/Druid/Bard/Necromancer INT).
Sprites: `roster/assets/items/{type}_{model}_1_1.png` (a list of existing models per slot type is embedded in `TR`), gems `assets/gems/<Full name>.png`,
potions `12_<id>_1_1.png`.

## Rules derived from the first 16 answers (2026-10-17) – implemented
1. **A usable gem socket has a value** (empty, or holding a Luck/secondary gem – `inventory::replaceable`): item value for the keep/equip decision = own stats + the expected gem (`inventory::slot_bonus`: best main/CON gem in the backpack, else as strong as the best worn one). So: (a) a plainly better item WITHOUT a socket does not replace a worn item WITH a usable socket if the socket bonus outweighs the difference (epics stay in the backpack, never sold); (b) a new item with a usable socket that beats the worn one by ≥ 20 once the gem is in is kept (`keep_candidate`, now also for sockets holding a Luck/secondary gem), the gem goes in first, then it is equipped; without a gem at hand it waits (only when the worn item has no socket).
2. **Gem placement:** main/CON gem → best (epic first) item among sockets that are empty or hold a secondary gem; a Luck gem is replaced only when no such socket exists (`gems::choose_socket`). Replacing a secondary gem is free.
3. **Potions:** drink at once; many in the backpack → biggest first, all wanted kinds into free slots. Secondary potions: sold. A SMALLER potion of a kind whose bigger one is active is KEPT (not sold) while there is room; when space is tight (≤ 1 free slot) it is drunk instead if the active bigger one has ≤ 3 days left; sold only when nothing else can be sold (`potions::Pressure`, `make_room` order: potions (tight) → loose gem → potions (desperate) → kept item → epic).
4. Trainer mirror (`botMove`) now follows the real code (replace gems, free-socket preference, slot bonus, keep candidate with the "worn item has a socket" skip); replaying the 16 answers gives 12/16 agreement, the rest are open questions below.
## Open questions after the first 16
- #12: the weapon (epic) holds a Luck gem +93 and a CON +73 gem arrives; real code replaces the weapon's Luck gem (weapon gem counts double), the user socketed a worn secondary gem instead.
- #16: a Luck gem +151 – chest plate or gloves (both hold INT gems); the bot picks the item "swapped out soonest".
- #2 was answered with a mis-click (keep it, the old item stays and gets the gem).
