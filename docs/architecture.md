# Architecture and pitfalls

## Modules (src/)
| File | What it does |
|---|---|
| `main.rs` | Startup: finds the folder with `.env`, single-instance guard, tokio runtime, icon. `run_bot` = login + the `play` loop, re-login after a lost session. `play`: rewards → inventory → Guild → Arena → Hunt → Dungeons → Tavern (1 expedition) → Shops → Tasks (Gleeman) → City Guard → wait. |
| `tray.rs` | Icon next to the clock (tray-icon + Win32 message loop), menu Start/Stop/Open log/Exit, single-instance mutex, MessageBox. The only `unsafe` in the project. |
| `safe.rs` | **The only path to the server.** Command whitelist, cooldown guards (Arena, Dungeons, wheel), mushroom watchdog (decrease → dialog + `exit(2)`), random pauses 2.5–7 s. |
| `tavern.rs` | Expeditions: choosing the expedition, encounters, rewards, waiting, data checks (`checks`), journal. During an expedition it also calls inventory/Arena/Dungeons/Stable. Returns `Outcome` after every finished expedition. |
| `missions.rs` | Table of missions/cycles (chains, heroism, bonuses, limits). Data from the FAQ + from the user + verified on the server. |
| `journal.rs` | Expedition journal → `logs/expeditions.jsonl` (picks, heroism, keys, chests, verdict, `checks`, `unmapped`). |
| `arena.rs` | Arena: 3 opponents via `ViewPlayer`, strength by the formula, fight, log `logs/arena.jsonl`. Limit `fights_for_xp` < 10. `strength()` is shared with Dungeons and inventory. |
| `dungeons.rs` | Dungeons: `UpdateDungeons`, enemies via `Dungeons::current_enemy` (sf-api data), choice, fight. |
| `inventory.rs` | Inventory: equip better / sell worse, keep epics. Weapons by damage. |
| `shops.rs` | Weapon Shop + Magic Shop once a day after the Tavern: gold-only upgrades, spinning above the daily reserve. |
| `guild.rs` | Guild: once a day the quick-join list, join the best / switch to a clearly better one (Custom commands). |
| `session.rs` | Our copy of sf-api's `SimpleSession` with `send_raw` (raw response for keys sf-api ignores). |
| `tasks.rs` | Goblin Gleeman + event tasks: chests, guild skill, attributes, shell game (shop purchases in shops.rs, dungeon pick in dungeons.rs). |
| `hunt.rs` | Hall of Fame hunt for fight tasks (class, bare hands, no chest plate): weak opponent far below our rank. |
| `guard.rs` | City Guard: `StartWork`/`FinishWork`, length so it ends 00:00–00:59. |
| `daily.rs` | Daily login bonus (`CollectCalendar`) and the free Wheel of Fortune spin. |
| `stable.rs` | Renting a mount before an expedition (the only exception to the mushroom rule). |
| `report.rs` | Macro `report!` = println + timestamped line in `logs/progress.log`. |

Every game feature returns `tavern::Outcome` (`Done` / `SessionLost`). `SessionLost` bubbles up to `run_bot`, which logs in again.

## Pitfalls of sf-api 0.4.4 and the server (found during runs, 2026-10-07)
- **Stale state:** sf-api updates parts of the state only when the server sends them. The crossroads offer is often NOT
  refreshed after a pick → send `Update` after every game command and decide only on a fresh state. Same offer twice in a row → `Update` again.
- **`ExpeditionContinue` = `ExpeditionPickReward { pos: 0 }`** (to the server "pick option 1"). Never send it blindly.
- **After the waiting (travel after the boss)** sf-api shows the old round-5 offer and the server waits for "continue". Recognised by
  `floor_stage == 4` (the field is not public, read via serde: `serde_json::to_value(exp)["floor_stage"]`) → send `ExpeditionContinue`.
- **Per-item bonuses** of an expedition are credited right after the pick in round 10 (not after the boss). A poster (bounty) works only once.
- **The Dungeons timer** is only refreshed by `UpdateDungeons` (not `Update`, not a fight). Without it a stale time would let a fight through for a mushroom.
- **The server ignores `use_mushroom`** (Arena, Dungeons): a fight on cooldown always costs a mushroom. Hence `safe::cooldown_free`:
  after our fight the next one only with a NEW time from the server, later than our fight.
- **Session** via the S&F account (SSO) is invalidated by the server after roughly 7–10 min (and when logging in to the character in a browser) → error
  `sessionid invalid` → `run_bot` logs in again (limit 3 losses in a row; a session lasting 5 min resets the counter).
- **Server values take precedence** over the FAQ and the user's data (e.g. Sword Trial 6/3/−4, FAQ 5/2/−5).
- **"Mirror image"** in the Dungeons is a level 0 warrior in sf-api → counted as our own character.
- `fights_for_xp` (Arena) = today's wins for XP, the server resets it itself.
- Enum sizes/iteration: `gs.dungeons.light.iter()` (EnumMap), do not add `enum-map` as a dependency (different version than in sf-api).
- Fights (Arena `Fight`, `FightDungeon`, expedition boss `ExpeditionContinue`) have no duration: the server resolves them
  instantly and returns the result (`gs.last_fight`) in the same response. The fight animation exists only in the game client,
  so there is nothing to skip; the only wait is the human-like delay in `safe::send` (2.5–7 s before each command).
  Verified in `logs/progress.log` (Dungeons: result 5–7 s after "Fighting" = just that delay).

## Workflow (development)
- Tests: `cargo test` (does not need the bot stopped). Build for the user: `cargo build --release`.
- A release build needs the bot **exited** (the exe is locked) → ask the user to "Exit" in the icon; after the build the user starts it
  again with the desktop shortcut (do not start it for the user unless asked).
- Do not run `cargo run` while the bot from the icon is running (single instance → "already running" dialog).
- Follow the bot via `logs/progress.log` (e.g. Monitor with `tail -F` and a filter on `MUSHROOMS DECREASED|MISMATCH|Error|…`).
- After every change: commit (in English) + push. GitHub sometimes returns 500 → retry later.
- Editing files with Python from bash heredocs: beware of `\r`, `\s` etc. in normal strings (it broke CLAUDE.md once) and of quotes
  in heredocs (bash sometimes fails with "unexpected EOF") → put the script in a file in the scratchpad and run it, or use Write/Edit.
