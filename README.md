# StFuGAME

A bot for [Shakes & Fidget](https://sfgame.net) that plays the daily routine of one character, written in Rust on top of the
[`sf-api`](https://crates.io/crates/sf-api) crate. It runs in the background on Windows and is controlled with an icon next to the clock.

**Golden rule: the bot never spends mushrooms.** Every command goes through one whitelist with cooldown guards and a mushroom
watchdog that stops the bot immediately if mushrooms ever decrease. The only deliberate exception is renting a mount (see below).

## What it does
- **Tavern (expeditions):** picks the shortest expedition, chooses encounters to secure 40 heroism (the maximum reward) and then
  farms keys and chests, picks boss rewards (mushrooms > gold > hourglasses), waits out the travel – never skips with mushrooms.
- **Arena:** whenever off cooldown, challenges the weakest of the three opponents, up to 10 wins for XP a day.
- **Dungeons:** whenever off cooldown, one fight against the lowest-level enemy among the unlocked dungeons.
- **Inventory:** equips better items, sells worse ones, never sells epics.
- **City Guard:** when the Thirst for Adventure is used up, works a shift that ends shortly after midnight (when it resets).
- **Daily rewards:** the daily login bonus and one free Wheel of Fortune spin.
- **Stable:** right before an expedition without a mount, rents the griffin for 25 mushrooms (or a tiger when short on mushrooms).

Details of every feature, including what has been verified against the live server, are in [`docs/`](docs).

## Setup
1. Install Rust (https://rustup.rs).
2. Copy `.env.example` to `.env` and fill in your S&F account (`SF_USER`, `SF_PASS`) and the character name (`SF_CHARACTER`).
   `SF_SERVER` is only needed when you have characters with the same name on several servers. Never commit `.env`.
3. Build: `cargo build --release`.

## Running
- Start `target\release\stfugame.exe` (e.g. via a desktop shortcut). A round icon appears in the notification area:
  green = running, grey = stopped, red = ended with an error.
- Right click the icon: **Start bot / Stop bot / Open log / Exit**.
- Stop the bot before playing the same character in a browser, otherwise the two keep invalidating each other's session.
- Logs: `logs/progress.log` (last 100 messages), `logs/expeditions.jsonl` (one line per expedition), `logs/arena.jsonl` (one line per fight).

For development, `cargo run` starts a debug build with a console; `cargo test` runs the tests.

## Disclaimer
Automating the game may be against its terms of service. Use at your own risk, ideally on a secondary account.
