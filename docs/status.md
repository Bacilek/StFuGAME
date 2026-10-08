# Project status (handover document)

Updated: 2026-10-08 ~05:00. Rewrite after every bigger change.

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
  item data. No manual backfill has been done for her yet – ask the user if/how to handle her baseline (e.g. just
  let her first dashboard point start already elevated, flagged as not comparable to day 0).
- **`SF_ACCOUNTS` / `SF_USER` optional 4th field / `SF_USER_ALT`:** an account can list a fallback login (e.g. the
  registration email) alongside the username; `main.rs::login` tries the primary login first and only retries with
  the fallback if that is rejected (user 2026-10-08: prefer the username, but some accounts need the email).

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
