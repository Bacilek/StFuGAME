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
- Missing: the Response body of `GroupJoinList` (to parse Instructor/Treasure/…).

## Verification status
| What | Status |
|---|---|
| Quick-join list request + response | ⏳ waiting for the user's capture |
| Join command + response | ⏳ waiting for the user's capture |
