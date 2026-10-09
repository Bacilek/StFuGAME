# Project status (handover document)

Updated: 2026-10-09. Rewrite after every bigger change.

- **Mushroom chart fixed + saved-fights text log (2026-10-09, user request):** (1) the Mushrooms chart is the cumulative
  total gained over the character's whole life (like gold/XP, it already was); `roster::write_day` now takes
  `max(notes ledger, balance growth since the previous history row)` as the day's `mushrooms_gained`, because the ledger
  missed ~10 on Sanek's day 1 (14 in the chart vs 28 balance like everyone else; cause not provable, that day's notes are
  gone; NOT the Gleeman chest 3 – it only gives gold). Sanek's 2026-10-08 row was corrected by hand to 23. (2) `roster/arena_highlights.txt`
  (local, gitignored): one line per saved fight `time | character | vs opponent | msg id | category: why; …`; written by
  `arena_highlights::update_log`; when a character beats a record the category disappears from the old line and the line
  is deleted when none is left. Side fix: an older fight is un-marked in the game only when it no longer holds ANY record
  (before, beating one category un-marked it even if it still held others). Needs a release build.
- **Potion importance = real strength gain (2026-10-09, user: 10 % × main 100 = 25 % × Luck 40, ties should depend on what we have):** `potions::importance` now = size × attribute without potions (base + equipment) × arena weight (100/80/40/10); Luck stops counting at the crit cap (Luck ≥ 20 × level) and `targets` then uses the bigger secondary attribute instead of Luck. Replaces the old rank + size scale (and the `>= 1.0` "useful" threshold is now `> 0`). Compiles; not live yet. Pet bonus ignored, Eternal Life fixed on top. Needs a release build; watch `[potions]` lines (what is bought, swapped, sold).
- **Potion trimming prefers swapping over selling a useful potion (2026-10-09, user):** `potions::room_step` step 2: the least important bag potion is drunk if it stacks; if it is main/CON/Luck and a less important removable active potion exists (and no active of its type), that active one is removed and this one drunk instead of sold; secondary-attribute/redundant ones are still sold. `importance` now ranks Luck ≥ 1 even when Eternal Life takes the third target slot. Compiles; not live yet (needs a release build).
- **Shop spinning with potions at a full stock (2026-10-09, user: Pagan (STR) skipped cheap small DEX potions in the Magic Shop):** `shops::spin_offers` only counted potions while the potion stock was < 4, so with 4 potions in the backpack the cheap gold potions were never used to spin. Now potions are always spin offers and the spin loop calls `potions::trim_stock` after each purchase (least important potion drunk/sold, stock stays at 4). Compiles; not live yet – check `[shops] Spun …` totals/costs and that the stock stays at 4. Needs a release build.
- **Release run 2026-10-09 18:44 – coupons, dungeon + scrapbook unlocks verified live:** `[coupons] Redeemed …` then `Claimed the reward …` on TestChar1, Filminy, Mrožik, Wecros, Květoš (+ more); Mimimimi11/PajaRizz `rejected:error` (already redeemed by hand). `Unlocking feature (30/1)` and `(5/1)` worked, first Desecrated Catacombs fights won (+1287 xp). Since then added (not built yet): accept pending ident 9 (beginners task list, user request) in `ACCEPTED_UNLOCK_IDENTS` + `safe.rs`; needs a release build (exit the bot first) and a log check for `Unlocking feature (9/1)`. `40/1` still unknown.
- **Coupon automation written (2026-10-09, user chose full automation; code `N3UTR4L-EU31` in local `roster/coupons.txt`):** new `src/coupons.rs` (called from `daily::run`): POST `https://coupon.playa-games.com/redeem` (form `coupon`, `paymentstring`, `lang`), then `ClaimablePreview` + `ClaimableClaim` of the Coupon mail; `paymentstring` = `<player id>_<ownplayersavecharacter[0]>_<[len-2]>_1` (two captured samples agree, `SimpleSession::char_save()` keeps the raw numbers; new direct dependency `reqwest`). `safe.rs` allows view/claim only for a Coupon-type mail from our own Mail. Tests for the string pass; NOT run live yet – needs a release build, then check the log for `[coupons] Redeemed …` / `Claimed the reward …` and that the status text for already-redeemed characters is sane. Details: `docs/coupons.md`. Note: the user's Pagan capture suggests City Guard was unlocked by the same `30/1` list; I still treat ident 30 as the dungeons/first-visit unlock (unlock list at login `30/1,9/1,5/1,40/1`).
- **Dungeon unlock implemented (2026-10-09, user captured the game client on Novotné and Mimimimi11):** at login the server lists pending unlocks (`unlockfeature:30/1/9/1/5/1/40/1`); the client sends `UnlockFeature` `30/1` (params base64 `MzAvMQ==`) when the Dungeons tab is opened, after which Desecrated Catacombs go from `-1` to `0` (open). The bot never did this, so most level-16 characters only had a finished Training Camp (only Chlamydie, Sanek, TestChar1 had been unlocked in the browser). `dungeons::run` now sends `UnlockFeature` for pending main ident 30 before choosing a fight, then refreshes with `UpdateDungeons`; whitelisted in `safe.rs` only for `main_ident: 30`. Ident 5/1 is very likely the Scrapbook (user saw `scrapbook.r` arrive when it disappeared; request not captured) – the user wants it on every character (2026-10-09), so the bot now accepts idents 30 and 5 (`ACCEPTED_UNLOCK_IDENTS`, whitelisted in `safe.rs`); check the log for `Unlocking feature (5/1)` and that the card shows a Collection value. Idents 9/1, 40/1 stay pending and unknown (the user does not know them either) – left alone; the `No open dungeon (pending unlocks: …)` log stays. Compiles; NOT seen live yet. Needs a release build + a look at the logs (expect `Unlocking a dungeon (30/1)`, then a Desecrated Catacombs fight). Details and later unlock conditions: `docs/dungeons.md`.
- **Card dungeons list = only dungeons in progress (2026-10-09, user request):** finished dungeons are no longer listed (open ones incl. 0/10 stay); none left → "No dungeon in progress". The "only Training Camp" the user saw was real data: most characters have just a finished Training Camp, the rest are still locked. dashboard.html + app.html. Needs a release build.
- **Card: empty second hand crossed out, tiles sorted by rank (2026-10-09, user request):** an empty Shield slot (nothing in the second hand, only when the loadout is known) gets a red cross (`.pdslot.nohand`, dashboard.html + app.html). Character tiles are ordered by Hall of Fame rank: Charts tab (`tiles()`) by the selected day's rank, the live Characters tab (`renderTiles()`) by the current rank; no rank = last. Needs a release build + visual check.
- **Druid fight decoded (2026-10-09, user pasted a fight vs a Druid):** bear form = stance 11, swoop = `type=13` + free regular attack. `arena_highlights` now has `swoops`, `swoop_streak` and `bear_crits` categories for our own Druid (thresholds ≥ 3, my guess, tell me if different). Unit test added. Details in `docs/arena-highlights.md`. Needs a release build.
- **Card HP/armor tables from the user (2026-10-09):** HP = CON × mod × (level+1), mod Mage/Bard 2; Barbarian/Plague Doctor/Assassin/Ranger/Demon Hunter 4; Druid/Battle Mage/Warrior 5; **Necromancer 6 (sf-api: 4, scaled ×3/2 in `roster::derived`)**, Paladin 6. Armor reduction = armor × mod / own level, capped: Barbarian 0.5/25, Ranger 1/25, Paladin 1/45, Mage 1/10, Plague Doctor 0.5/25, Necro 2/20, Demon Hunter 1/50, Bard 2/50, Druid 0.5/25, Assassin 1/25, Warrior 1/50, Battle Mage 5/50 (own table in `roster::derived`; sf-api differs for Plague Doctor, Druid). Needs a release build.
- **Card fallback for old days + short potion labels (2026-10-09, user request):** day snapshots without `derived` left the Luck/Con sub-lines blank and showed "DEFENSE" under the main stat. `legacyDerived()` in `src/dashboard.html` now rebuilds main stat, crit, HP (class multiplier × CON × (level+1), Eternal Life ×1.25, no portal/runes) and damage (weapon text `min–max dmg` × class factor × (1+main/10)) from the day's attrs/level/class; approximate, old days only. Potions show `STR 10%`-style labels under the icon (dashboard.html and app.html). Needs a release build + visual check.
- **Demon Hunter revive decoded (2026-10-09, user pasted a fight where the opponent revived 2×):** `type=14` row (see `docs/arena-highlights.md`). `arena_highlights` now counts revives by that type for both sides: `revives` (ours, ≥1) and new `opp_revives` (opponent revived ≥2× and we still won; threshold my pick, tell me if it should be different). Unit test added. Needs a release build.
- **Card shows the game's derived stats + potion icons (2026-10-09, user request):** `roster::derived()` computes damage
  (avg weapon hit × (1 + main attr/10), pre-armor, like the game's "~"), hit points (`UpgradeableFighter::hit_points`),
  crit % (LCK×5/(2×own level)), armor and damage reduction (sf-api formula); non-main STR/DEX/INT show "defense" =
  total/2 (matches the user's screenshot: 94→47, 99→49). Stored in `now.json` and in the daily snapshot (`days/<date>.json`
  → `derived`, `potion_items`), forwarded to the dashboard rows. Potion icons = `assets/items/12_<id>_1_1.png`
  (id = size 0/5/10 + STR 1, DEX 2, INT 3, CON 4, LCK 5; Eternal Life 16, inverse of sf-api's parse), without a countdown (user 2026-10-09: just the potions, no time; the name/size is the hover title; `left_sec`/`until_ts` still stored, unused). **Earlier days have no `derived`/`potion_items`** (shown as "–"/text) – only snapshots written after
  the next release build get them (the 23:50 report, or "Run end of day now"). Unverified against the game:
  unarmed damage. Card damage uses the **user's table (2026-10-09, from the game)**: weapon × factor × (1 + main/10), factor Berserker 1.25, Demon Hunter 1.25, Paladin 0.83, Mage 0.83, Necromancer 0.56, Bard 1.125, Druid 0.33, Scout/Battle Mage/Plague Doctor/Warrior 1.0; Assassin = (left + right hand avg) × 0.625 (`roster::derived`; deliberately NOT sf-api's `damage_multiplier`). Armor reduction **is** verified: capped per class (`max_armor_reduction`: Mage 10, Warrior/BattleMage/DemonHunter/Bard 50, Paladin 45, Scout/Assassin/Berserker/Druid 25, Necromancer/PlagueDoctor 20) – the user's Paladin screenshot's 45 % is that cap. Crit cap is 50 % for every class (Druid 75 % only while raging). Needs a release build + visual check.
- **Character card redesigned like the game's own screen + diacritic portraits fixed (2026-10-09, user request, reference `my_input/image.png`):**
  both cards (Characters tab `app.html`, Charts `dashboard.html`, shared `charPanel()` copied into each) now show a 4×4 grid:
  hat/chest/gloves/boots left, amulet/belt/ring/talisman right, portrait in the middle (name + [guild] overlaid, level bar
  below), weapon+shield between boots and talisman; then the 5 attributes in two columns (STR/CON/DEX/LCK/INT, total +
  base/bonus underneath), then potion chips. The Charts card feeds it the selected day's snapshot, the app card live data.
  Not done: the game's derived values (damage, hit points, defense, crit %, armor) and potion icons / countdown – not in
  our data. **Portrait bug:** `app::serve_asset` did not percent-decode the URL, so `Květoš`/`Novotné` (and any non-ASCII
  name) 404'd; now decoded (`percent_decode`, `..` rejected). All characters except TestChar1 have `portrait.png`.
  Needs a release build (bot was running → exe locked) and the user's visual check.
- **Charts tab opens on the latest day (2026-10-09, user request):** `src/dashboard.html` now defaults `upto` to the
  last date instead of Day 0 (`#day=N` in the URL still overrides). Also the Charts **tiles** (level, rank, power) now show the selected day's snapshot (`rows[date]`) and refresh on day change (tiles() runs from render()), not live `now`. UI label "Strength" renamed to "Power" everywhere (dashboard, app, leaderboard.md header); data keys unchanged. Pending: user confirms after next dashboard regen.
- **Arena fight highlights implemented (2026-10-09, new `src/arena_highlights.rs`, user request):** after a won
  Arena fight, measures a set of categories (crit/block/evade streaks, paladin block+heal, companion summon/crit,
  combo/rampage turns, mid-fight revive, winning at very low HP, a much stronger opponent, a single huge hit) and
  marks it via `PlayerCombatLogMark` (shows up in Quarter → Mail, like clicking "save" in-game) only when it
  beats that **character's own previous personal best** in at least one category – not an additive score above a
  fixed bar (first design, rejected by the user after seeing it: "nepotřebuju na každé postavě 10 záznamů").
  Thresholds are high on purpose and records are tracked **per character per category**
  (`roster/<character>/arena_highlights.json`), never compared across characters. The whole thing rests on a raw
  per-round combat-log format (`fight.r`) that sf-api does not parse at all for the server's current
  `fightversion` (silent `// TODO: Actually parse this` stub) – reverse-engineered field-by-field from 9 of the
  user's own real saved fights across 7 classes (necromancer ×2, plague doctor, berserker, bard, battle mage,
  assassin, paladin as our own side) over this session, each matched against the user's own blow-by-blow
  description of what happened. Full writeup, confirmed code table, category list and open questions:
  `docs/arena-highlights.md`. `PlayerCombatLogMark` is a `Command::Custom` sf-api doesn't know either (same
  base64-params mechanism as guild/daily), whitelisted in `safe.rs::custom_allowed` only for an id already
  present in our own `gs.mail.combat_log` – free UI action, no mushroom risk. `arena::run` now uses
  `safe::send_raw` (not `safe::send`) for the `Fight` command so both the parsed `GameState` and the raw response
  string are available. New unit tests (`arena_highlights::tests`) replay one of the captured fights verbatim as
  a parser regression test, plus a record-comparison test. `cargo check`/`clippy`/`cargo test` all pass (50 tests
  now, was 46). **Nothing verified live yet**: does `PlayerCombatLogMark` actually mark the fight (does the
  server accept it, does it actually show up in Quarter → Mail)? Is `gs.mail.combat_log` populated right after a
  `Fight` response, or does it need a separate fetch (`maybe_mark` degrades gracefully and just logs a miss if
  not – watch for "no matching combat log entry to mark yet" in the log)? Are the per-category thresholds
  reasonable, or do they mark too much/too little? Demon hunter revive and druid bear form/swoop were never
  confirmed against a real sample (no opponent of those classes found) – covered only by the generic signals
  (revive-from-≤0-life, crit streak), not verified to actually fire for them. Un-marking a beaten record (so only
  the single best fight per category actually stays saved in-game) is untested – see `docs/arena-highlights.md`'s
  open questions. **Needs a release rebuild** before any of this takes effect; user to report back once more
  class samples (or a live fight) are available to tune against.

