# Shadow World dungeons (user 2026-11-07; NOT implemented, the bot skips them)

- Same dungeons as the Light World, from the start, but MUCH harder: the user's main character loses to a level 180 monster in the Shadow World while it nearly beats a level 255 one in a Light dungeon.
- Fought together with the companions (sf-api `Dungeon::is_with_companions`), so they are only worth it once the companions have gear (`docs/tower.md`).
- Every win drops an epic. Not guaranteed to be for our class: may suit a companion, or us after a Wash.
- Unlock chain: beating the 5th opponent of a Shadow dungeon lets us find the KEY of the next Shadow dungeon in the Gem Mine (Night Ghoul = 5th in Desecrated Catacombs -> a key for The Mines of Gloria can be found).
  The gem search itself must therefore keep working (`src/gems.rs`); nothing in the bot handles keys yet.
- Until then `dungeons::open_dungeons` offers no Shadow dungeons at all (otherwise the "lowest enemy level" choice would pick a level 180 monster over a 255 Light one and lose the hour).
- Open: captures of a Shadow fight (request, response, epic drop), the key find message in the Gem Mine, win estimate with companions (sf-api `simulate`).
