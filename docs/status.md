# Project status (handover document)

Updated: 2026-10-07 ~19:30. Rewrite after every bigger change.

## Where we are
- The bot runs on the user's machine from the icon next to the clock (release exe, independent of Claude Code), character **TestChar1** on **s31.sfgame.eu**
  (secondary test account, login via the S&F account / SSO).
- Finished features: Tavern (expeditions), Arena, Dungeons, inventory, City Guard, daily rewards + Wheel of Fortune, Stable, icon controls.
  Plan: `docs/todo.md`. The user's decisions: `docs/precedents.md`. Internals and pitfalls: `docs/architecture.md`.
- Character state on the evening of 2026-10-07: level ~10, griffin until 21.10. 13:30, City Guard until 23:06 (then 1 h until 00:06), Arena 3/10 wins,
  Thirst for Adventure used up for today, the full inventory was solved by selling.
- 2026-10-07 the repo was translated to English (code, logs, docs; `docs/todo.md` stays Czech). The running exe may still be the
  Czech version (it writes `logs/prubeh.log` and `logs/expedice.jsonl`) until the user exits it and the new build is started.
  After that, merge any new lines from those old files into `logs/progress.log` / `logs/expeditions.jsonl` (translated) and delete them.

## Pending verification (check the log `logs/progress.log` and record in the matching doc)
| What | When | Record in |
|---|---|---|
| City Guard pay via `FinishWork` | 2026-10-07 23:06 | `docs/city-guard.md` |
| Second 1 h shift (23:06 → 00:06) per the 00:00–00:59 rule | 23:06 | `docs/city-guard.md` |
| Thirst for Adventure reset at midnight, Tavern starts after the shift | 2026-10-08 00:06 | `docs/city-guard.md` |
| Daily login bonus (`CollectCalendar`) | 2026-10-08 00:00 | `docs/daily-rewards.md` |
| Free Wheel of Fortune spin on day 2 | 2026-10-08 00:00 | `docs/daily-rewards.md` |
| Arena/Dungeons during an expedition (during City Guard already verified) | 2026-10-08 | `docs/arena.md`, `docs/dungeons.md` |
| Buying the griffin in the Stable (25 mushrooms) | when the mount expires, 2026-10-22 at the earliest | `docs/stable.md` |
| Sanitary +20 / −5, bonuses of Bewitched Stew, Toxic Fountain, Build A Friend | when they come up | `docs/expeditions.md` |
| Revealing Lady: hypothesis "couple really +7, bonus +10" | when it comes up again | `docs/expeditions.md` |
| Reward after the 2nd expedition boss (never seen yet) | every expedition | `docs/expeditions.md` |
| Tuning the expedition strategy from the journal (`logs/expeditions.jsonl`) | after more runs | `docs/expeditions.md` |

## Open questions for the user
- Epic items are never sold → the backpack fills up over time and the Dungeons stop (in the TODO).
- Unmapped missions: barkeeper (Mugs → DraftBeer → Barkeeper), merman, riding (Chicken → Tiger → RidingStan), lovebirds.

## Expedition journal 2026-10-07 (summary)
| Mission | Heroism | Verdict |
|---|---|---|
| Dragon Taming | 46 | 40 reached (started by the user) |
| Unicorn Whisperer | 45 | success |
| Barkeeper (unmapped) | 23 | below 40 |
| The Sword Trial | 47 | 40 reached |
| Revealing Lady | 54 | success |
| The Sword Trial (leftover) | 39 | below 40 |