- **Chart-day card now shows that day's own stats, not today's live ones (2026-10-09, user report):** the
  roster dashboard's charts tab already pinned *equipment* to the selected chart day's end-of-day snapshot, but
  the rest of the character card (Hall of Fame rank, Honor, Strength, Gold, Mushrooms, Lucky coins, the
  Attributes base/bonus/total table, Potions, Guild) still read live `now.json` regardless of which day was
  selected — so Day 0 showed today's current armor-bonus attributes, which makes no sense (Day 0 should be
  stat-wise bare). Fixed in two places:
  - `src/roster.rs`'s `snapshot()` (written to `days/<date>.json` every day) now also stores `attrs_total`
    (base + equipment/potion bonus per attribute, same shape as the live card's table) alongside the existing
    basis-only `attrs` (kept bare for `tournament::fighter_from_day0`, which needs the unequipped Day 0 baseline
    — not touched). `write_dashboard` now also forwards `level`/`honor`/current `gold`/`mushrooms`/`lucky_coins`
    (from each day's `history.csv` row) and `attrs_total`/`potions`/`guild` (from each day's `days/<date>.json`)
    into the dashboard JSON's per-date `rows[date]`, next to the existing `equip`.
  - `src/dashboard.html`'s `openCard()` now reads all of the above from `c.rows[dayDate]` (the day picked by the
    chart's day stepper) instead of `c.now`. Only what has no daily history at all — achievements, collection,
    XP progress, the dungeon list, "updated" timestamp — still falls back to live `now.json`, and is now labelled
    "(now)" in the card so it's not mistaken for that day's value. The main dashboard's tiles (outside the charts
    tab) are intentionally unaffected — they still show live current stats, per the original design.
  - Old `days/<date>.json` files written before this change lack `attrs_total`, so the Attributes table shows
    "No snapshot for this day yet." for dates already recorded before the fix; every day from today's report
    onward will have it. `cargo build`/`cargo test` pass (46 passed, 2 ignored; `changes_between_snapshots` and
    `fighter_from_day0`'s underlying `attrs` format untouched). **Not yet visually confirmed in a browser** —
    ask the user to check the charts tab (Day 0 particularly) after the next dashboard regen.

- **Attribute-purchase weighting + surplus-gold buying (2026-10-09, new `src/attributes.rs`):** when a task lets
  the bot choose freely which attribute to buy (`UpgradeAnyAttribute`/`SpendGoldOnUpgrades`), it no longer always
  picks the main attribute – it picks whichever of the 5 attributes is currently the best weight-per-gold (weight
  = the Arena strength formula's weights: main 100 %, CON 80 %, LCK 40 %, the other two side attributes 10 %
  each; divided by that attribute's next purchase price). Also new: `tasks::buy_surplus_attributes` spends gold
  on attributes even with **no open task**, but only once gold piles up past 5× the shop reserve (the user's
  reasoning: attribute cost is fixed regardless of character level/income, so there's no rush to buy them, but
  letting gold just pile up is wasteful too since spinning the shop burns some of it on the buy/sell spread).
  The tricky part: the price of an attribute's next gold-bought point depends only on how many times *that*
  attribute has ever been bought with gold (never its value or character level), and the server doesn't expose
  this lifetime counter – so it's tracked ourselves, persisted per character
  (`roster/<nick>/attribute_levels.json`), advanced after every purchase and self-corrected (`[check] MISMATCH`
  log line, searches ±50 levels) if the price actually paid doesn't match what was expected. Prices come from a
  lookup table (`attributes::COST_TABLE`, levels 1..=216, 1 gold = 100 silver) sourced from
  sf.kalais.net/english/attributes.html (user-provided link) – **truncated at 216** because the site's own table
  turns unreliable higher up (blank/`"?"` cells past that point); beyond it the price is extrapolated with the
  table's last step. Builds and unit-tests pass (`cargo test attributes`: price table non-decreasing,
  extrapolation sane). **Not yet verified live** – no character has bought enough attribute points yet to
  exercise the self-correction path or hit the 216-level truncation boundary; watch for `[check] MISMATCH`
  lines and `[tasks] Buying … from surplus gold` after the next release rebuild.

- **Potion swap rule fixed (2026-10-09, user correction):** removed the `REPLACE_WITHIN` (≤ 3 days left) gate from
  `src/potions.rs` (`removal_ok`, `shop_step`'s `Replace` branch) — swapping a smaller active potion for a bigger
  same-type one now triggers purely on backpack pressure (full backpack + strictly better available), not on how
  many days are left on the active one. The old gate was based on a wrong assumption; see `docs/precedents.md`
  2026-10-09 ("Potion swap: smaller active → bigger bought one") for the corrected reasoning (drinking a bigger
  potion really does discard the smaller active one's remaining days — confirmed by the user — but waiting for it
  to run low doesn't actually preserve more value since ongoing small-potion refills can delay the swap forever).
  Compiles clean (`cargo check`); not yet run live — next character pass through `shops.rs`/`potions.rs` will
  exercise it, watch `logs/progress.log` for `[potions]`/`[shops]` swap/replace lines.

- **Paperdoll: real slot positions, square portrait, day-pinned equipment (2026-10-08/09, user feedback round 2):**
  - Grid rearranged per the user's correction (weapon + shield belong at the very bottom, not flanking the
    portrait): now hat (top-center) → amulet/breastplate → belt/gloves → ring/boots → talisman (bottom-center,
    above weapon+shield) → weapon/shield (bottom row). `grid-template-areas` in both `src/app.html` and
    `src/dashboard.html` (kept identical). Not yet confirmed against the user's actual in-game screen – ask if
    still off after they look at it live.
  - `.pdportrait` is now a fixed 130×130 box (`object-fit:cover`, was `contain`) so it's always a square
    regardless of the source image's own aspect ratio.
  - **Charts dashboard's card now pins equipment to the day being viewed**, not live `now.json` (the app tab's
    own card stays live – that distinction was explicit from the user). `src/roster.rs::write_dashboard` now
    reads each character's full `days/<date>.json` history (new `read_days` helper, `daily_changes` takes its
    output instead of re-reading the dir) and stores that day's `equip` (icons included) into
    `DATA.chars[i].rows[date].equip`. `src/dashboard.html::openCard` looks up `DATA.dates[upto-1]` (the globally
    selected chart day) and renders *that* snapshot's equipment, falling back to "No snapshot for this day yet"
    when absent.
  - **Known gap, not fixable retroactively:** every character's Day 0 snapshot (`days/<first-date>.json`) was
    written before the icon feature existed, so it has no `icon` field – only the text description. The paperdoll
    for Day 0 will show empty/icon-less slots; from tonight's real 23:50 report (Day 1) onward, new snapshots
    carry icons. Confirmed as expected with the user (2026-10-08), not something to backfill (the raw item fields
    needed for the icon were never saved for those old snapshots).

