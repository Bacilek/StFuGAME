# Guild

## Rules (from the user, 2026-10-07)
- A character without a guild picks the best guild from the quick-join list in the Guild tab ("choose a guild")
  and joins it automatically.
- The list shows: name, rank, number of members, Hall of Knights points, Treasure, Instructor, raids.
- Priority: Instructor first, then Treasure, then the overall strength of the guild (members, their levels, …).
- Once a day check the list and switch only to a clearly better guild (Instructor at least 10 higher), and only
  after at least 3 days in the current guild (user 2026-10-08: otherwise the 24 h battle-eligibility window could
  keep resetting and the character would never actually take part in a guild fight).
  Leaving = `GroupRemoveMember:<own player id>` (to verify that it is the own id).

## What sf-api 0.4.4 can do
- `ViewGuild { guild_ident }` → `hall_of_fames.other_guilds`: per member level, treasure/instructor level, pet level,
  last activity, rank; honor, finished raids. Sum over members = the guild's Treasure / Instructor.
- Inbox: guild invitations (`MessageType::GuildInvite`).
- NOT supported: the quick-join list and the join command itself. Needs a capture from the browser
  (DevTools → Network → `cmd.php`): opening the Guild tab without a guild + clicking join, with the Response bodies.

## Captured commands (the user, 2026-10-07, browser DevTools)
| Request | params (base64 → text) | Meaning |
|---|---|---|
| `GroupRemoveMember` | `MjY0NDg=` → `26448` | leaving the guild (own player id); never needed by the bot |
| `GroupJoinList` | `MA==` → `0` | the quick-join list (0 = probably page/offset) |
| `GroupJoin` | `QXJ0dcWhb3ZhIEdhcmRhL2ludA==` → `Artušova Garda/int` | joining the guild by name; `/int` meaning unknown (constant?) |
- Leaving and joining again works right away (the user, 2026-10-07). After joining the character cannot sign up for
  guild fights for 24 h: the server answers `not 24 hours member` (seen 2026-10-07 22:08; the user guessed ~12 h).
- `GroupJoin` response (captured): the full own-guild data (`owngroupsave`, `owngroupname`, `owngroupmember`,
  `owngrouppotion`, `owngroupknights`, `groupskillprice`, …) + `ownplayersavecharacter`, `charactergroup`.
  sf-api parses these keys → after a successful join `gs.guild` is `Some` with the guild name (use it as the success check).
- `GroupJoinList:0` response (captured 2026-10-07): key `joinablegrouplist.r`, ~50 guilds, 14 fields each separated by `/`
  (text fields use the sf string escapes `$s` space, `$b` newline, `$c` …). Example:
  `108/504/Artušova Garda/48/3/106/106/0/<emblem hex>/10/33/17/<description>/xx`
  | # | Value | Meaning |
  |---|---|---|
  | 0 | 108 | Hall of Fame rank (= `owngrouprank` after joining) ✅ |
  | 1 | 504 | guild id (= first value of `owngroupsave`) ✅ |
  | 2 | Artušova Garda | name ✅ |
  | 3 | 48 | members (49 after we joined) ✅ |
  | 4 | 3 | Hall of Knights ✅ (user: same order as in the game) |
  | 5 | 106 | Treasure ✅ |
  | 6 | 106 | Instructor ✅ |
  | 7 | 0 | raids ✅ |
  | 8 | hex | emblem |
  | 9–11 | 10/33/17 | min / max / average member level (max 33, min 10 matches the member levels) ✅ |
  | 12 | text | description |
  | 13 | xx | language (xx, cs, pl, de, fr, en, …) |

## Implementation (src/guild.rs)
- Once a day (first pass of the main loop that day): `GroupJoinList:0` → `parse_list` (14 fields per guild).
- Candidates: not full (< 50 members); sorted by Instructor, then Treasure, then strength = members × average level.
- No guild → join the best. In a guild → only if another one has Instructor ≥ ours + 20 (ours = `gs.guild.total_instructor_skill`):
  leave (`GroupRemoveMember:<own player id>`) and join. Up to 3 candidates are tried when joining fails.
  Success = `gs.guild` has the new name after `GroupJoin`.
