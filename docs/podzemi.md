# Podzemí (dungeony)

## Pravidla (od uživatele)
- Kdykoli je podzemí **mimo cooldown**, jeden boj. **Nikdy za houby** (server příznak `use_mushroom` ignoruje,
  takže boj na cooldownu by houbu stál vždy).
- Jde souběžně s hospodou i hlídkou.
- Cooldown má být 1 h (uživatel). Bot se ale řídí časem ze serveru (`dungeons.next_free_fight`).
- S plným inventářem se boj nespouští (vyřešíme jindy).
- Víc odemčených podzemí: vybrat protivníka s nejnižším levelem, při podobném levelu (do +2) toho se slabšími staty
  (síla stejně jako v aréně: 100 % hlavní + 80 % CON + 40 % LCK + 10 % vedlejší).

## Implementace (src/dungeons.rs)
- Před rozhodnutím vždy `UpdateDungeons`: čas podzemí neobnoví ani `Update`, ani boj (dokumentace sf-api).
- Protivníci: `Dungeons::current_enemy` (data sf-api), světlá i stínová podzemí, kromě věže (má vlastní příkaz).
- „Zrcadlový obraz“ (sf-api: válečník s levelem 0) se počítá jako naše postava (level a staty).
- Když boj neproběhl (plný inventář), další pokus nejdřív za 5 min.

## Pojistky (src/safe.rs, společné s arénou)
- `FightDungeon` jen s `use_mushroom: false` a jen když konec cooldownu + 30 s už uplynul.
- Po našem boji smí další boj proběhnout jen tehdy, když server mezitím poslal NOVÝ konec cooldownu (pozdější než náš boj).
  Zastaralý stav tak nikdy nepustí boj za houbu.
- Hlídač hub: když po jakémkoli příkazu ubude hub, bot se okamžitě ukončí.

## Stav ověření
| Co | Stav |
|---|---|
| Cooldown 1 h | ✅ 2026-10-07: boj 16:59:29, další 17:59:29 |
| `current_enemy` odpovídá protivníkovi ve hře | ✅ 2026-10-07: Training Camp Henry Hobbyhorse (lvl 3) → po výhře Broken Brian (lvl 4), boje proběhly |
| Výhra se „přijme“ sama (bez dalšího příkazu) | ✅ 2026-10-07: Training Camp, xp +280 |
| Boj během hlídky jde | ✅ 2026-10-07 18:01 (výhra, xp +360, +1 g 90 s) |
| Boj během expedice jde | ⏳ neověřeno |