- **Dashboard's own card also got the paperdoll (2026-10-08, user-reported gap):** the paperdoll/hover-tooltip
  equipment layout only existed in the app tab's card (`src/app.html::openCard`) – the **Charts tab's own card**
  (`src/dashboard.html::openCard`, a separate implementation, opened by clicking a character tile inside the
  chart dashboard iframe) was still the old plain-text equipment list. Ported the same markup/CSS there (`c.nick`
  instead of `c.name` for the portrait path, otherwise identical). Both cards now read `now.json`'s `equip` map,
  which already carries `icon` (see below) – no new data plumbing needed, just the missing UI port.
  **Open question clarified with the user:** the card always shows the character's *current* (`now.json`,
  refreshed every ~10 min) equipment, not a per-day-in-the-chart snapshot – when day-0 (baseline) is still the
  only day on the chart, that's necessarily day-0's starting gear; once the first real day's report runs
  (~23:50, `write_day`) and day 1 appears on the chart (~1 h later incl. the 23:40 duel round), the live
  `now.json` will already reflect that day's end-of-day gear by then anyway, so no extra "pin equipment to the
  selected day" feature is needed – confirmed this matches what the user wants.
- **Character card: paperdoll layout (2026-10-08, user request):** the Equipment section in `openCard`
  (`src/app.html`) is no longer a text table – it's a CSS-grid "paperdoll" (`.paperdoll`) with the 10 item
  icons arranged around a central portrait the way the game's own character screen does (hat top-center,
  weapon/amulet/belt/ring left column, shield/breastplate/gloves/boots right column, talisman bottom-center),
  and item stats (`it.d`) only show in a tooltip on hover (`.pdslot .tip`, shown via `:hover`), not as always-
  visible text. Epic/legendary items get a colored border on the slot instead of colored text. The portrait
  image itself is `roster/<nick>/portrait.png` (see `roster/README.md`) – supplied by hand by the user, static
  for the challenge; a missing file just leaves the center empty (`<img>` `onerror` removes it), the bot never
  generates or touches it.
