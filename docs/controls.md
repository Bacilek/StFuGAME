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

- **Never screenshot the user's whole screen** to check the app window (a mis-timed `GetWindowRect` can return
  a zeroed rect and `CopyFromScreen` silently falls back to the full screen, which may show the user's own
  browser/game session). Confirm the window via the process list, title, and log output instead, or ask the
  user to look. If a screenshot is genuinely needed, resolve the exact client rect first and sanity-check its
  width/height are plausible before capturing.
