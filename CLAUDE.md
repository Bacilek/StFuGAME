# StFuGAME

A Shakes & Fidget bot that automates daily tasks (Tavern, shopping, Arena, Dungeons, …).

## Start here (new session)
1. `docs/status.md` – where we are, what is waiting for verification, open questions.
2. `docs/architecture.md` – modules, sf-api/server pitfalls, workflow (build, restarting the bot, push).
3. `docs/precedents.md` – the user's decisions (follow them), `docs/todo.md` – the plan (kept in Czech on purpose).
4. Feature docs: `docs/expeditions.md`, `arena.md`, `dungeons.md`, `inventory.md`, `shops.md`, `potions.md`, `guild.md`, `tasks.md`, `city-guard.md`, `daily-rewards.md`, `stable.md`, `controls.md`.
5. The bot is probably running on the user's machine right now (icon next to the clock). What it did: `logs/progress.log`, journals `logs/expeditions.jsonl`, `logs/arena.jsonl`.

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
  `logs/<character>/` (progress.log, arena.jsonl, expeditions.jsonl).

## Rules
- All docs are mainly for Claude: update them whenever anything new is learned (from runs, from the user, from the FAQ). Record verified facts in the "Verification status" tables, rewrite `docs/status.md` after bigger changes.
- Plan of further features: `docs/todo.md` (add new ideas, tick off finished ones). Whenever a big game TODO comes to mind, add it to the "Návrhy od Clauda" section.
- Whenever unsure about any decision (strategy, data, what the bot may do), ask the user and record the answer in `docs/precedents.md`. Follow the precedents in similar situations.
- Random delays between actions (human-like), never spam the server.
- NEVER spend mushrooms under any circumstances until the user explicitly changes this rule. The ONLY exceptions (user, 2026-10-07): (1) renting a mount in the Stable – griffin/dragon for 25 mushrooms, or a tiger for 10 g + 1 mushroom when there are not enough mushrooms; only when the character has no mount and is heading to the Tavern (see `docs/stable.md`); (2) beer (`BuyBeer`, 1 mushroom each) only for a Goblin Gleeman/event "Drink beer" task (it may ask for 1 or 10 beers) when those beers are needed for a chest that has at least as many mushrooms as the beers cost, after everything else for the day (`tasks::plan`, see `docs/tasks.md`; during future events beer may be allowed more). The mushroom watchdog allows a decrease only for these commands and exactly by their price. Send all commands through `safe::send` (src/safe.rs), which only lets whitelisted commands (`is_allowed`) through. Add a command to the whitelist only after verifying it does not spend mushrooms. Never allow:
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
- Choosing an expedition: the shortest, ties go to an unmapped one, otherwise the easiest to reach 40 (`Mission::ease`).
- Strategy (src/tavern.rs): secure 40 heroism (including bonuses/penalties at the end), then only keys and chests and never drop below 40.
  Until then score = immediate gain + future value (poster, chain step) weighted by the chance of completing it before round 10.
- Items of foreign cycles (not the main mission) only for their immediate heroism, never for future steps.
- Rewards: mushrooms > gold > hourglasses; for a leftover expedition (Thirst for Adventure ≤ 3 min) mushrooms > hourglasses > gold. NEVER use hourglasses or mushroom skips; beer only per the task exception.
- Gleeman/event task "travel to <location>": an expedition through that location wins even when longer (user 2026-10-07).
- Timestamped progress: `logs/progress.log` (output via the `report!` macro, keeps the last 100 messages). Money is always shown in gold (`report::gold`).
- Mission data may be wrong (from the user and from the FAQ): the bot verifies it during runs (`checks` in the journal, `[check] MISMATCH`); after runs update the "Verification status" table in docs/expeditions.md and fix `src/missions.rs`.
- Journal `logs/expeditions.jsonl`: evaluate after runs (below 40 / far above 40) and tune the strategy.
- The bot cannot do classic quests (`StartQuest`/`FinishQuest`); when `tavern.available_tasks()` returns `Quests`, it skips the Tavern.