- **Character card: item icons added (2026-10-08, user request – end-of-day visual snapshots):** the Equipment
  table in the card (`openCard` in `src/app.html`) now shows the real game sprite next to each slot, not just
  text. `src/roster.rs::item_icon` builds the filename `{type}_{model}_{color}_{class}.png` from the equipped
  `Item`'s `typ.raw_id()`/`model_id`/`color`/`class` fields (`sf-api` already parses `color` with the same
  1-indexed convention the game's web client uses; `class` is 0-indexed `Class::Warrior/Mage/Scout` so +1 for
  the filename; raw type ≥ 10, i.e. talismans and beyond, always use variant/class 1 – matches `sf-tools`'
  `Loca.pic`, reverse-engineered from that independently open-source project). Added to the `equip` map built in
  `snapshot()` (shared by both the day-diff snapshot and `card_data`), so no extra server calls needed – it's
  the character's own already-fetched `GameState`. Icon pack vendored at `roster/assets/items/` (~1930 PNGs,
  43 MB, gitignored like the rest of `roster/` – never pushed to the public repo). `src/app.rs::serve_asset` now
  sends `.png` as `image/png` (was falling back to `application/octet-stream`). Not yet tested live in the app
  (needs a restart of the running bot to pick up the new build) – user to verify the icons actually render.
  **Portrait itself (the user's ask also included this): out of scope by design** – the user supplies it by hand
  since it won't change during the challenge; the bot only handles items. See `docs/precedents.md` 2026-10-08.
- **Character card: hourglasses added (2026-10-08, user request):** the card opened by clicking a character tile
  (`openCard` in `src/app.html`) now also shows "Hourglasses" next to Gold/Mushrooms/Lucky coins. Source:
  `gs.tavern.quicksand_glasses`, added to `card_data` in `src/roster.rs` (written to `now.json`, read by the app).

- **Fixed "invalid chest" spam at a new event theme's start (2026-10-09, user-reported: "všechny postavy by
  měly dostat uplně nové druhy questů"):** every running character hit this at the 2026-10-09 midnight reset,
  repeating ever since on every `tasks::run` pass. See `docs/tasks.md` for the root cause (event reward-chest
  reset arriving before the event task list syncs) and the fix in `src/tasks.rs::claim_chests`/`log_tasks`.
  **Needs a release rebuild + bot restart** – the bot was still running the old build when this was found, not
  restarted by Claude per the "user starts the bot themselves" rule.

## Where we are
- The bot is now a **desktop app** (`app.rs`: `tao` window + `wry`/WebView2, served over a custom `app://` protocol,
  not `file://` – see `docs/controls.md`), not just a tray icon. The "StFuGAME bot" shortcut opens a window with a
  **Characters** tab (a tile per character, on/off switch, current activity + live countdown, Thirst for Adventure
  bar) and a **Charts** tab (the dashboard). The tray icon still exists for global Start all/Stop all/Exit/Open log.
- **Every character defaults to switched off**, even on a brand new install – opening the app never starts anything
  by itself. Each character's on/off position is remembered in `roster/switches.json` (local only) across restarts.
  There is no `.env`-level autostart any more (`SF_AUTOSTART` was removed).
- **Several characters run in one process**, each in its own tokio task (`ctx::CHARACTER` task-local). State kept
  between calls uses `ctx::PerChar<T>` instead of a plain static. Accounts: `SF_USER`/`SF_PASS`/`SF_CHARACTER`
  (one, optional) + `SF_ACCOUNTS=login|pass|character;…` (more, for the friends' challenge) in `.env`. Logs:
  combined `logs/progress.log` (`[character]` prefix) + `logs/<character>/` (own progress.log, arena.jsonl,
  expeditions.jsonl).
- Character used for testing: **TestChar1** on **s31.sfgame.eu**, class **Battle Mage** (not Warrior – that was a
  wrong assumption from made-up demo data, corrected 2026-10-08 against the real `now.json`). Currently in guild
  **Venom**. Not running right now (all characters are off; the user starts them from the app when wanted).
- **5 friends' accounts** are in `.env` (`SF_ACCOUNTS`) but not yet started even once: Filminy (Scout), Mimimimi11
  (Paladin), Mrožik (Mage), Wecros (Berserker), Květoš (Demon Hunter). A 6th, PajaRizz (Bard), and a 7th, Chlamydie
  (Druid, owner not noted yet), are known but not yet added to `.env` – see `roster/roster.md`.
- All finished bot features: Tavern (expeditions), Arena, Dungeons, inventory, City Guard, daily rewards + Wheel of
  Fortune, Stable, shops (incl. spinning + potions bought there), potions, Goblin Gleeman + event tasks (incl. the
  costly-task planner: shell game/lucky-coin spins/beer), Hall of Fame hunt (class/bare-hands fight tasks), guild
  (auto-join the best + daily switch check with a 3-day minimum tenure), guild battle sign-ups.
- **Character challenge** (`roster/`, entirely local, gitignored): `roster/dashboard.html` is the one page to look
  at – character tiles + cards (current state) and charts (Gold/XP/dungeons/Arena wins total/Hall of Fame rank/
  mushrooms/strength/simulated win rate, day-by-day, "Why up?" panel explaining win-rate jumps). A daily simulated round-robin runs at
  23:40, the real daily report (history.csv, issues, dashboard refresh) at 23:50; "Run end of day now" in the app
  does both immediately as a preview (does not count as the real day). `roster/issues.txt` collects every
  character's biggest success + flagged issues for the day – only written at 23:50 or via that preview button, not
  continuously (the underlying per-character `notes.log` **is** written continuously, in case a manual read is
  needed sooner).
- **`progress.log` noise cleanup (2026-10-08):** the per-character log keeps only the last 100 lines
  (`report::MAX_LINES`), so verbose loops were pushing out the day's real events. Removed/collapsed (user request):
  Arena's per-opponent strength dump (keeps only "Challenging: X"), Dungeons' per-candidate dump, Hunt's per-inspected-
  candidate dump (logs only the chosen opponent), the "Nothing to do, next check in…" heartbeat in `main.rs`'s main
  loop, and the Shop's per-spin lines (up to `MAX_SPINS = 60`/day → now one "Spun Nx, total cost …" summary per run),
  plus the Shell game and Wheel-of-Fortune lucky-coin loops in `tasks.rs` (now one summary line instead of one per
  bet/spin), and `buy_attributes` (one line per purchase instead of two). Data for the dashboard/charts is read
  straight from `GameState`, never parsed from these log lines, so trimming them is safe.
- **"Day 0" baseline for new challenge characters (2026-10-08):** `roster::write_day0` fires once, right after a
  character's very first successful login (`main.rs::run_character`, before `play()`'s first action), as long as
  it has no `history.csv` yet AND is still level ≤ 3 (`DAY0_MAX_LEVEL`) – i.e. caught right after the tutorial,
  before dungeons/Tavern/Arena can level it up. It writes a `history.csv` row and a `days/<date>.json` snapshot
  backdated to "the day before the challenge's Day 1" (`roster::day0_date`, inferred the same way as
  `tournament::start_date`), so every character's chart starts at the same point no matter when it is actually
  added. `tournament::run_day0` then (re)runs a baseline duel round for that same backdated date, covering every
  roster participant loadable via `ViewPlayer` at that moment – re-run every time a new character reaches its own
  Day 0, so latecomers get folded in. A character that already leveled up before its first bot run (too late for
  an accurate baseline) is skipped with a log line, not silently given a wrong snapshot: **Chlamydie** hit this –
  she went from level 2 to 13+ within her first run (Training Camp farming) before any report was ever written, so
  no accurate pre-bot snapshot of her exists; only `notes.log`'s classified events survive (first login ~04:08,
  first dungeon win ~04:14, "Level up: 2 → 4" at 04:18), which only gives a rough lower bound, not real gold/rank/
  item data.
