# StFuGAME

A Shakes & Fidget bot that automates daily tasks (Tavern, shopping, Arena, Dungeons, …).

## Start here (new session)
1. `docs/status.md` – where we are, what is waiting for verification, open questions.
2. `docs/architecture.md` – modules, sf-api/server pitfalls, workflow (build, restarting the bot, push).
3. `docs/precedents.md` – the user's decisions (follow them), `docs/todo.md` – the plan (kept in Czech on purpose).
4. `docs/bugs.md` – the bug hall of fame: ADD an entry ONLY for a funny, character-specific bug that hit 1–3 named characters (short, with the consequence in numbers; no generic bugs; user 2026-10-18).
5. Feature docs: `docs/expeditions.md`, `arena.md`, `arena-highlights.md`, `dungeons.md`, `inventory.md`, `shops.md`, `potions.md`, `guild.md`, `tasks.md`, `city-guard.md`, `daily-rewards.md`, `stable.md`, `coupons.md`, `controls.md`, `scrapbook.md`, `trainer.md`.
6. The bot is probably running on the user's machine right now (icon next to the clock). What it did: combined `logs/progress.log`, per-character journals `roster/<character>/logs/` (progress.log, expeditions.jsonl, arena.jsonl).

## Context
- Friends' character challenge: everything about it lives in `roster/` (local only, gitignored – never commit it):
  `roster/README.md` (files, how the daily 23:50 report works), `roster/plan.md` (plan/TODO, Czech), `roster/roster.md`.
  The code is `src/roster.rs` + `src/tournament.rs`, the page template `src/dashboard.html` (→ `roster/dashboard.html`,
  the only page the user wants).
- The user does not know Rust. Claude writes the code and explains only what is necessary.
- The user knows C#, comparisons with C# are welcome.
- **Communicate with the user in Czech.** The repository itself (code, logs, docs, commit messages) is in English,
  except `docs/todo.md`, which stays Czech. Game terms as in the English game client (Tavern, City Guard, Arena, Dungeons,
  Thirst for Adventure, Wheel of Fortune, …).

## Technology
- Rust, the `sf-api` crate (crates.io), async via `tokio`, `.env` via `dotenvy`.
- Reference implementation: github.com/tjira/rsfb (do not copy, only as an example of API usage).
- Credentials from `.env` (SF_USER, SF_PASS = S&F account; SF_CHARACTER = character name; SF_SERVER optional; more characters in
  `SF_ACCOUNTS=login|password|character;…`); never hardcode them or print them to logs. Never read the `.env` file.
- Several characters run in one process, each in its own tokio task (`ctx::CHARACTER`); state kept between calls must be
  `ctx::PerChar<T>` (per character), never a plain static. Logs: combined `logs/progress.log` ([character] prefix) +
  `roster/<character>/logs/` (progress.log, arena.jsonl, expeditions.jsonl; local only, gitignored with the rest of `roster/`).
- No `.env`-level autostart setting: every character defaults to switched off, even `SF_USER`/`SF_CHARACTER`. What
  autostarts on launch is only the user's last on/off choice per character, in `roster/switches.json` (local only).
- The bot is a desktop app (`app.rs`: tao window + wry/WebView2, loading `roster/app.html`), not just a tray icon.
  `control.rs` starts/stops individual characters (abort handle per character); `tray.rs` only builds the icon+menu,
  `app.rs` owns the single Win32 message loop (via tao) that pumps both the window and the tray icon's events.
  Details and the dev-only `STFU_NO_LOGIN=1` flag: `docs/controls.md`.

## Rules
- **Before ending a turn where any task was finished** (a feature, a fix, a decision, anything non-trivial), update
  every doc that changed as a result — at minimum `docs/status.md` (where we are / pending verification / open
  questions) and `docs/todo.md` if it affects the plan, plus the relevant feature doc and `docs/precedents.md` for
  any new decision. Do this unprompted, every time, not just for "bigger" changes: the user may run `/clear` at
  any point without warning, and the next session starts from these docs alone (see "Start here" above) — they
  must already reflect the finished state, not need a reminder to be brought up to date.
