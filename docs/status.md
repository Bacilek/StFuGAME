# Project status (handover document)

Updated: 2026-10-08 ~18:50. Rewrite after every bigger change.

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
  time, never hardcoded – the rest of this shop is the real-money one. **Not yet verified live.** Needs a release
  rebuild.
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

## Open questions for the user
- Epic items are never sold → the backpack fills up over time (in the TODO, no decision yet).
- Unmapped expedition missions: barkeeper, merman, riding, lovebirds (`docs/expeditions.md`).
- Attribute-purchase key for the challenge (which stats, how split) – not decided yet.
- `Chlamydie` (Druid): whose character is this (Bacilek/Novotné/Radek/other)? `roster/roster.md`.

## Next steps (not started)
- Add PajaRizz + Chlamydie to `.env`'s `SF_ACCOUNTS` (blocks `MrozikMarta@seznam.cz|mrozikChall1|Mrožik` already
  fixed; PajaRizz capitalization confirmed as `PajaRizz`).
- Start characters one at a time from the app (TestChar1 first, already proven tonight), watch each for issues
  before adding the next.
- Decide the attribute-purchase key, then implement general (non-task) attribute buying.
- Write `roster/start.txt` (day 1 of the challenge) once all characters are confirmed stable.