- **Chlamydie's Day 0 manually backfilled (2026-10-08, user request: "zasimuluj druid starting stats"):**
  `roster/Chlamydie/history.csv` and `roster/Chlamydie/days/2026-10-07.json` were written by hand (not from real
  data – there is none) with "level 2, fresh Druid, one tutorial run" numbers: level 2, base STR/DEX/INT/CON/LCK
  5/5/7/6/5 (strength 15, no equipment yet), no mushrooms/guild/potions. Gold 2.36, honor 101, rank 12560 are the
  user's own numbers (2026-10-08), not estimates. First tried borrowing 2 non-weapon items from TestChar1's
  current equipment for her "2 starting items", but the user rejected that (TestChar1's gear is from a much
  higher level, far too strong for a Day 0 baseline) – reverted to no equipment for now. **Decision (user
  2026-10-08): wait until 1–2 other challenge characters have their own real Day 0 captured, then borrow 2 of
  their actual fresh-level-2 items for Chlamydie instead.** Resolved the same day: **Novotné** (Plague Doctor)
  was the first to log in and get a real Day 0 (`roster/Novotné/days/2026-10-07.json` – level 2, rank 13474,
  honor 101, gold 2.36, 5 mushrooms, 102 hourglasses, equip Boots DEX+6 / Chest plate CON+2 / Weapon 2–6 dmg –
  confirms the mechanism works end to end). Borrowed the 2 non-weapon items (Boots DEX+6, Chest plate CON+2) for
  Chlamydie; final strength = 17.
  This fixes her Gold/XP/rank/strength line charts (picked up automatically by the next `write_now`, within
  ~10 min while she keeps running). It does **not** fix the duel/tournament baseline: `tournament::run_day0` can
  only simulate a battle from a character's *live* `ViewPlayer` data, so as soon as any other character joins a
  Day 0 round, Chlamydie would be pulled in with her real (already level 13+) stats, not these simulated level-2
  ones – unfair against others' genuinely fresh Day 0. Not solved; flagged for the user, since faking a full
  synthetic opponent (equipment etc.) for `simulate_battle` felt too fragile/invasive to do silently.
- **`SF_ACCOUNTS` / `SF_USER` optional 4th field / `SF_USER_ALT`:** an account can list a fallback login (e.g. the
  registration email) alongside the username; `main.rs::login` tries the primary login first and only retries with
  the fallback if that is rejected (user 2026-10-08: prefer the username, but some accounts need the email).
- **Day 0 mechanism confirmed working live (2026-10-08):** Novotné (Plague Doctor) was the first real challenge
  character started after the rebuild – `roster/Novotné/history.csv` and `days/2026-10-07.json` got written
  automatically and correctly (level 2, rank 13474, honor 101, gold 2.36, equip Boots DEX+6 / Chest plate CON+2 /
  Weapon 2–6 dmg). Chlamydie's manual Day 0 got its final touch-up: borrowed Novotné's 2 non-weapon items
  (Boots DEX+6, Chest plate CON+2) instead of TestChar1's (rejected – too strong for a Day 0 baseline); final
  strength = 17. Also wrote `roster/start.txt` = 2026-10-08 by hand, *before* any other character's first login,
  to lock the challenge's "Day 1" = today – without it, `tournament::start_date()` would have inferred "Day 1"
  from Chlamydie's already-backfilled 2026-10-07 row and computed every other character's Day 0 as 2026-10-06,
  one day off from Chlamydie's and Novotné's.
