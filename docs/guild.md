# Guild

## Rules (from the user, 2026-10-07)
- A character without a guild picks the best guild from the quick-join list in the Guild tab ("choose a guild")
  and joins it automatically.
- The list shows: name, rank, number of members, Hall of Knights points, Treasure, Instructor, raids.
- Priority: Instructor first, then Treasure, then the overall strength of the guild (members, their levels, …).

## What sf-api 0.4.4 can do
- `ViewGuild { guild_ident }` → `hall_of_fames.other_guilds`: per member level, treasure/instructor level, pet level,
  last activity, rank; honor, finished raids. Sum over members = the guild's Treasure / Instructor.
- Inbox: guild invitations (`MessageType::GuildInvite`).
- NOT supported: the quick-join list and the join command itself. Needs a capture from the browser
  (DevTools → Network → `cmd.php`): opening the Guild tab without a guild + clicking join, with the Response bodies.

## Verification status
| What | Status |
|---|---|
| Quick-join list request + response | ⏳ waiting for the user's capture |
| Join command + response | ⏳ waiting for the user's capture |