- Commands are `Command::Custom`; `safe.rs` (`custom_allowed`) lets through only `GroupJoinList:<number>`,
  `GroupJoin:<name>/int` and `GroupRemoveMember:<own player id>` (never kicks anyone else).
- The raw response comes from our own `src/session.rs` (copy of sf-api's `SimpleSession` with `send_raw`).
- Guild fights (user 2026-10-07: sign up automatically): `guild::battles` in every pass of the main loop.
  When `gs.guild.attacking` / `defending` holds a future battle (attacks incl. raids, `is_raid`) and our member entry
  does not show it as joined (`battles_joined`), send `GuildJoinAttack` / `GuildJoinDefense` (free, whitelisted).
  Each battle (kind + time) is signed up for only once per run (in case the command toggles); a failure
  (e.g. ~12 h after joining a guild, not yet a 24 h member) is retried after 4 h (`RETRY_SEC`, bumped from 1 h
  2026-10-09 – the underlying "not 24 hours member" error doesn't change until the 24 h mark anyway, no point
  retrying hourly). This throttle is in-memory only (`PerChar`), so a reconnect/restart still resets it and can
  retry sooner than that – observed live: Novotné relogged at 23:07:56 right after a failed attempt at 22:57:19,
  retrying after only ~11 min instead of the full interval. Not fixed further (reconnects are infrequent enough
  that this is an acceptable, rare exception, not worth persisting the throttle to disk).
- sf-api reads `battles_joined` as value % 100; the raw values look like `010` / `100` / `000` (3 digits, the hundreds digit
  is probably the raid) → a raid sign-up may not be visible in `battles_joined`, hence the local memory.

## Verification status
| What | Status |
|---|---|
| Quick-join list request + response | ✅ captured 2026-10-07, parsing covered by tests |
| Bot: list loads, decision logged (`[guild]`) | ✅ 2026-10-08 (`Staying in Venom (Instructor 93), no guild in the list is clearly better`) |
| Leaving + joining by the bot | ✅ happened between 2026-10-07 evening and 2026-10-08 (character moved from Artušova Garda to Venom). The exact `[guild] Leaving …` decision line rotated out of the log before it could be read back, so the Instructor margin on that specific switch is not directly confirmed from the log – trusted from the code, which only switches through `candidates()` (Instructor ≥ current + 10). The new `MIN_TENURE` guard (3 days) was added after this switch. |
| Join command + response | ✅ captured 2026-10-07 |
| `gs.guild.total_instructor_skill` = Instructor from the list (106 for Artušova Garda) | ⏳ not verified |
| Sign-up for attack/defense (`[guild] Signing up`), whether `Poll` refreshes planned battles | ⏳ not verified |

## Guild skill upgrades without a task (2026-10-11, user)
Gold-only upgrades of Treasure/Instructor happen even with no Gleeman task (`tasks::upgrade_guild_surplus`, before the surplus attribute buying): the cheaper of the two (Instructor on a tie), only while the price has no mushrooms and the gold stays above the shop reserve. The price ladder per skill (user, from `groupskillprice`): 5, 10, 25, 100, 250 gold, afterwards mushrooms (then it stops, never mushrooms). Capture: after an Instructor upgrade `groupskillprice(6):2500/0/10000/0/250/0` (Treasure 25 g, Instructor 100 g, a third skill 2.5 g), after the Treasure upgrade `10000/0/10000/0/250/0`. Gold priority: shop purchases > fortress > spinning the shops > guild upgrades > attributes.
**Reserve (user 2026-10-11, lowered from 5× the same day): after a guild upgrade without a task 1× the most expensive shop item must remain** (`tasks::upgrade_guild_surplus`). The surplus attributes keep 5× (`SURPLUS_RESERVE_MULTIPLE`).

## Rules changed (user 2026-10-11)
- **No minimum tenure** (the 3-day `MIN_TENURE` is gone). Switch as soon as a joinable guild has BOTH Instructor and Treasure at least 10 higher than ours (total skills, `SWITCH_MARGIN`).
- **Exception:** while an open daily/event task wants a guild fight (`TaskType::GuildReadyFight`, `FightGuildHydra`, `FightGuildPortal`) the bot stays; right after the task is done it leaves for the better guild (the check runs before the timer, so no waiting for the next window).
- **Ranking:** weighted score 3 × Instructor + 2 × Treasure (Instructor leads, "but not that much – levels are similar everywhere"), then strength (members × average level).
- **Check every 6 h** (`CHECK_EVERY`) instead of daily; the list is loaded page by page (`GroupJoinList:0,1,2…`, max 5) until a page brings no new guild. ⏳ Unverified: whether the page parameter really pages (watch for `[guild] Quick-join list page N brought …`).
- **Descriptions:** the list's description is parsed; an "N/M" in it (e.g. "5/5") is read as a skill requirement and the guild is skipped when our own Instructor or Treasure (`own_*_skill`) is below the larger number (`unmet_requirement`; free text, a false hit only skips a guild). Skips are logged. The order of N/M (Instructor/Treasure?) is unknown, hence "larger number for both".
- Not done: leaving when the CURRENT guild's description starts demanding something (no signal except being kicked – then the no-guild branch joins the best again).
- Open: whether the quick-join list contains only freely joinable guilds (very likely: the bot and the user joined through it without an invite).

## No guild → retry every 5 min (user 2026-10-12)
PajaRizz lost BloodLegacy (probably kicked) and the one join attempt failed (`group has no matching join slots`, Legion Mruczków); the 6 h `CHECK_EVERY` then left him guildless with no Instructor/Treasure bonuses. Without a guild the list is now re-checked every `NO_GUILD_RETRY` (5 min); with a guild it stays 6 h. Each retry reloads the list, so full guilds drop out. ⏳ Unverified live. Open idea (not done, asked): hold new expeditions for a limited time until a guild is joined; running expeditions are never interrupted (unknown whether the bonus counts at start or at the end).

## Event weekends (2026-10-16, user)
During Exceptional XP / Glorious Gold the guild's Instructor bonus counts double and gold five-fold, so a good guild matters more: while one of these events runs the quick-join list is checked every 2 h (`EVENT_CHECK_EVERY`) instead of 6 h. Every start of the bot already checks once at once (no previous check time), so the new build re-checks all characters right away. The user's friends' guild "Smietanka Shakes" (bonuses ~120 % / 130 %) is the kind of guild we want everyone in; the usual switch rule (Instructor AND Treasure ≥ ours + 10, not full, no unmet skill requirements) is unchanged.

## Exceptional XP: check every 15 min (2026-10-16, user: characters in good guilds pulled ahead by ~5 levels, PajaRizz/Pagan lag)
While Exceptional XP runs the list is checked every `XP_EVENT_CHECK_EVERY` = 15 min (the main loop wakes at least every 30 min, usually much more often because of the Arena) instead of 2 h; a slot in a better guild opens whenever a member leaves. Every check also logs `[guild] Top of the list (N guilds): …` – the three best guilds by the weighted score, marked FULL or REQUIREMENT when we cannot join, so "why are we not in a better guild" can be read from the log. State 2026-10-16 20:14: Pagan → The Black World (Instructor 364), PajaRizz → Predators (350, 18:34), Filminy/Mimimimi11 in Smietanka shakes (Instructor 500 / Treasure 462). The switch margin (Instructor AND Treasure ≥ +10) is unchanged. Not possible: joining guilds that are not on the quick-join list (they need an application/invite).

## Guild skills 6/6 (2026-10-16, user; one-time mushroom exception)
Guild descriptions ask for "Instructor/Treasure N/M" (`unmet_requirement`), the user wants every character to have both at 6 so we are competitive and wanted. The step 5 → 6 costs 500 g AND 2 mushrooms per skill (user), the lower levels are gold only (existing task/surplus upgrades). `guild::six_bonuses` (called from `tasks::run` after the surplus upgrades, once the Tavern is done) buys the missing level 6 of Instructor, then Treasure, when: own skill is exactly 5, the server price has 1–2 mushrooms and at most 500 g, we can pay it and still keep the shop reserve in gold. `safe.rs`: `guild_upgrade_ok` accepts it only through `six_price_ok`, the mushroom watchdog allows exactly the price. Done once per character and skill (`roster/<character>/guild_six.txt`, also written when a skill is already ≥ 6). Pet is never bought. ⏳ Verify: look for `[guild] Buying Instructor 5 → 6 …` and that the price really is 500 g + 2 mushrooms (the code refuses anything above 500 g / 2 mushrooms).

## Pagan's join/leave complaints (2026-10-18, user)
A guild member (Chii) messaged Pagan on 10-17 21:28 ("why do you join and leave the guild constantly, it messes up your progression") and 10-18 18:28 ("Dont join again its just annoying"). The 100-message `progress.log` had rotated the decisions out, the bonuses had not changed between the visits, so it was NOT the switch margin: most likely we were kicked and the guildless 5-min retry joined the same best guild again. Fix (`guild.rs`, NOT in a release yet): (1) every join/leave/kick is appended to `roster/<character>/logs/guild.log` (`glog`); (2) `track_membership` notices "had a guild, now none" (a kick) and remembers the guild in `roster/<character>/guild_avoid.txt` for 7 days; a guild we leave ourselves is avoided for 24 h; candidates skip avoided guilds (`avoided()`), BUT a guildless character always joins the best joinable guild even if avoided (user 2026-10-18: never stay without a guild). `guild_last.txt` holds the guild we were last seen in. Check `guild.log` after the next Pagan run.

## Guild skills 7/7 (2026-10-19, user; one-time mushroom exception, extends 6/6)
User: "we should raise our competitiveness on the guild job market; buy Treasure and Instructor once more, to 7/7 on all characters. Each upgrade costs 1000 g and 4 mushrooms. Mushrooms hurt, but in the long run a guild with such a strong Instructor (big extra XP) pays off." `guild::six_bonuses` now walks `STEPS` = 5 → 6 (≤ 500 g, ≤ 2 mushrooms) and 6 → 7 (≤ 1000 g, ≤ 4 mushrooms); `six_price_ok` (used by `safe.rs` and the mushroom watchdog) accepts exactly those two steps and only if the server's price is within the caps and affordable, so a surprise price refuses the purchase. Needs the shop reserve on top of the gold. Marker file `roster/<character>/guild_six.txt`: `Instructor`/`Treasure` = 6 done, `Instructor7`/`Treasure7` = 7 done; a level already past a step is marked without buying. Pet never. Tests `guild::six_tests`, `safe::guild_six_tests`. ⏳ not verified live (needs release build + restart); the 6 → 7 price (1000 g + 4 mushrooms) is the user's figure, not yet seen in `upgrade_price`.

## Guild-only expeditions and Dungeons (2026-10-19, user)
The Instructor bonus makes XP inside a guild so much bigger that it is almost a permanent XP event. So a character WITHOUT a guild pauses both XP activities and keeps trying to join one (the guild check runs every `NO_GUILD_RETRY` = 5 min; the main loop sleeps at most that long while guildless):
- **Dungeons:** `guild::dungeons_allowed` (= in a guild), checked at the top of `dungeons::run`. No override, paused all day.
- **Expeditions:** `guild::expeditions_allowed` (= in a guild, or the clock is 22:00 or later, `NO_GUILD_TAVERN_FROM_HOUR`), checked in `main.rs` before `tavern::run`. Only a NEW expedition is blocked: one already running is finished (stopping mid-way would waste it). While paused `tavern_done` stays false, so City Guard, the tasks that follow a finished Tavern and beers do not start either; the character just waits. After 22:00 expeditions run without a guild so Thirst for Adventure is used before the midnight reset.
- Arena, Fortress, shops, tasks and the rest are untouched. A hourly `[guild] No guild: … paused` line explains the wait (`guild::report_pause`). Test `guild_only_tests`. ⏳ not verified live (needs release build + restart). Open: the join is retried only every 5 min, not the instant the guild is lost; a failed switch (leaving a guild, new one full) now costs XP time, see "leave before the new one is certain" in the status.

## Switch margin 10 → 20 (2026-10-19, user)
A failed switch leaves the character guildless, and since 2026-10-19 that also pauses expeditions and Dungeons, so switching must pay off clearly: `SWITCH_MARGIN` = 20, i.e. another guild needs Instructor AND Treasure at least 20 higher than ours. Not decided yet (proposed, user took only the margin): fresh `ViewGuild` check of the target's member count before leaving, and a rejoin attempt of the old guild when the join fails.