- **Arena opponent picking changed from the attribute formula to a simulated win chance (2026-10-08):** found
  digging into why TestChar1 was on a long losing streak (32 losses / 41 fights) despite always challenging "the
  weakest of 3" – the old `strength()` formula is attribute-only, it ignores weapon damage, crit/block chance and
  class matchups entirely, so "weakest by the formula" isn't reliably "easiest in practice". `arena::run` now
  builds a `Fighter` for the character and for each of the 3 candidates (same sf-api types `tournament.rs` already
  uses for the daily duels: `PlayerFighterSquad`/`UpgradeableFighter`/`Fighter`/`simulate_battle`) and picks
  whichever candidate has the highest simulated win ratio (300 iterations, vs the tournament's 1000 – this runs
  every ~10 min per character, not once a day). The old formula (`arena::strength`/`arena::total`) is kept as-is
  for `hunt.rs`'s Hall of Fame search (a full battle sim per scanned candidate there would mean far too many
  `ViewPlayer` calls). **Not yet verified live** – the user said to try it and revert if it doesn't actually
  improve the win rate (`docs/arena.md`'s Verification status table tracks this). **Needs a release rebuild
  before it takes effect** – the bot was running again (all characters) when this was written, so the rebuild is
  still pending; `cargo check`/`cargo test` both pass on dev, but the release exe does not have this yet.
- **Found and fixed a thundering-herd login bug + reduced CPU/IO load (2026-10-08).** The user toggled several
  challenge characters on one after another (not via "Start all", which already staggers logins –
  `control::start_all`) and 4 of them (Filminy, Květoš, Mimimimi11, Sanek) failed with `ConnectionError`
  ("Could not connect to the server") within the same ~5 min window; the PC also got noticeably slower. Fixes:
  - **`main.rs::throttle_login`**: a process-wide minimum gap (`LOGIN_SPACING_SEC` = 4 s) between ANY two login
    attempts across all characters, including reconnects after a session loss – not just the initial "Start all"
    stagger, which didn't cover starting characters one by one from their own tile switch (delay always 0 there)
    or several reconnecting together.
  - **Arena's `SIM_ITERATIONS` 300 → 100** (see the entry above) – 3 candidates × up to ~10 characters every
    ~10 min adds up; 100 is still plenty to rank 3 candidates.
  - **`roster::write_now`**: the dashboard rebuild (scans every character's folder – O(all characters)) now only
    actually runs once per 10-min window for the whole process, not once per character on its own independent
    timer (previously could fire close to once a minute with ~10 characters running).
  Not yet verified that this actually fixes the connection errors or the slowdown – needs a release rebuild and
  another multi-character start to confirm.
- **Actual root cause of the "ConnectionError" logins, found after a rebuild (2026-10-08): swapped credentials,
  not load.** The thundering-herd theory above turned out to be wrong for these specific failures – Filminy and
  Mimimimi11 kept failing even well after the throttle fix, spaced tens of seconds apart (clearly staggered).
  `sf-api`'s SSO client (`sso.rs::send_api_request`) maps ANY non-2xx HTTP status *and* any `"success": false`
  API response (wrong password, wrong/unknown login, account locked, real network failure – all of it) to the
  same generic `SFError::ConnectionError`, so "Could not connect to the server" is not trustworthy as "it's a
  network problem" – it just as easily means wrong credentials. Root cause here: **Filminy's and Mimimimi11's
  e-mails were swapped** in `SF_ACCOUNTS` (the user's own slip when typing them in, confirmed by the user).
  Fixed by swapping them back. Lesson: when several characters get `ConnectionError` on login and the bot itself
  hasn't changed, check the credentials before suspecting the bot/network.
- **Day 0 tournament round was not actually "everyone at level 2" (found and fixed 2026-10-08):** Sanek's first
  login triggered `run_day0` at 14:03, but by then Filminy/Mrožik/Wecros/Květoš/PajaRizz/Pjotr/MimiMimi11/Novotné
  had all been running since ~13:30 and were already level 4-7 – the round used live `ViewPlayer` data for
  everyone except the hand-fixed Chlamydie override, so most "Day 0" win rates (e.g. Květoš 0.4%, PajaRizz 88.9%)
  reflected whatever level each happened to be at 14:03, not level 2. Fixed properly (not just patched for
  Chlamydie): `tournament::run_day0` now builds a synthetic `Fighter` for EVERY participant from their own stored
  `roster/<nick>/days/<day0_date>.json` snapshot (`tournament::fighter_from_day0`, parsing level/base attrs/
  equipped items' "d" description strings back into numbers – `tournament::parse_item_desc`), falling back to
  live `ViewPlayer` only when a character has no Day 0 snapshot yet. This replaces and generalizes the earlier
  Chlamydie-only hardcoded fighter (removed) – her manually simulated baseline is just a snapshot like everyone
  else's now, no special-casing needed. `run()`'s regular daily rounds are untouched, still always live data.
  Needs `roster/<nick>/days/<date>.json` to exist (it does for everyone who has reached their own Day 0) and a
  release rebuild; re-run via "Run end of day now" in the app (also now re-runs `run_day0` – `main.rs`) once
  rebuilt, to regenerate `roster/tournament/2026-10-07.json` with the corrected data. Added `enum-map = "2.7.3"`
  as a direct dependency (matching sf-api's own pinned version) to construct `Fighter`'s attribute/resistance
  `EnumMap`s by hand; `Fighter`'s `ident` field needs `Default::default()` rather than naming `FighterIdent`
  directly – that type exists but its containing module isn't re-exported, so it can't be named outside sf-api.
  Rebuilt + re-run: win rates spread out sensibly (e.g. Květoš ~33% average, not near 0% against everyone) –
  except Chlamydie then showed **0% against everyone**, the opposite extreme: her synthetic baseline had 2
  non-weapon items but no weapon at all (per the user's original ask to avoid TestChar1's over-strong items),
  and `simulate_battle` makes a weaponless fighter deal essentially no damage. Fixed by giving her a plain Druid
  starting wand, **8–10 dmg** (the user's own number, not borrowed from anyone), added to
  `roster/Chlamydie/days/2026-10-07.json`'s equip – she now has the same 3 equipped slots (weapon + 2 armor) as a
  normal fresh character (e.g. Novotné: Boots/Chest plate/Weapon). Not yet re-confirmed after this last change –
  needs one more "Run end of day now" click. Dashboard also only auto-refreshes every 5 min
  (`<meta http-equiv="refresh" content="300">` in `dashboard.html`) – a browser tab opened right after a click
  can show stale numbers for a few minutes; hard-refresh (Ctrl+F5) to confirm.
- **"Run end of day now" progress indicator (2026-10-08, user: "chtělo by to nějaký ukazatel, že data se
  načítají"):** `request_end_of_day` now also stores how many characters were running at click time
  (`control::running_count`) into `ctx::EOD_PENDING`; each character decrements it (`ctx::eod_done_one`) once it
  finishes processing that request (report + Day 0/duels). The app's status tick (`app.rs`, ~2/s) now sends this
  count to `window.onStatus(status, eodPending)` alongside the usual per-character array; `app.html` disables the
  button and shows "Updating… (N characters left)" while pending > 0, then "Done – dashboard refreshed" for 15 s.
  Note this only covers characters that were running at the moment of the click – one started afterwards won't
  be counted (acceptable: it'll pick up the next click's request like before, nothing is lost, just not reflected
  in that particular progress count).
- **Head-to-head outlier annotations in the dashboard (2026-10-08):** investigated why Day 0 showed Květoš
  (Demon Hunter) losing 98% to Novotné (Plague Doctor) despite near-identical level-2 stats, but winning 92%
  against Mrožik (Mage) – confirmed via a throwaway test (built both fighters, swapped only their `class` field,
  the result flipped from 2% to 99.85%) that this is **real class-mechanic combat, not a bug**: Plague Doctor's
  poison tincture (stacking extra damage over 3 rounds, skipped vs a Mage, ignores a Paladin's block) and Demon
  Hunter's ~44%-declining revive chance (disabled vs a Mage) are both faithfully ported from the real game by
  sf-api's simulator. Per the user's request, this is now surfaced directly on specific match-ups rather than as
  a separate class glossary: `dashboard.html`'s head-to-head table flags a cell as an outlier
  (`h2hOutlierNote`/`.outlier` CSS) when its win rate is far (≥30 points) from that character's own average
  against everyone else, or is itself extreme (<8% / >92%), and shows a tooltip naming both classes' special
  mechanic (`CLASS_QUIRKS`) as the likely factor. Template lives in `src/dashboard.html` (`include_str!`'d into
  the binary by `roster.rs`), so **this needs a release rebuild** before it shows up in `roster/dashboard.html`.
- **Dashboard "Day N" labels now 0-indexed (2026-10-08, user request):** the baseline point (the backdated Day 0
  date) now displays as "Day 0", today as "Day 1", etc. – it previously displayed as "Day 1"/"Day 2" (purely
  positional 1-based labeling in `dashboard.html`'s JS, unrelated to the backend's own `day` numbering), which
  didn't match how we'd been talking about Day 0 vs Day 1 all along. Only the displayed text changed (x-axis
  ticks, stepper, tooltips, "Why up?" panel, head-to-head title, per-day table header) – the internal `upto`
  index and URL `#day=N` deep link are still 1-based, now one off from the label (acceptable, an internal/testing
  mechanism, not user-facing).
- **Shell game now runs independently of the chest-value planner and of `tavern_done` (2026-10-08, user:
  "goldy jsou postradatelnější než houby"):** unlike Wheel of Fortune (lucky coins) and beer (mushrooms), which
  stay gated by `plan`'s `chest_worth` check, `tasks::planned` now also adds `Extra::Gamble` whenever the
  `DefeatGambler` task is open and `Means::gamble` (≥ 5 g) is true, regardless of whether any unopened chest
  actually needs it. `tasks::run` also calls `gamble()` right after `claim_chests`, no longer inside the
  `tavern_done` branch – it can run any time during the day now, not just after the Tavern/shops are done (the
  other costly tasks, guild-skill and attribute buying stay exactly as before, gated behind `tavern_done`). New
  test `gambling_open_even_when_no_chest_needs_it`. `CLAUDE.md`, `docs/tasks.md` and `docs/precedents.md` updated.
- **Found and fixed a real starvation bug in `main.rs`'s main loop (2026-10-08):** the Tavern section does
  `if after != before { continue; }` to retry immediately whenever an expedition made progress – which is
  almost every single pass while one is actively running. Everything written AFTER that point in the loop body
  (shops, tasks/shell game, City Guard, **and** the tournament due-date/manual-request/daily-report block) was
  only ever reached on a pass where the Tavern state happened not to change – rare during a continuously
  progressing expedition, which can run for many minutes at a time. Found via the new "Run end of day now"
  progress indicator: it stayed on "N characters left" for 30+ minutes because characters mid-expedition never
  got back around to checking `ctx::take_end_of_day_request()`. This could in principle also have delayed the
  **real** 23:40 duels / 23:50 report for a character still deep in an expedition at that exact moment – not
  confirmed to have actually happened yet, but the mechanism was there. Fixed by moving the whole
  due-date/manual/report block to the very top of the loop (right after `write_now`), before the Tavern section,
  so it is now checked on every single pass regardless of what the Tavern does. Shops/tasks/City Guard are left
  as they were (lower priority than being deep in an expedition – not wrong just because those also get skipped
  on a `continue`'d pass; the main loop comes back to them very soon after, once the expedition finally pauses or
  finishes), but may be worth revisiting the same way later if anything similar turns up.
- **Second "N characters left" bug, same day:** after the starvation fix above, the progress indicator still got
  stuck (went 10 → 3 and stalled). Root cause: `ctx::eod_done_one()` was called at the *end* of the manual block,
  after `tournament::run`/`run_day0`/the report `Update` – but `take_end_of_day_request()` already marks the
  request "seen" for that character the moment it returns `true`, before any of that runs. If one of those calls
  hit `SessionLost` and the function returned early, the decrement never happened – and a reconnect wouldn't
  retrigger it either, since `manual` would now read `false` for this same request (already seen). Fixed by
  calling `ctx::eod_done_one()` immediately once `manual` is confirmed true, before doing any of the actual work,
  so exactly one decrement always happens per character per request regardless of what happens afterward.
- **Chlamydie's Day 0 baseline attributes were still way too low (found and fixed 2026-10-08):** even after
  adding the borrowed items and the Druid wand, her average Day 0 win rate stayed at ~1%. Built a throwaway
  `#[ignore]`d test (`fighter_from_day0` for her vs Novotné, printed both fighters) and found why: her *base*
  attributes (my original hand-picked guess from before any real Day 0 data existed – STR/DEX/INT/CON/LCK
  5/5/7/6/5) were roughly half of what real level-2 characters actually have. Now that several real ones exist,
  the pattern is obvious: Mrožik (Mage) and Pjotr (Necromancer) – both INT-main, like Druid – have the *exact
  same* base block, CON13/DEX9/INT18/LCK12/STR13, suggesting INT-main classes get a fixed stat split at creation
  regardless of race. Replaced Chlamydie's base attrs with that block (keeping her 2 borrowed items + the Druid
  wand on top); her simulated win rate vs Novotné went from 0% to 25% – still class-mechanic-skewed (Plague
  Doctor's poison, see the head-to-head outlier notes) but no longer absurd. Updated
  `roster/Chlamydie/days/2026-10-07.json` and her `history.csv` strength (17 → 38). Data-only change, no rebuild
  needed – re-run via "Run end of day now" to regenerate `roster/tournament/2026-10-07.json`.
- **Implemented the free Mushroom Dealer "welcome pack" (2026-10-08, `daily::claim_welcome_pack`):** the user
  captured the real request/response live via browser DevTools (Network tab) on one of the challenge characters –
  `ShopCatalog`/`ShopCheckout`, `Command::Custom` + base64 params exactly like the guild list, see
  `docs/daily-rewards.md` for the full capture and item details (`starterpacks_item_2`, price 0, gold/mushrooms/
  hourglasses/lucky coins). `ShopCheckout`'s response is a JSON blob *followed by* sf-api's normal `&key:value`
  tail – neither shape alone, so `GameState::update` can't be trusted on it. Added
  `SimpleSession::send_raw_only`/`safe::send_raw_only`, which skip updating the game state for these two commands
  entirely; the caller always does a normal `Command::Update` right after, which both refreshes the state and
  re-triggers the mushroom watchdog against the state from before the checkout. Safety invariant: only ever
  checks out an `identifier` whose catalog `price.amount` it just confirmed is exactly `0`, re-checked fresh every
  time, never hardcoded – the rest of this shop is the real-money one. Added a persistent marker file
  (`roster/<nick>/welcome_pack_claimed`, not `PerChar` state) once claimed – user 2026-10-08: unlike calendar/wheel
  (which have their own server-side daily reset timestamp to check against, no extra cost either way), the
  welcome pack has no natural "done" signal, so without this it would keep polling `ShopCatalog` every 30 min
  forever, even long after being claimed. Calendar/wheel need no such fix – `daily::run` only ever reads already-
  cached `GameState` fields to decide "is it ready", never an extra request, so checking every pass is free; the
  real claim command only ever fires once the server's own timestamp says so (next day). **Not yet verified
  live.** Needs a release rebuild.
- **Task/chest log now refreshes every 15 min instead of once a day (2026-10-08, `tasks::log_tasks`):** while
  watching for the welcome-pack's chest point math live, realized the human-readable overview was stuck at
  whatever it printed once per day – the underlying decisions (claim_chests, the costly-task planner) already run
  fresh every pass, only this printout was stale. `TASK_LOG_EVERY` = 15 min, `PerChar<Option<Instant>>` instead of
  a date guard; stops entirely once every daily and event chest shows `opened`, so it doesn't keep repeating a
  static view for the rest of the day. Live finding while building this: Mrožik and MimiMimi11 both reached
  10/10 arena wins (19:50, 20:14) with no beer attempt following – suspect the welcome pack's
  `ClaimNewCustomerPack` Gleeman task isn't actually crediting (the mushroom/gold reward itself landed fine), so
  the chest-3 point math (15 fixed + 2 arena + 2 beer = 19) falls 1 short of the needed 20 without it. Not yet
  confirmed with a fresh task printout – pending this fix reaching a running character.
- **Found and fixed a real bug in the costly-task planner: `natural_points()` wrongly assumed open dungeon tasks
  would finish on their own (2026-10-08).** Confirmed live: the welcome-pack suspicion was a red herring –
  `ClaimNewCustomerPack` DOES credit correctly, just with a delay (all 11 showed 1/1 after a restart gave it time
  to sync). The real blocker: 9 of 11 characters reached Arena 10/10 + the pack (18 points), chest 3 needs exactly
  20, and beer (+2) should have closed it – but `tasks::natural_points()` counted the OPEN
  `DefeatMonstersLightDungeon` task's full 3 points as "will complete today" even though the character was stuck
  at 6-8/10 with the dungeon's 1-attempt/hour cooldown making 10/10 unrealistic before midnight. That inflated
  `expected` past the chest's 20-point requirement, so `plan()` concluded the chest was already covered by
  "natural" progress and never selected beer – a silent, permanent lock for the rest of the day, not a timing
  issue. Fixed by dropping `FightInDungeons`/`DefeatMonstersLightDungeon` from `natural_points()`'s optimistic
  set (Arena and City Guard stay – the bot retries both every cooldown with no cap on attempts, so those reliably
  land; dungeon tasks don't). New test `open_dungeon_task_does_not_block_a_chest_reachable_without_it`.
  **Verified live after the release rebuild (2026-10-08 ~21:50):** 9/11 characters (Filminy, Mimimimi11, Mrožik,
  Květoš, PajaRizz, Pjotr, Pagan, Novotné, Chlamydie) drank the beer and claimed daily chest 3 within seconds, no
  errors, no unexpected mushroom loss (mushroom watchdog never tripped). Sanek and Wecros drank the beer
  (`[tasks] Drinking a beer` logged) but chest 3 claim hadn't shown up yet as of the last check that day –
  most likely the same server-sync delay seen earlier with `ClaimNewCustomerPack` (it should self-resolve on a
  later pass; re-check their logs next session and confirm `Claiming the daily chest 3` eventually appears).
- **`SF_ACCOUNTS` normalized for consistency (2026-10-08):** every entry now follows `login|password|character`
  (optionally `|alt_login`), username first, e-mail as the 4th-field fallback, wherever `roster.md` records both
  as genuinely distinct identifiers (Filminy, Mimimimi11, Květoš, Chlamydie joined Sanek/Pagan/Novotné in this
  pattern). Left as a single e-mail-only field for Mrožik/Wecros/PajaRizz, where the "username" in `roster.md` is
  just the e-mail's local part, not a separately known login – adding it as a 4th field would just duplicate the
  same credential, no real fallback coverage gained.

## Bugs found and fixed tonight (2026-10-08), all live on TestChar1
- **City Guard was starving the Tavern of fresh Thirst for Adventure.** After a shift ended, "Tavern done" was set
  true merely because the Tavern had been *skipped* (character busy with City Guard), not only when it was
  genuinely *attempted* and nothing was affordable – so a new shift started immediately even with 100 min of
  freshly-reset ALU sitting unused. Fixed in `main.rs`'s main loop.
- **Guild could have switched every day forever**, never letting the character reach the 24 h needed for guild
  battles. Added `MIN_TENURE` = 3 days before another switch is even considered, on top of the existing "Instructor
  ≥ current + 10" margin and the once-a-day check (`src/guild.rs`).
- **The app window's controls did nothing at all** (switches, Start all/Stop all) – root cause: the window loaded
  `app.html` over `file://`, and Chromium/WebView2 treats every `file://` page as its own unique, untrusted origin,
  silently breaking `window.ipc` injection and the dashboard iframe (no exception, nothing in the log). Fixed by
  serving over a custom `app://localhost/…` protocol (`wry`'s `with_custom_protocol`) instead. Devtools are now
  enabled (right click → Inspect / F12) and IPC errors show as a red bar, as a safety net.
- Task chest logs showed raw `Silver 5200` instead of gold; now goes through the existing `report::reward()` helper
  like everywhere else.
- Decided **not** to implement the ad-based shop reroll (`docs/shops.md`): `AdvertisementsCompleted`'s response
  includes a `trust_counter` field, almost certainly anti-fraud/bot-detection telemetry from the ad network – not
  worth the risk for a minor convenience.

## Verified live tonight (see each doc's "Verification status" table for detail)
Potions (drink from backpack, stock purchases) · shops (gold-only, mushrooms unchanged, shop slot refresh, spin
cost) · guild (decision logged, a real switch happened) · City Guard pay · shell game (does win, not just lose) ·
Goblin Gleeman chest claims + attribute-task counting · session-loss auto-relogin (unprompted, worked cleanly).

## Still pending verification (time-gated, nothing to do but wait and check the log)
| What | When | Record in |
|---|---|---|
| Guild battle sign-up actually succeeding (24 h membership + now 3-day tenure gate) | a few days into a guild membership | `docs/guild.md` |
| Daily report + simulated duels actually firing at 23:40/23:50 on a day the bot runs that long | any evening the bot is left running | `roster/README.md` |
| Potions: `RemovePotion`, replacing a smaller active potion (needs a full backpack) | whenever it happens | `docs/potions.md` |
| Hourglasses bought while spinning: backpack item or straight to the counter? | next spin that offers one | `docs/shops.md` |
| Lucky-coin wheel spin actually counting for the Gleeman task | next time it's needed | `docs/tasks.md` |
| Hall of Fame hunt (class/bare-hands fight tasks) end to end | next time such a task is open late in the day | `docs/tasks.md` |
| Free deal at the Mushroom Dealer (`ShopCheckout`/`ShopCatalog`) | next brand-new character | `docs/daily-rewards.md` |
| Sanitary +20/−5, Revealing Lady bonus hypothesis, 2nd expedition boss reward | whenever they come up | `docs/expeditions.md` |
| City Guard 23:00 checkpoint actually caps a shift there + bridging shift starts on a no-beer day | next full day the bot runs, after a release rebuild | `docs/city-guard.md` |
| Staleness fix: beer drunk near the checkpoint lets the Tavern run before a new shift starts | next time beer lands close to 23:00 | `docs/city-guard.md` |
| Hourglass skip (`ExpeditionSkipWait{Glass}`) fires only in a genuine midnight-crunch case | next time bonus ALU is stranded late in the day | `docs/city-guard.md` |
| Attribute cost table/self-correction (`src/attributes.rs`) against real purchases; surplus-gold buying actually triggering | next character that buys several attribute points, or piles up 5×+ the shop reserve | `docs/tasks.md` |

## Open questions for the user
- Epic items are never sold → the backpack fills up over time (in the TODO, no decision yet).
- Unmapped expedition missions: barkeeper, merman, riding, lovebirds (`docs/expeditions.md`).
- Attribute-purchase key for the challenge (which stats, how split) – not decided yet.
- `Chlamydie` (Druid): whose character is this (Bacilek/Novotné/Radek/other)? `roster/roster.md`.

- **City Guard checkpoint + hourglass safety valve for beer's bonus ALU (decided and implemented 2026-10-08):**
  resolves the previously-open question about beer drunk during a City Guard shift wasting its bonus Thirst for
  Adventure past midnight. User's refined proposal: split guard coverage at a 23:00 checkpoint instead of
  shortening it outright (`guard::guard_hours` – a shift starting before 23:00 now caps there; one starting
  at/after 23:00 still rides to 00:00–00:59 as before, which automatically produces a "bridging" shift to the new
  day with zero extra code on a day beer was never needed – no guaranteed pay loss). Also found and fixed, as a
  prerequisite: a same-pass staleness hazard where `guard::run` trusted the `tavern_done` boolean computed at the
  *top* of a main-loop pass, so a beer drunk later in that same pass (via `tasks::run`) could still get buried
  under a freshly-started guard shift; fixed by re-checking `gs.tavern.thirst_for_adventure_sec` fresh right
  before `StartWork`. For the residual case where bonus ALU still ends up stranded with too little real time
  before midnight (e.g. beer drunk during the bridging shift itself), the user explicitly approved a narrow,
  scoped exception to "never use hourglasses": `tavern::should_skip_wait_with_glass` sends
  `Command::ExpeditionSkipWait { typ: TimeSkip::Glass }` (newly whitelisted in `safe.rs`, `TimeSkip::Mushroom`
  stays forbidden) instead of sleeping through an expedition's `Waiting` stage, but only within a conservative
  15-min safety margin before midnight. What exactly happens to an in-progress expedition right at the midnight
  reset is unknown/unverified – the margin is deliberately conservative to stay clear of finding out live. See
  `docs/city-guard.md` for the full writeup and `docs/precedents.md` for the dated decision. **Needs a release
  rebuild and a full live day to verify** – nothing about this has been observed running yet.

- **Daily report + duel round now catch up a day the bot was switched off for entirely (2026-10-09, user
  request):** answered the user's "what if I turn the bot on only at 5am instead of 23:40/23:50" question –
  confirmed `roster::due()`/`tournament::due_today()` only ever checked "today", so a fully-missed window meant
  that day's `history.csv` row, `days/<date>.json` snapshot and `tournament/<date>.json` duel round were lost
  for good (a permanent gap in the dashboard chart), not just delayed. User's reasoning for why backfilling is
  still accurate: nothing happens to a character while the bot isn't running, so the state at whatever moment
  it comes back next IS that missed day's real end-of-day state (gains show as 0, which is also correct).
  Replaced `roster::due()`/`tournament::due_today()` with `overdue_days()` in both modules (scans for the most
  recent `.final` marker, returns every date from the day after it up to yesterday – always overdue regardless
  of time of day – plus today once its own report time has passed). `write_day`/`run` now take an explicit
  `date` parameter (was always `Local::now().date_naive()`) so a past date can be backdated the same way
  `write_day0`/`run_day0` already backdate Day 0. `main.rs`'s main loop now loops over `overdue_days()` instead
  of a single today-or-nothing check, processing multiple missed days in one pass, oldest first, if needed.
  Only covers "the whole app was off" (all characters frozen together, so comparing one's current state across
  them stays apples-to-apples) – an individual character toggled off while others kept running is a different
  case, not handled here. New unit test `overdue_days_backfills_past_but_not_future` (`src/roster.rs`); no
  automated test for `tournament::overdue_days` (would have to touch the real, shared `roster/tournament/`
  directory to verify, too risky to script against live data – left for live verification instead, see
  `roster/README.md`'s Verification status table). **Needs a release rebuild and a live gap to confirm.**
- **Guild battle sign-up retry interval 1 h → 4 h (2026-10-09, user request):** checked in on a status question –
  all 11 challenge characters had their daily chest 3 (max, 20 pts) + all 3 event chests open, no mushroom-watchdog
  trips, no connection errors. Only noise: `[guild] Sign-up failed: not 24 hours member` repeating for Novotné
  faster than the intended hourly throttle – traced to a reconnect (`Logging in...` again at 23:07:56, ~10 min
  after a failed attempt) resetting the in-memory `LAST_FAIL` state (`src/guild.rs`), not a logic bug in the
  throttle itself. Since the server-side reason doesn't change until the character hits 24 h membership anyway,
  bumped `RETRY_SEC` 3600 → 14400 (4 h) to cut down the noise; left as in-memory (not persisted across
  reconnects/restarts) since those are rare. See `docs/guild.md`.
- **Fixed Mimimimi11 missing from the dashboard's winrate chart/movers feed and wrong (grey "Other") matrix
  color (2026-10-09, user report):** `roster/roster.md`'s table had the nick spelled `MimiMimi11` (capital M
  mid-word), while the actual roster directory/`.env` character name is `Mimimimi11` (capital M only at the
  start) – confirmed by `roster/Mimimimi11/` on disk and consistent usage everywhere else (`docs/status.md`
  prose, dashboard JSON). `tournament::participants()` reads nicks straight from `roster.md`, so the daily
  win-rate map was keyed `MimiMimi11` while `roster::write_dashboard` looks characters up by their directory
  name `Mimimimi11` – an exact, case-sensitive string match that never hit. Effect: `winrate` stayed `null`
  for every day → filtered out of the chart (`v!=null` checks) and the movers/"changes" side panel (`!isNaN`
  checks on a null-derived NaN), and in the head-to-head matrix `classOf()` (keyed off the same nick) found no
  matching character, so it fell back to the generic `--Other` grey instead of Paladin gold. Fixed by
  correcting the nick in `roster.md` to `Mimimimi11`; also fixed the same stale casing in `src/roster.rs`'s
  `demo_dashboard` test data for consistency. Not touched: already-written `roster/tournament/2026-10-07.json`
  and `2026-10-08.json` still have the old casing baked in, so those two historical days will keep showing a
  gap for this character; every day from today (`2026-10-09`) onward regenerates with the corrected nick and
  should line up normally. **Needs the next daily report/dashboard regen to verify live.**

## Next steps (not started)
- Add PajaRizz + Chlamydie to `.env`'s `SF_ACCOUNTS` (blocks `MrozikMarta@seznam.cz|mrozikChall1|Mrožik` already
  fixed; PajaRizz capitalization confirmed as `PajaRizz`).
- Start characters one at a time from the app (TestChar1 first, already proven tonight), watch each for issues
  before adding the next.
- Write `roster/start.txt` (day 1 of the challenge) once all characters are confirmed stable.