- All docs are mainly for Claude: update them whenever anything new is learned (from runs, from the user, from the FAQ). Record verified facts in the "Verification status" tables, rewrite `docs/status.md` after bigger changes.
- Plan of further features: `docs/todo.md` (add new ideas, tick off finished ones). Whenever a big game TODO comes to mind, add it to the "Návrhy od Clauda" section.
- Whenever unsure about any decision (strategy, data, what the bot may do), ask the user and record the answer in `docs/precedents.md`. Follow the precedents in similar situations.
- Random delays between actions (human-like), never spam the server.
- NEVER spend mushrooms under any circumstances until the user explicitly changes this rule. **GUILD 7/7 (user 2026-10-19, one time, on top of 6/6):** Instructor and Treasure 6 → 7 costs 1000 g + 4 mushrooms per skill, same mechanism (`guild::STEPS`, marker `Instructor7`/`Treasure7` in `guild_six.txt`), all characters, Pet never. **GUILD 6/6 (user 2026-10-16, one time):** buying the own Instructor and Treasure level 5 → 6 costs 500 g + 2 mushrooms per skill; allowed once per character and skill (`guild::six_bonuses`, `guild::six_price_ok`, `roster/<character>/guild_six.txt`), the watchdog allows exactly the price; Pet never. **REVISED 2026-10-16: the Goblin Gleeman beer exceptions (2)–(4) below are REVOKED (user: "for the Gleeman we never drink beers any more", at most a quest may offer mushrooms); the only beer exception is (5).** (5) EXCEPTIONAL XP EVENT (user 2026-10-16): up to 10 beers a day (the game's limit) Friday–Sunday while the server's Exceptional XP event is active, as soon as Thirst for Adventure runs out and the character is idle (after a City Guard shift, never cancelling one); all characters, even those with few mushrooms; gold event alone never (`tasks::beer_justified`, `docs/events.md`). Old list (user, 2026-10-07): (1) renting a mount in the Stable – griffin/dragon for 25 mushrooms, or a tiger for 10 g + 1 mushroom when there are not enough mushrooms; only when the character has no mount and is heading to the Tavern (see `docs/stable.md`); (2) beer (`BuyBeer`, 1 mushroom each) only for a Goblin Gleeman/event "Drink beer" task (it may ask for 1 or 10 beers) when those beers are needed for a chest that has at least as many mushrooms as the beers cost, after everything else for the day (`tasks::plan`, see `docs/tasks.md`; during future events beer may be allowed more). (3) ONE beer, only from 22:00 (user 2026-10-10), when the highest unopened Gleeman chest (daily/event) lacks exactly one `TravelTo(location)` visit, the character has no Thirst for Adventure, one of the expeditions on offer in the Tavern passes that location, and nothing without mushrooms (gold-only tasks, Arena, City Guard) would reach the chest anyway; once per day (`tasks::travel_beer_needed`, see `docs/tasks.md`). (4) QUEST BEER (user 2026-10-11): ONE beer, only from 22:00, for a "Drink beer" task that wants a single beer, when its points GUARANTEE the highest unopened Gleeman chest (earned + free points fall short, adding the beer reaches it, no second mushroom needed afterwards); shares the one-mushroom-a-day budget with (3) – never more than 1 mushroom a day on quests for a chest that pays no mushrooms (`tasks::quest_beer_needed`). NEITHER (3) nor (4) applies unless that chest itself pays at least one mushroom (user 2026-10-11: gold or XP is not worth a mushroom long-term; `tasks::pays_mushrooms`). The mushroom watchdog allows a decrease only for these commands and exactly by their price. Send all commands through `safe::send` (src/safe.rs), which only lets whitelisted commands (`is_allowed`) through. Add a command to the whitelist only after verifying it does not spend mushrooms. Never allow:
  - `BuyBeer` except the task exception above, `TimeSkip::Mushroom` (in `FinishQuest { skip }` and `ExpeditionSkipWait`),
  - `GambleMushrooms`, `GuildLoadMushrooms`, anything with `Mushrooms` in its name (e.g. `SocketUpgradeWithMushrooms`, `GemExtractWithMushrooms`),
  - and any other command that is not certain to spend no mushrooms (rather refuse).
- Add every new feature separately and let the user test it.
- Testing happens on a secondary account.
- After every change: git commit (English message) and push to origin (GitHub). Never commit `.env`.
- The user starts the bot themselves (desktop shortcut); do not start it for them unless asked.

## Tavern
- The Tavern no longer works with classic quests ("pick one of 3 and wait"). It has expeditions with choices along the way:
  - `ExpeditionStart { pos }` (one of 2 expeditions) → repeatedly `ExpeditionStage` from `tavern.expeditions.active()`:
    - `Encounters` → `ExpeditionPickEncounter { pos }`, `Boss` → `ExpeditionContinue`, `Rewards` → `ExpeditionPickReward { pos }`,
    - `Waiting` → wait until `busy_until` and send `Update` (never skip with mushrooms), `Finished` → end.
- Missions, rules and open questions: `docs/expeditions.md` (machine-readable in `src/missions.rs`). Unknown missions/encounters do not stop the bot; they are logged to the journal (`unmapped`), then ask the user and fill them in.
- Choosing an expedition (`tavern::choose_expedition`, user 2026-10-19): (1) one passing a wanted task location; (2) an open travel task that neither offer meets -> the shorter; (3) better XP tier; (4) same tier -> better OUR success rate of reaching 40 (from the journals, unmapped/unverified mission first); (5) shorter, then `Mission::ease`.
- Strategy (src/tavern.rs): secure 40 heroism (including bonuses/penalties at the end), then only keys and chests and never drop below 40.
  Until then score = immediate gain + future value (poster, chain step) weighted by the chance of completing it before round 10.
- Items of foreign cycles (not the main mission) only for their immediate heroism, never for future steps.
- Rewards: mushrooms > gold > hourglasses >>> Fortress wood/stone (user 2026-10-14; also for a leftover expedition, no special case). NEVER use hourglasses as a *reward preference for skipping* or mushroom skips; beer only per the task exception.
  **One narrow exception (user 2026-10-08):** `ExpeditionSkipWait { typ: TimeSkip::Glass }` (hourglass, never `Mushroom`) is allowed during the `Waiting` stage, but only when normal waiting would leave less than 15 min before the midnight Thirst for Adventure reset (`tavern::should_skip_wait_with_glass`) – to avoid wasting a late-granted bonus ALU (e.g. from a Gleeman-task beer drunk near/during a City Guard shift, see the City Guard section) instead of letting it reset unused. See `docs/city-guard.md`.
- Gleeman/event task "travel to <location>": an expedition through that location wins even when longer (user 2026-10-07).
- Timestamped progress: `logs/progress.log` (output via the `report!` macro, keeps the last 100 messages). Money is always shown in gold (`report::gold`).
- Mission data may be wrong (from the user and from the FAQ): the bot verifies it during runs (`checks` in the journal, `[check] MISMATCH`); after runs update the "Verification status" table in docs/expeditions.md and fix `src/missions.rs`.
- Journal `logs/expeditions.jsonl`: evaluate after runs (below 40 / far above 40) and tune the strategy.
- The bot cannot do classic quests (`StartQuest`/`FinishQuest`); when `tavern.available_tasks()` returns `Quests`, it skips the Tavern.

## Arena
- Description and verification status: `docs/arena.md` (update with every new finding).
- Fight ONLY off cooldown (otherwise it costs a mushroom, the server ignores use_mushroom), any time, even during an expedition.
- At most 10 wins per day (no rewards after that), driven by `arena.fights_for_xp` from the server. Losses are fine.
- Challenge the weakest of 3: power = 100 % main attribute + 80 % CON + 27 % LCK + 2 % secondary attributes (since 2026-10-19, was 10 %; `arena::SIDE_WEIGHT`).

## Dungeons
- Description and verification status: `docs/dungeons.md`.
- One fight whenever off cooldown (NEVER for mushrooms), even during an expedition. Not with a full inventory.
- Choice: lowest enemy level, at a similar level weaker stats.
- The cooldown is guarded by `safe.rs`: after a fight the next one only with a new time from the server (`UpdateDungeons`).

## Inventory
- Description and verification status: `docs/inventory.md`.
- Equippable item: better (Arena formula on the item's attributes; weapons by damage) → equip, worse → sell. NEVER sell epic items.

## Shops
- Description and verification status: `docs/shops.md`.
- Weapon Shop and Magic Shop a full pass right after midnight (00:03, user 2026-10-18; no longer after the Tavern), later in the day again when gold arrives (an affordable upgrade → full pass at most every 20 min, spare gold above the reserve → spin only). The fortress uses gold all day, before spinning. ONLY for gold (`mushroom_price == 0`), never `RefreshShop`.
- Better item (even slightly) → buy and equip. Spare gold above the reserve (most expensive gold item seen today) → spin
  (buy the cheapest, sell it right away) until all items cost mushrooms. Never buy or sell epic items when spinning.
- Hourglasses for gold may be bought when spinning (kept, never used).

## Potions
- Description and verification status: `docs/potions.md`.
- Targets: main attribute + CON + Eternal Life (only when for gold; otherwise Luck). Only for gold, after equipment upgrades,
  before tasks and spinning. Stock of up to 4 potions of any type in the backpack (gold potions also spin the shop).
  Full backpack: remove a less important active one for a better target from the backpack, else drink (if it stacks) or
  sell the least important. Swap a smaller active one for a bigger bought one only with a full backpack — immediately
  once full, not gated by days left on the active one (a bigger potion replaces the smaller active one's remaining
  time rather than adding to it, but waiting doesn't preserve more value since ongoing small-potion refills would just
  delay the swap forever; see `docs/potions.md`). Eternal Life is never removed or sold.

## Guild
- Description and verification status: `docs/guild.md`.
- Quick-join list every 6 h (docs/guild.md): no guild → join the best (weighted 3×Instructor + 2×Treasure); in a guild → switch as soon as another has Instructor AND Treasure ≥ ours + 10, no minimum tenure, but stay while a guild-fight task is open; skip guilds whose description demands skill levels we lack.
  when another one has Instructor ≥ ours + 10. Commands via `Command::Custom`, allowed one by one in `safe.rs`.
- Signs up for every planned guild attack/raid and defense (`GuildJoinAttack`/`GuildJoinDefense`, free).

## Goblin Gleeman (daily tasks)
- Description and verification status: `docs/tasks.md`.
- Claim task chests (daily + event); do tasks that cost only gold or nothing (attributes, guild skill – the cheaper one,
  Instructor on a tie –, shop purchases, Training Camp). Never beer, paid wheel spins, hourglasses, skips.
- Costly tasks: Wheel of Fortune for lucky coins and beer only when needed for a chest, looking at the chest
  rewards (`tasks::plan` – lucky coins only for a chest with mushrooms or as many lucky coins back, beer only
  for a chest with mushrooms ≥ beers). Shell game (minimal bets) is different (user 2026-10-08: gold is more
  dispensable than mushrooms/lucky coins) – play it to finish the task whenever it's open and there's enough
  gold (`GAMBLE_MIN_SILVER`), any time of day, not gated by a chest being "worth" it and not waiting for the
  Tavern/shops to be done first.
- Attributes and guild skill for tasks only after the Tavern and shops and never below the shop reserve (better equipment first).
- Fight tasks (win against <class>, bare hands, no chest plate): in the Arena prefer a weaker opponent of that class; otherwise a
  Hall of Fame hunt (rank + 1500…, low honor, clearly weaker) once the 10 XP wins are done or after 21:00 – Hall of Fame
  fights do not count towards the XP wins. Bare hands: weapon off, fight, weapon back on (src/hunt.rs).

## City Guard
- Description and verification status: `docs/city-guard.md`.
- After the Tavern is done: min(10 h, hours until midnight rounded up), but capped at a 23:00 checkpoint (user
  2026-10-08) so a beer-granted bonus Thirst for Adventure around that time still has a real chance to reach the
  Tavern before the midnight reset – a shift starting before 23:00 ends there instead of past midnight; one
  starting at/after 23:00 falls back to ending 00:00–00:59 as before (this is what bridges the gap to the new day
  on days when no beer is needed, no extra logic). Before starting a new shift, the bot re-checks Thirst for
  Adventure fresh (not a value cached from earlier in the same pass) so a beer drunk moments earlier in the same
  loop pass isn't immediately buried under a new shift. Arena and Dungeons keep running during the shift.

## Daily rewards
- Description and verification status: `docs/daily-rewards.md`.
- Only the daily login bonus (calendar) and one free Wheel of Fortune spin a day. Never for mushrooms; lucky coins only for a
  Gleeman/event task spin count when a chest needs it (`docs/tasks.md`).

## Stable
- Description: `docs/stable.md`. Buy a mount only right before an expedition when the character has none (not right after it expires).

## Controls
- The bot is controlled with the icon next to the clock (src/tray.rs); the "StFuGAME bot" desktop shortcut runs `target/release/stfugame.exe`. Details in `docs/controls.md`.
- The bot must be exited before a release build (locked exe). Do not run `cargo run` while the bot from the icon is running (single instance).

## Fortress
- Description, captures and open questions: `docs/fortress.md`, code `src/fortress.rs` (only Fortress, Laborers' Quarters, Woodcutter's Hut for now; enforced in `safe.rs`). Rules (user 2026-10-11): NEVER upgrade the Wall (fortifications); Gem Mine only from character level 125; focus upgrades on the Fortress and the Laborers' Quarters; never cancel a build and never speed up a build/upgrade with mushrooms or hourglasses (user 2026-10-11; only `FortressBuildFinish` with `mushrooms: 0`, a test in `safe.rs` pins it). Gold priority: better items > fortress > spinning the shops > attributes (the shop reserve does NOT apply to the fortress; it starts builds after the day's shop pass; guild upgrades without a task must leave 5× the reserve).
  Since 2026-10-14: the Academy is rushed after Fortress 6; the Hall of Knights may be upgraded during builds (not during the Fortress build); soldier training is limited so it never blocks a Barracks upgrade; one soldier is sacrificed when soldiers are full and no opponent is worth it; resources (esp. Academy XP) must not overflow; `SF_ATTACK_ONLY=Name` in `.env` = a character that only does fortress raids + Academy XP; the app has a Fortress tab; the surplus-attribute floor is 20× the shop reserve (since 2026-10-18) and secondary attributes are never bought. See `docs/fortress.md` "Changes 2026-10-14".

## Ads
- No ad automation (user 2026-10-11): never send `AdvertisementsCompleted` or imitate the ad network's view verification; ads are watched by hand. See `docs/shops.md`.

## Weekly events
- Every Friday 00:01 – Sunday 23:59 a set of events runs; the user lists them and the rules live in `docs/events.md`. New mushroom exception (user 2026-10-16): while "Exceptional XP" (2× XP) is active, up to 10 beers a day in the Tavern. A gold-only event never justifies beers.
