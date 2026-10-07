# Guild

## Rules (from the user, 2026-10-07)
- A character without a guild picks the best guild from the quick-join list in the Guild tab ("choose a guild")
  and joins it automatically.
- The list shows: name, rank, number of members, Hall of Knights points, Treasure, Instructor, raids.
- Priority: Instructor first, then Treasure, then the overall strength of the guild (members, their levels, …).
- Once a day check the list and switch only to a clearly better guild (proposal: Instructor at least 10 higher).
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
- Leaving and joining again works right away (the user, 2026-10-07). After joining there is a cooldown of about 12 h
  before the character can take part in guild fights.
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
- No guild → join the best. In a guild → only if another one has Instructor ≥ ours + 10 (ours = `gs.guild.total_instructor_skill`):
  leave (`GroupRemoveMember:<own player id>`) and join. Up to 3 candidates are tried when joining fails.
  Success = `gs.guild` has the new name after `GroupJoin`.
- Commands are `Command::Custom`; `safe.rs` (`custom_allowed`) lets through only `GroupJoinList:<number>`,
  `GroupJoin:<name>/int` and `GroupRemoveMember:<own player id>` (never kicks anyone else).
- The raw response comes from our own `src/session.rs` (copy of sf-api's `SimpleSession` with `send_raw`).
- Guild fights (`GuildJoinAttack`/`GuildJoinDefense`) are not done yet (in the TODO).

## Verification status
| What | Status |
|---|---|
| Quick-join list request + response | ✅ captured 2026-10-07, parsing covered by tests |
| Join command + response | ✅ captured 2026-10-07 |
| Bot: list loads, decision logged (`[guild]`) | ⏳ not verified |
| `gs.guild.total_instructor_skill` = Instructor from the list (106 for Artušova Garda) | ⏳ not verified |
| Leaving + joining by the bot | ⏳ not verified (only when a clearly better guild appears) |
