# Precedents (the user's decisions)

When Claude is unsure about a decision, it asks the user and records the answer here as a precedent.
In similar situations it then follows this list. Newest at the bottom.

| Date | Situation | Decision |
|---|---|---|
| 2026-10-07 | Spending mushrooms | Never, under any circumstances, until the user changes it. |
| 2026-10-07 | Choosing an expedition | The shortest; at equal length an unmapped one, otherwise the easiest to reach 40. |
| 2026-10-07 | Encounters from a foreign mission (not the main task) | Take them only for their immediate heroism, not for future steps. |
| 2026-10-07 | Conflicting data (user × FAQ × server) | The server is right; toilet paper +20 / −5 per the user until verified. |
| 2026-10-07 | Reward after a boss | Mushrooms > gold > hourglasses; leftover expedition (Thirst for Adventure ≤ 3 min): mushrooms > hourglasses > gold. |
| 2026-10-07 | Hourglasses, beer | Do not use. |
| 2026-10-07 | Arena | Whenever off cooldown (even during an expedition), the weakest of 3, at most 10 wins a day (`fights_for_xp`). |
| 2026-10-07 | Dungeons | Whenever off cooldown, never for mushrooms, not with a full inventory; lowest level, at a similar level weaker stats. |
| 2026-10-07 | Inventory | Equip better, sell worse, never sell epic items. |
| 2026-10-07 | Weapon value | Damage (average × (1 + M/20)) + the weapon's other stats 80 % CON / 40 % LCK / 10 % secondary. Ignore armor. |
| 2026-10-07 | Git | Commit and push after every change. |
| 2026-10-07 | Arena losses | Fine, do not tune the formula: opponents rotate, honor drops, weaker opponents next time. |
| 2026-10-07 | City Guard | After the Tavern is done; at most 10 h, ending 00:00–00:59 (hours until midnight rounded up) so we do not wait for the new Thirst for Adventure. |
| 2026-10-07 | Arena/Dungeons during City Guard | Works, nothing gets in the way. |
| 2026-10-07 | Wheel of Fortune, daily rewards | Only the free spin once a day (never mushrooms/lucky coins) and the daily login bonus. Nothing more. |
| 2026-10-07 | Stable (mushroom exception) | Griffin for 25 mushrooms for 14 days, only when the character has no mount and heads to the Tavern (buy only when needed). Without 25 mushrooms a tiger for 10 g + 1 mushroom. |
| 2026-10-07 | Logs in the GitHub repo | Fine (the user may make the repo private). |
| 2026-10-07 | Language | The whole repo in English (game terms from the English client), except `docs/todo.md`. Commit messages in English. Communication with the user stays Czech. |
| 2026-10-07 | Starting the bot | The user starts it themselves (desktop shortcut); do not start it for them unless asked. |
| 2026-10-07 | Money in logs | Always gold (silver / 100, e.g. 50 silver = 0.50 g), never "g + s". |
| 2026-10-07 | Progress log | Keep only the last 100 messages in `logs/progress.log`. |
| 2026-10-07 | Arena log | Only date, fight of the day, opponent, won, honor, gold, xp (no time, strength, `fights_for_xp`). |
| 2026-10-07 | Old Czech log, README | Translate everything to English (old log translated into `progress.log`), write an English README. |
| 2026-10-07 | Multi-character challenge (idea, not implemented yet) | ~10 characters, each a different class, all on the same world, probably 10 accounts with one character each; run ~14 days, then compare strength. Attribute split (`UpgradeSkill`): one fixed key for all classes; the class's primary attribute is read from the character (the user will look it up if the API does not say). All classes are unlocked, no conditions. |
| 2026-10-07 | Weapon Shop, Magic Shop | Never for mushrooms, only gold. After the whole Thirst for Adventure. Buy any upgrade (even 1 stat point) and equip it; epic items never sold. Spare gold → spin (buy, sell right away) until all items cost mushrooms. Reserve = the most expensive gold item seen that day (upgrades may use all gold). |
| 2026-10-07 | Choosing a guild | From the quick-join list in the Guild tab: Instructor first, then Treasure, then the overall guild strength. |
| 2026-10-07 | Changing the guild | Once a day check the quick-join list and switch only to a clearly better guild (proposal: Instructor at least 10 higher), not for small differences. Joining again works right away, only ~12 h without guild fights. |
| 2026-10-07 | Guild attacks and defenses | Sign up automatically for every planned guild attack (incl. raids) and defense. |
| 2026-10-07 | Goblin Gleeman + event tasks | Do what costs only gold or nothing, claim the chests; never mushrooms. Event tasks under the same rules. Day 1 of a new character matters most (10 mushrooms). |
| 2026-10-07 | Guild skill upgrade (task) | The cheaper of Treasure/Instructor; Instructor on equal price. Gold only. |
| 2026-10-07 | Shell game (3 shells, ~33 %) | Allowed, but not an income: smallest bets, when gold is low (after shopping / before the Thirst for Adventure), only when the points are needed for a chest; the top chest does not need every task. Playable from 5 gold, bet ≤ 1/10 of the gold. |
| 2026-10-07 | Attributes for tasks | Yes, but never so that better equipment cannot be bought (after the shops, keep the shop reserve). |
| 2026-10-07 | Beer (mushroom exception 2) | Default: only when "Drink beer" is the last missing task for the max reward (10 mushrooms) and nothing else can be completed; 1 mushroom for 10 is worth it. Events may allow more later. |
| 2026-10-07 | Expedition location/type tasks | Prefer the expedition with the task location (or type), even when longer / more Thirst for Adventure. |
| 2026-10-07 | "Win against <class>" / bare hands tasks | If not done through the XP fights or it is late: Hall of Fame far below our rank, weak low-honor player (of that class); bare hands = weapon off, fight, weapon back. Hall of Fame fights do not count towards the XP wins. |
| 2026-10-07 | Wheel of Fortune task (e.g. 5 spins) | 1 free, the rest for lucky coins only when a reward would not be reached otherwise; do not waste lucky coins. |
| 2026-10-07 | Beer task with 10 beers | Only when it brings more (e.g. 10 mushrooms back + Thirst for Adventure); otherwise careful, 10 mushrooms is a lot. Implemented as: chest mushrooms ≥ beers. |
| 2026-10-07 | Costly tasks in general | Decide by what the chests actually contain (rewards), not just points. |
| 2026-10-07 | Attribute tasks vs. spinning the shops | When the attribute task helps to a better chest, it goes before spinning the shops. |
| 2026-10-07 | Hourglasses | Save them, never use. Buying them for gold in the shop (spinning) is fine. |
| 2026-10-07 | Potions | Main attribute + CON + Eternal Life (only for gold), otherwise Luck third. Only gold. After equipment, before spinning. Bigger one replaces a smaller active one with ≤ 3 days left (esp. main attribute); long stacked ones are a harder question. Eternal Life while Luck is active: buy and keep in the backpack. |
| 2026-10-07 | Potion stock | Keep up to 4 potions in the backpack (buy whenever for gold, not only when one runs out). Full backpack → drink/sell from the least important. Replacing a smaller active potion only with a full backpack, otherwise store the bigger one. |
| 2026-10-07 | Potion stock (update) | Non-target potions may be kept too (better some than none), they go first when there is no room. Buying gold potions is also good for spinning the shop. With a full backpack remove a non-target active potion and drink a better one ("never remove" was wrong). Potions sell in either shop. Same type stacks by its full duration. |
| 2026-10-07 | Friends' roster | `roster/roster.md` (classes, nicks, friends' names) stays local only – `roster/` is in .gitignore (the repo is public). |
| 2026-10-07 | Shell game bet | Keep the minimal bet (1 silver); 10 losses in a row is just bad luck, no need to worry. |
| 2026-10-07 | The user's own character | The user plays their own character on another account while the bot runs – fine, the bot must not log in to that account. |
| 2026-10-07 | Dashboard colours | Colour per class: Warrior steel grey-blue, Scout light brown, Assassin orange, Battle Mage purple, Berserker red, Druid light green, Demon Hunter pink, Bard light blue, Necromancer turquoise, Paladin yellow, Plague Doctor dark green; Mage not specified → blue. |
