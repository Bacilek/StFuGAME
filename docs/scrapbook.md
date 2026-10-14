# Scrapbook ("Library of Meticulousness") – spare Arena fights + fill chart

Rules (user 2026-10-12): the fuller the scrapbook, the bigger the XP and attribute bonus. It fills by owning an item once or by beating an opponent who wears it. When the 10 Arena XP wins of the day are done, characters hunt opponents wearing items they still lack – but ONLY when no quest/event needs the Arena/Hall of Fame (tasks and events always go first, they may also be combined). Plus a chart of the fill level in the dashboard.

## Data (sf-api)
- `PlayerPollScrapbook` (`Command::ViewScrapbook`, read-only, allowed in `safe.rs`) → `scrapbook.r` (URL-safe base64 bitmask) + `legendaries.r`. `ScrapBook { items: HashSet<EquipmentIdent>, monster }`; bit index 1..=800 = monsters, 801..4224 = items (`EquipmentIdent { class, typ, model_id, color }`).
- An opponent's worn items → `item.equipment_ident()` (`OtherPlayer.equipment`, from `ViewPlayer`).
- sf-api keeps its position table private; `src/scrapbook.rs` repeats it to know the total. sf-api's own table maps 36 positions (talisman models 50–74 twice, some weapons) to idents that already exist, so the real total is **3358 distinct items** (539 accessories, 749 weapons/shields, the rest armor) + 800 monsters.

## Our percentages vs. the game's
`scrapbook::stats` = owned / 3358 for items (plus weapons/armor/accessories parts, monsters / 800). The user's main shows 27.37 % in the game (weapons 31, armor 33, accessories 38, bestiary 12), but the captured bitmask he sent gives us 598 owned items = 17.8 % (accessories 20 %, monsters 88/800 = 11 %); neither per-class nor per-model counting reproduced the game's figures. So our numbers are only comparable with each other (day to day, character to character), not with the in-game screen. ⏳ If the user compares a character's in-game numbers with the chart, calibrate the denominator.

## The hunt (`hunt::Hunt::Scrapbook`)
- Wanted only when: `wanted_hunts` is empty (no open task/event hunt), `arena.fights_for_xp ≥ 10`, the Arena is free, the scrapbook is incomplete and the last fruitless search was ≥ 30 min ago (`SCRAPBOOK_RETRY`).
- Opponent search like the other hunts (Hall of Fame from the bottom upwards, lowest honor, level ≤ ours, clearly weaker: power ≤ 0.6 × ours) but wider: 10 pages, 6 players per page, and the player must wear ≥ 1 item missing from our scrapbook (`scrapbook::missing_on`). Log: `[hunt] <name> wears N item(s) missing from our scrapbook`, after the fight `ViewScrapbook` and `[hunt] Scrapbook: before -> after items`.
- Hall of Fame fights do not count toward the 10 XP wins. Losses are fine.
- ⏳ Unverified live: whether beating a player really adds their worn items (the user says so), whether the fight response already updates the scrapbook, how many candidates the search finds, the 0.6 power ratio.

## Chart
`roster::snapshot` stores `scrapbook` (pct, owned, total, per-kind pct, monsters) in every day snapshot; `write_dashboard` puts `scrapbook_pct` into each day's row; `dashboard.html` has the metric tab "Scrapbook (collection fill) [%]". Days before 2026-10-12 have no data (the chart starts with the first report after the rebuild; "Run end of day now" writes a preview of today). The character card shows "Collection: N items (P %)".

## Correction 2026-10-12: the game's percentage (fitted)
The profile (Library of Meticulousness, = the XP bonus) shows 16 % for Sanek, 14 % Chlamydie, 12 % Filminy, 12 % Pjotr, 12 % Mimi where our first formula (items / 3358) gave 10.5 / 9.3 / 8.1 / 7.9 / 7.7. The game's number fits **(owned items + seen monsters) / D with D ≈ 2600** for all five (owned+monsters = 416 / 370 / 321 / 316 / 309; any D between 2568 and 2684 is consistent with the rounded values). `scrapbook::GAME_TOTAL = 2600` and `Stats::game_pct` now drive the chart, the card and the day snapshots (`pct`; our old items-only value is kept as `items_pct`; today's existing day files were migrated). The category percentages from the main's profile (weapons 31, armor 33, accessories 38, bestiary 12) suggest the game's category totals are ~432 / ~1060 / ~300 / ~733–800 – not understood, so the chart shows only the total. ⏳ To pin D exactly the user can read the profile with two decimals (like the 27.37 % of the main): D = (items + monsters) / percentage, e.g. from `roster/<character>/now.json` ("scrapbook" = items + monsters).

## D pinned: 2506 (2026-10-12, 2-decimal profile values from the user)
Chlamydie 14.76 % (370 = items + monsters), Květoš 12.37 % (310), Wecros 12.09 % (303) – all whole numbers with D = **2506**, and the main's 27.37 % = 686 / 2506. `scrapbook::GAME_TOTAL = 2506` (the earlier fit 2600 is replaced; today's day files migrated; test with the four values). The game's percentage therefore = (owned items + seen monsters) / 2506.

## Analysis 2026-10-12 16:30 (first hours of the feature)
- The dedicated scrapbook hunt never started (no `Scrapbook` lines in any log): it needs ALL 10 XP wins done AND no open fight task. Every character has the Ruffian event open until it ends (win 20× vs a class, 10× vs another, 30× in the HoF, 5 back to back) plus daily class tasks, so every free Arena window went to task hunts – by design (tasks first). The bot only gets to the scrapbook when those are done.
- Scrapbook counts (items + monsters) between ~13:30 and 16:30: Sanek 416 → 416, Chlamydie 370 → 370, Filminy 321 → 321, Wecros, Pagan, PajaRizz, Mrožik unchanged; Pjotr 316 → 317, Mimi 309 → 314, Novotné 313 → 318, Květoš 305 → 310 (Květoš won against Bacilek31 at 13:41, who wears ~10 items: +5 – consistent with "beating a player adds what he wears"). The task hunts fight the weakest level-1 players of a class, who wear starter items we mostly own already, so they add almost nothing.
- Change: ALL hunts now (not just the scrapbook hunt) inspect the weaker candidates of a Hall of Fame page and take the one wearing the most items missing from our scrapbook (`find_opponent`; tasks stay first, the scrapbook rides along; costs up to 2 extra `ViewPlayer` per search).

## Event/daily fight tasks ride along with the scrapbook (2026-10-12, user allowed it)
For the Ruffian event (and the daily class tasks) the user allowed combining: the task hunts themselves now pick the scrapbook-useful opponent. `hunt::find_opponent` (all hunts except the back-to-back streak, the scrapbook hunt and attack-me): inspects up to 5 players per Hall of Fame page (was 3), and among the clearly weaker ones (same power ratios as before) takes the one wearing the most items missing from our scrapbook; if the page has none it looks at up to 3 pages before it settles for the first valid candidate. The "win back to back" task keeps the plain weakest-first search (a loss would reset the streak; ratio 0.3). Tasks still decide WHEN and against which class; the dedicated `Hunt::Scrapbook` stays reserved for free time. Cost: a few more `ViewPlayer` per hunt.

## Hunt journal (2026-10-14)
`roster/<character>/logs/hunts.log` (uncapped, `hunt::journal`): one line per hunt attempt – `Scrapbook | vs <name> | win | scrapbook A -> B` or `Scrapbook | no clearly weaker opponent`. Use it to measure the success rate (searches that find nobody, wins, items added per win) instead of the 100-message progress log.
