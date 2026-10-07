# Controlling the bot

## For the user
- **Start:** double click the **"StFuGAME bot"** shortcut on the desktop. A round icon appears next to the clock and the bot starts right away.
  (If you cannot see the icon, it may be hidden under the ^ arrow in the notification area.)
- **Icon:** green = bot running, grey = stopped, red = ended with an error (details in the log).
- **Right click on the icon:** Start bot / Stop bot / Open log / Exit.
  - *Stop bot* = the bot stops playing, the icon stays (can be started again).
  - *Exit* = closes the program including the icon. Start it again with the desktop shortcut
    (or directly `target\release\stfugame.exe`).
- When you want to play manually in the browser, **stop** the bot first (otherwise you fight over the session).
- The bot can run only once; a second start reports "already running".
- Log: `logs\progress.log` (also via "Open log"), keeps only the last 100 messages.
- If mushrooms ever decrease outside the allowed exception, the bot stops itself and shows a warning.

## For Claude (development)
- The shortcut runs `target\release\stfugame.exe` (working folder = project). The exe finds the folder with `.env` by itself.
- The release build has no console (`windows_subsystem = "windows"`), debug (`cargo run`) has a console and the icon.
- Before `cargo build --release` the user must **Exit** the bot (the exe is locked otherwise); after the build the user starts it again
  with the shortcut (or `Start-Process target\release\stfugame.exe -WorkingDirectory <project>`).
  Do not start the bot for the user unless asked – the user starts it themselves.
- Do not run `cargo run` while the bot from the icon is running – the single-instance guard shows the "already running" dialog.
