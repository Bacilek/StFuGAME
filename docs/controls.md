# Controlling the bot

## App window (2026-10-08)
The bot is a desktop app (`tao` window + an embedded WebView2 control via `wry`), not just a tray icon:
- **"StFuGAME bot"** shortcut opens the **app window**: a "Characters" tab with a tile per character (name, class,
  level, Hall of Fame rank, strength, a colored status dot, and an on/off switch that starts/stops just that
  character) and a "Charts" tab (the dashboard, `roster/dashboard.html`, in an iframe). Clicking a tile opens a
  card with that character's current state (gold, mushrooms, attributes, potions, achievements %, collection,
  dungeons, equipment, guild) – the same data the dashboard's tiles show, refreshed every ~10 min while it plays.
- Buttons in the header: **Start all / Stop all**, **Run end of day now** (preview: duels + report + dashboard
  right away; the real 23:20/23:50 runs replace it, nothing counts twice), **Open in browser** (the dashboard as
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
- `SF_AUTOSTART` (env var, optional) = `character;character;…`: exactly those characters autostart when the app
  launches. Unset (default): only the `SF_USER`/`SF_CHARACTER` account autostarts; every `SF_ACCOUNTS` entry is
  added (visible in the app, switched off) and starts only when the user flips its tile or clicks "Start all".
  This is how new challenge characters get added without touching the one already being tested (user 2026-10-08).
- `STFU_NO_LOGIN=1` (env var) skips auto-starting every character – handy for trying out the window itself
  without touching the server. Never use it to justify logging in with throwaway/fake credentials instead;
  when a no-network check is needed, use this flag, not real or fake login attempts.
- `.env.example` must stay in sync with real `.env` options (`SF_ACCOUNTS`, `SF_AUTOSTART`) – it was missing them
  once, which contributed to the user editing the wrong file.
- Startup/crash errors must always reach a dialog (`tray::message_box`), never just the log – the release build has
  no console, so a silently-failed `accounts()` or a panic looks like "nothing happens" to the user.

- **Bug found 2026-10-08: none of the app window's controls (switches, Start all/Stop all, …) worked** – confirmed
  by the complete absence of `[control] App window: …` log lines despite the user clicking them. The `post()`
  helper swallowed any `window.ipc.postMessage` error silently (`catch (e) {}`), so nothing showed the failure
  (the switch still *looked* like it did something for ~5 s thanks to the optimistic debounce below, then
  reverted once that window expired and the real, unchanged backend state won). Root cause not pinned down yet
  (wry's documented `window.ipc.postMessage` bridge should work regardless of the page being loaded via
  `file://`) – fixed defensively: errors now show in a red bar at the top of the window and devtools are enabled
  (`.with_devtools(true)`, right click → Inspect or F12) so a real stack trace is visible next time instead of
  silence. If it recurs, check that bar / the console before anything else.
- Opening the app no longer force-starts every character: each character's on/off switch position is remembered
  in `roster/switches.json` (local only) across restarts. Only on the very first run (no saved file yet) does
  `SF_AUTOSTART` / the `SF_USER` account's default apply; after that, the switches are the single source of
  truth, updated by every start/stop – individual tile or Start all/Stop all alike.
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