## Arena
- Description and verification status: `docs/arena.md` (update with every new finding).
- Fight ONLY off cooldown (otherwise it costs a mushroom, the server ignores use_mushroom), any time, even during an expedition.
- At most 10 wins per day (no rewards after that), driven by `arena.fights_for_xp` from the server. Losses are fine.
- Challenge the weakest of 3: strength = 100 % main attribute + 80 % CON + 40 % LCK + 10 % secondary attributes.

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
- Weapon Shop and Magic Shop once a day after the Tavern is done. ONLY for gold (`mushroom_price == 0`), never `RefreshShop`.
- Better item (even slightly) → buy and equip. Spare gold above the reserve (most expensive gold item seen today) → spin
  (buy the cheapest, sell it right away) until all items cost mushrooms. Never buy or sell epic items when spinning.
- Hourglasses for gold may be bought when spinning (kept, never used).

## Potions
- Description and verification status: `docs/potions.md`.
- Targets: main attribute + CON + Eternal Life (only when for gold; otherwise Luck). Only for gold, after equipment upgrades,
  before tasks and spinning. Stock of up to 4 potions of any type in the backpack (gold potions also spin the shop).
  Full backpack: remove a less important active one for a better target from the backpack, else drink (if it stacks) or
  sell the least important. Swap a smaller active one for a bigger bought one only with a full backpack and ≤ 3 days left.
  Eternal Life is never removed or sold.

## Guild
- Description and verification status: `docs/guild.md`.
- Once a day the quick-join list: no guild → join the best (Instructor > Treasure > strength); in a guild → switch only
  when another one has Instructor ≥ ours + 10. Commands via `Command::Custom`, allowed one by one in `safe.rs`.
- Signs up for every planned guild attack/raid and defense (`GuildJoinAttack`/`GuildJoinDefense`, free).

## Goblin Gleeman (daily tasks)
- Description and verification status: `docs/tasks.md`.
- Claim task chests (daily + event); do tasks that cost only gold or nothing (attributes, guild skill – the cheaper one,
  Instructor on a tie –, shop purchases, Training Camp). Never beer, paid wheel spins, hourglasses, skips.
- Costly tasks (`tasks::plan`) only when needed for a chest, looking at the chest rewards: shell game (minimal bets), Wheel of
  Fortune for lucky coins (only for a chest with mushrooms or as many lucky coins back), beer (chest mushrooms ≥ beers).
- Attributes and guild skill for tasks only after the Tavern and shops and never below the shop reserve (better equipment first).
- Fight tasks (win against <class>, bare hands, no chest plate): in the Arena prefer a weaker opponent of that class; otherwise a
  Hall of Fame hunt (rank + 1500…, low honor, clearly weaker) once the 10 XP wins are done or after 21:00 – Hall of Fame
  fights do not count towards the XP wins. Bare hands: weapon off, fight, weapon back on (src/hunt.rs).

## City Guard
- Description and verification status: `docs/city-guard.md`.
- After the Tavern is done: min(10 h, hours until midnight rounded up) → ends 00:00–00:59. Arena and Dungeons keep running during the shift.

## Daily rewards
- Description and verification status: `docs/daily-rewards.md`.
- Only the daily login bonus (calendar) and one free Wheel of Fortune spin a day. Never for mushrooms; lucky coins only for a
  Gleeman/event task spin count when a chest needs it (`docs/tasks.md`).

## Stable
- Description: `docs/stable.md`. Buy a mount only right before an expedition when the character has none (not right after it expires).

## Controls
- The bot is controlled with the icon next to the clock (src/tray.rs); the "StFuGAME bot" desktop shortcut runs `target/release/stfugame.exe`. Details in `docs/controls.md`.
- The bot must be exited before a release build (locked exe). Do not run `cargo run` while the bot from the icon is running (single instance).
