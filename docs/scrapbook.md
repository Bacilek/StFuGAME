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
