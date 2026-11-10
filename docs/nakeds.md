# Naked players (Underworld attack targets)

User 2026-11-10: for the Underworld attacks we need the easiest possible opponents at every level that is a multiple of 5
(5, 10 … 200): players WITHOUT any equipment and with the lowest total power (Arena formula, `arena::power` on base + additions + pet bonus).

Code: `src/nakeds.rs`, called from the main loop (after Arena/Dungeons) but only for the scanner character `SCANNER = "TestChar1"`.
Output (local, gitignored): `roster/nakeds.json` (state) and `roster/nakeds.md` (readable table, rewritten after every level).

## Rules (user)
- One entry per level. Levels that HAVE an entry are not searched; we do not hunt for better ones. A weaker naked player replaces
  the holder only when he turns up by chance (`offer`).
- Every new day: the saved players are re-viewed. Still naked and level a multiple of 5 in 5..=200 → kept (a different multiple of 5 →
  moved to that slot if it is free or he is weaker); dressed / gone / off the multiples → dropped and the level searched again.
- A level where nobody naked was found is retried only after 3 days (`MISS_DAYS`).

## How the Hall of Fame is searched
- Ranking = honor, which follows the level, so the page where level L ends is found by bisection on "does the page contain a player
  of level >= L" (`Hof::last_page_with`); the pages read are cached for the other levels. The page is saved as `hints[level]` and
  the next day starts there (exponential search around the hint, usually 2-3 requests).
- The weakest players of a level are at the END of its range: pages are read from there upwards (max 8 pages / 20 candidates),
  the lowest-honor players of that exact level are viewed (15 views until a naked one is found, then 6 more for a weaker one).
- All requests via `safe::send` (2.5-7 s pause each). One pass works at most 12 minutes (`BUDGET`), then continues in the next main
  loop pass; progress is saved after each level.

## Verification status
| What | Status |
|---|---|
| Bisection finds the level boundary | ⏳ untested live (compiles, unit test only for `offer`) |
| `equipment` of `ViewPlayer` is all `None` for a naked player | ⏳ assumed, check the first found entries |
| First full scan duration (~40 levels) | ⏳ read `[nakeds]` lines + `roster/nakeds.md` after the first run |

## Next
- Use the list for the Underworld attacks (not implemented; the file is the interface).
