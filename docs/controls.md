# Controlling the bot

## App window (2026-10-08)
The bot is a desktop app (`tao` window + an embedded WebView2 control via `wry`), not just a tray icon:
- **"StFuGAME bot"** shortcut opens the **app window**: a "Characters" tab with a tile per character (name, class,
  level, Hall of Fame rank, power, a colored status dot, and an on/off switch that starts/stops just that
  character) and a "Charts" tab (the dashboard, `roster/dashboard.html`, in an iframe). Clicking a tile opens a
  card with that character's current state (gold, mushrooms, attributes, potions, achievements %, collection,
  dungeons, equipment, guild) – the same data the dashboard's tiles show, refreshed every ~10 min while it plays.
- Buttons in the header: **Start all / Stop all**, **Run end of day now** (preview: duels + report + dashboard
  right away; the real 23:40/23:50 runs replace it, nothing counts twice), **Open in browser** (the dashboard as
  a normal web page, e.g. to share the link elsewhere).
- Each tile shows what the character is doing right now (Idle / City Guard / Expedition (choosing/boss/reward/
  waiting) / Quest) with a live countdown to when that ends, when the server gives an end time (City Guard,
  the expedition's waiting stage). Ticks every second client-side between the ~2 s status refreshes.
- Each tile has a yellow bar along its bottom edge showing the character's Thirst for Adventure (ALU) as a
  fraction of the daily max (6000 s / 100 min) – shrinks through the day, refills after the midnight reset.
  A quick glance tells you a character is stuck (bar not moving) without opening its card.
- **Closing the window** (✕) only hides it; the bot keeps running. The tray icon's **"Open StFuGAME"** brings it
  back, **"Exit"** really quits.
- The tray icon still exists (same icon as before): Open StFuGAME, Start/Stop **all** characters, Open log, Open
  dashboard in browser, Run end of day now, Exit. Green = at least one character running, grey = all stopped,
  red = all ended with an error.

## For the user
- **Start:** double click the **"StFuGAME bot"** shortcut on the desktop. The app window opens and every
  configured character starts right away (a few seconds apart, not all at once).
- Turn a single character on/off with the switch on its tile; **Start all/Stop all** do it for everyone.
- When you want to play a character manually in the browser, **stop that character first** (switch off its
  tile, or Stop all) – otherwise you fight over the session. Playing your own character on a *different*
  account is always fine, it never touches the bot's session.
- The bot can run only once; a second start reports "already running".
- Logs: `logs\progress.log` is everyone combined (character name in brackets); `logs\<character>\progress.log`
  is just that one. Both keep only the last messages.
- If mushrooms ever decrease outside the allowed exception, the bot stops itself and shows a warning.

## For Claude (development)
- The shortcut runs `target\release\stfugame.exe` (working folder = project). The exe finds the folder with
  `.env` by itself.
- The release build has no console (`windows_subsystem = "windows"`), debug (`cargo run`) has a console.
- Before `cargo build --release` the user must **Exit** the bot from the tray icon (the exe is locked
  otherwise); after the build the user starts it again with the shortcut.
  Do not start the bot for the user unless asked – the user starts it themselves.
- Do not run `cargo run` while the bot from the shortcut is running – the single-instance guard shows the
  "already running" dialog.
- `STFU_NO_LOGIN=1` (env var) skips auto-starting every character – handy for trying out the window itself
  without touching the server. Never use it to justify logging in with throwaway/fake credentials instead;
  when a no-network check is needed, use this flag, not real or fake login attempts.
- `.env.example` must stay in sync with real `.env` options (`SF_ACCOUNTS`) – it was missing them once, which
  contributed to the user editing the wrong file.
- Startup/crash errors must always reach a dialog (`tray::message_box`), never just the log – the release build has
  no console, so a silently-failed `accounts()` or a panic looks like "nothing happens" to the user.

- **Root cause found and fixed 2026-10-08: the app window loaded `app.html` over `file://`, and Chromium/WebView2
  treats every `file://` page as its own unique, untrusted origin.** This silently breaks wry's `window.ipc`
  injection (confirmed in devtools: `window.ipc` was `undefined`) and blocked the dashboard iframe (console:
  `'file:' URLs are treated as unique security origins`) – with no exception on the page and nothing in the log,
  so every control looked like it "did nothing". Fixed by serving the app over a custom `app://localhost/…`
  protocol instead (`wry`'s `with_custom_protocol`, backed by a small handler that reads files from `roster/`) –
  custom-protocol pages get a single stable origin, same mechanism official wry examples use instead of `file://`.
  Devtools stay enabled (right click → Inspect or F12) and IPC errors still show as a red bar at the top, as a
  safety net for the future.
- Opening the app never force-starts anything: every character defaults to switched off, including on a brand
  new install. Each character's on/off switch position is remembered in `roster/switches.json` (local only)
  across restarts, updated by every start/stop – individual tile or Start all/Stop all alike – and is the only
  thing that decides what autostarts. There is no `.env`-level autostart setting (`SF_AUTOSTART` was removed
  2026-10-08: the switches fully replace it and are easier to use).
- Start all / Stop all move every tile's switch immediately (optimistic, like the individual switch) instead of
  waiting for the next ~2 s status tick – fixes the status text flipping to "stopped" well before the switch
  visually caught up (user 2026-10-08).
- Per-character tiles keep persistent DOM elements and debounce the start/stop switch for 5 s after a click
  (user 2026-10-08: without this, the periodic status refresh – every ~2 s – recreated every tile from scratch
  and the switch visually snapped back before the backend's state change had propagated, looking like "the
  slider does nothing"). Every command from the app window (switch, Start all/Stop all, Run end of day, Open in
  browser) is now logged (`[control] App window: <cmd> <name>`) – check `logs/progress.log` if a control still
  seems unresponsive, to tell a JS-side problem from a Rust-side one.
- **Never screenshot the user's whole screen** to check the app window (a mis-timed `GetWindowRect` can return
  a zeroed rect and `CopyFromScreen` silently falls back to the full screen, which may show the user's own
  browser/game session). Confirm the window via the process list, title, and log output instead, or ask the
  user to look. If a screenshot is genuinely needed, resolve the exact client rect first and sanity-check its
  width/height are plausible before capturing.

## "Run end of day now" progress (2026-10-12, user)
Symptom: the app said all 12 characters were done, yet the charts did not show everyone. Causes: (1) `ctx::eod_done_one()` ran at the START of a character's processing, before duels + the preview report (about a minute, 3 min for a character that was mid-expedition) – the counter hit 0 too early; (2) the charts iframe only reloads itself every 5 min (`<meta refresh 300>` in `dashboard.html`). Now: `EOD_PENDING` is a list of NAMES, a character is removed only when its report is really written (`ctx::EodGuard` dropped after the report block in `main.rs::play`, or on any early return); the app shows `Updating… waiting for X, Y` (only characters that are still running, so a stopped one cannot block it) and reloads the charts iframe the moment the list empties ("Done – all characters updated, charts reloaded"). A character that is stuck therefore stays visible by name. ⏳ Needs a release rebuild + live check. Note: `tavern::tests::expedition_shortest_then_easiest` already failed before this change (unrelated, not investigated).

## One-off: mass friend adding (2026-10-12, user)
`cargo run --release --bin befriend -- [--go] [--skip Name] [--also Name]` (src/bin/befriend.rs, separate from the bot, reads `.env` like the bot). Logs in every account (4 s apart), collects the player ids, then each character sends `PlayerFriendSet` for every other one and for the `--also` names (e.g. `--also Bacilek31`); skips existing friends; checks the friend list in the response. Without `--go` it only prints the plan. The bot must be stopped first (a second login kicks its sessions). Only `Update`, `ViewPlayer` and `SetPlayerRelation{Friend}` are used, none costs mushrooms. Run 2026-10-12 (12 characters + Bacilek31): 143 added, 1 already friends, 0 failed; every add verified in the friend list. Done, no need to repeat.

## "Quests" tab (2026-10-12, user request)
New third tab in the app window (`src/app.html`): character tiles with only the portrait and the name; a click shows that character's quest state: **Daily quests (Goblin Gleeman)** and **Event quests**, each with a points bar like the game client (chest thresholds marked 🔒 / 🎁 ready / ✔ opened, the rewards under each chest), "N points earned · next chest in K points" (or "ready to open" / "all chests opened"), the event's end countdown, and ONLY the unfinished tasks with progress bars (`current/target · +points`). "Drink beer" tasks are marked as skipped by the bot (they cost mushrooms). Data: `roster::write_quests` writes `roster/<character>/quests.json` on every main-loop pass whenever the quests change (and at least every 2 min; local only), `control::status_json` adds it to each character's status as `quests`, so the tab refreshes with the normal status tick (no 10-minute `now.json` lag). The page logic was tested with a small DOM stub in node (rendering of the sample data). ⏳ Not seen in the real window yet (needs a release rebuild; no GUI check was done).

## Quests tab filter + inventory in the card (2026-10-12, user)
- **Quests tab:** a section (daily or event) whose three chests are opened is not shown; a character whose chests are ALL opened (the three daily ones and, when an event runs, the three event ones) has no tile any more (`questsDone`); with nobody left it says "Every character has collected all chests (daily and event) – nothing to do.". A character with no data yet stays visible.
- **Inventory in the character card** (Characters tab → click the name): section "Inventory (used/total slots)" under the character screen, a grid of backpack slots with sprites (`roster/assets/items`: equipment `{type}_{model}_{color}_{class}.png`, gems `15_<sub id>_1_1.png` with `Item.full_model_id` as the id, potions `12_<id>_1_1.png`), epic/legendary borders, a tooltip with the kind and details (equipment: damage/attributes + `gem slot: empty` or `gem: <type> +<power>`; gems: type and power; potions: type and size). A missing sprite falls back to the item text. Data: `backpack` in `now.json` (`roster::backpack_json`), refreshed with `now.json` (every ≤ 10 min). Tested with a DOM stub in node, not yet seen in the real window.

## Attack-only character and the Fortress tab (2026-10-14)
`SF_ATTACK_ONLY=Name1;Name2` in `.env` (documented in `.env.example`): those characters run `main::play_attack_only` (fortress raids + Academy XP only). The app window has a "Fortress" tab with a tile per character (data: `roster/<name>/fortress.json`). See `docs/fortress.md`.
