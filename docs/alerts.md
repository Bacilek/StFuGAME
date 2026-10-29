# Notifications (the bell in the app) – user 2026-10-26

Header button 🔔 + a number (unresolved alerts) in the app window; click = dialog with the alerts (tabs "Nové" / "Vyřešené"). NO popups any more
(the old `stuck.rs` popups are gone); the only popups left are the fatal ones (crash, mushroom watchdog – the app exits right after, the bell
could not be seen).

## For Claude: reading the user's answers
`roster/notifications.json` (local, gitignored): array of `{id, key, character, kind, title (plain Czech), detail (technical), first, last,
count, status: new|resolved, note}`. **`note` = the user's answer to "what should ideally have happened"** – when the user says "let's go through
the alerts", read this file, act on every alert with a note (fix the rule, map the encounter, ...), record decisions in `docs/precedents.md`.
The user marks an alert "Vyřešeno" himself; a resolved alert that happens again opens a fresh one.

## Code
`src/alerts.rs`: `raise(kind, key, title_cs, detail)`, `raise_limited(.., cooldown_min)` (for conditions checked every loop pass), `json`, `set_note`,
`set_status`. The same character + `key` while unresolved = ONE entry with a counter (`×5`). Texts must be plain Czech for the user, technical bits go to `detail`.
App: `app.rs` sends the list with every status tick (4th argument of `window.onStatus`), IPC commands `alert_note` (saved on blur of the textarea),
`alert_status`; the list is not re-rendered while the user types.

## Sources (kind)
- `stuck.rs`: no real action for 8 h (stuck), the same command failing 10× in a row (error), unknown expedition encounter (unknown), fortress waiting for gold (uncertain).
- `safe.rs`: a command refused by the whitelist (safety), mushroom watchdog (safety, plus the popup).
- `tavern.rs::check`: mission table vs. server MISMATCH (uncertain).
- `control.rs`: a character ended on its own (crash).
- `potions.rs::make_room`: backpack full and nothing may be sold/dismantled (inventory).
- `guild.rs`: no joinable guild for a guildless character (uncertain).
To add a source: call `alerts::raise*` with a stable `key` and a Czech title. Ideas not done yet: uncertain expedition choice (unmapped mission), Gleeman task the bot could not plan, shop/potion decisions without a rule.

## App dialog (2026-10-29)
"Vyřešit vše" (`#alClear`) resolves every open alert in one click (notes already saved stay); Enter in an alert's textarea = the "Vyřešeno" button (saves the note and resolves/reopens), Shift+Enter = newline.
