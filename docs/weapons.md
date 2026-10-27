# Weapons tab – weapon swap log (user 2026-10-27)

Purpose: the user wants to verify that we really swap weapons when we should and that we do not give up too many stats for a bit more
damage (many characters wear a main weapon with some secondary stat purely for the damage).

## What is recorded
`src/weapons.rs`: `prepare(gs, item, slot, why)` before `Command::Equip` (None for non-weapons), `Pending::finish(gs)` after the
`Update`; appends one JSON line to `roster/weapons.jsonl` (shared by all characters, local only like the rest of `roster/`) – only
when the new weapon really is worn afterwards. Fields: `time`, `character`, `level`, `slot`, `old` (null = bare hands) / `new`
(`roster::item_card`: `d` description, `icon`, `epic`, `legendary`, `gem`, `price`), `before` / `after` (`roster::stat_line`: derived
`damage`, `hp`, `crit_pct`, … plus `attrs` = STR/DEX/INT/CON/LCK totals with equipment and potions), `why` (the inventory log reason).
Hooked in: `inventory::manage` (every Equip), `inventory::swap_kept_item`, `gems.rs` (kept item put on before socketing a gem).
NOT logged: the temporary weapon off/on of the bare-hands hunts (`hunt.rs`).

## The tab
App tab "Weapons" (`app.html`, `loadWeapons`/`renderWeapons`; the file is fetched over `app://localhost/weapons.jsonl` when the tab opens):
one collapsible row per character (newest swap first), inside each swap: old icon → new icon (hover = description and gem), the
damage change (`10 000 → 12 000 (+20 %)`, green/red) and the changed attribute totals/HP.
Damage is the character-screen damage of `roster::derived` (weapon × class factor × (1 + main attribute / 10)), so it includes
the main attribute of the new weapon and its gem.

## Status
Implemented 2026-10-27, tests green, NOT verified live (needs a real swap after the next release; the file starts empty).
