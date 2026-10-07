# Aréna

## Pravidla (od uživatele)
- Bojovat kdykoli je aréna **mimo cooldown**, i během expedice (hospoda aréně nevadí).
- **Nejvýš 10 výher denně**, pak už aréna nedává odměny. Řídí se počítadlem serveru `fights_for_xp`
  (podle uživatele = dnešní výhry za xp, 0–10), takže den resetuje server. Pro kontrolu se výhry počítají i z `logs/arena.jsonl`.
  Na cooldownu by boj stál houbu: server příznak `use_mushroom` ignoruje a bojuje vždy (dokumentace sf-api u `Command::Fight`).
- Vyzvat nejslabšího ze 3 nabízených soupeřů.
- Síla soupeře = 100 % hlavní atribut + 80 % odolnost (CON) + 40 % štěstí (LCK) + 10 % každý vedlejší atribut.
  Např. lučištník: 100 % DEX + 80 % CON + 40 % LCK + 10 % STR + 10 % INT.
  Atribut = základ + bonusy z vybavení + bonus mazlíčků (`attribute_basis + attribute_additions + attribute_pet_bonus`).
- Hlavní atribut podle třídy (`Class::main_attribute` v sf-api): STR válečník, paladin, bitevní mág, berserker;
  DEX lučištník, assassin, lovec démonů, morový doktor; INT mág, druid, bard, nekromant.

## Pojistky (src/safe.rs)
- `Fight` jen s `use_mushroom: false` a jen když `next_free_fight` + 30 s rezerva už uplynul.
- Po našem boji smí další boj proběhnout jen tehdy, když server mezitím poslal NOVÝ konec cooldownu (pozdější než náš boj).
- Hlídač hub: když po jakémkoli příkazu ubude hub, bot se okamžitě ukončí (exit 2).

## Průběh (src/arena.rs, smyčka v src/main.rs)
- Smyčka: aréna (když jde) → jedna expedice → znovu. Během expedice se aréna zkouší mezi kroky
  a čekání se zkrátí, když se aréna uvolní dřív. Když není co dělat, bot čeká do konce cooldownu (+30–120 s),
  po 10 výhrách 30 min.
- Bot běží, dokud ho nezastavíme.

## Stav ověření
| Co | Stav |
|---|---|
| Soupeři se načtou z `enemy_ids` (jinak `CheckArena`) | ✅ 2026-10-07 |
| `ViewPlayer` vrátí staty soupeře | ✅ 2026-10-07 |
| Cooldown arény 10 min | ✅ 2026-10-07: boj 16:49:24, další 16:59:24 |
| Boj během hlídky jde | ✅ 2026-10-07 (17:13–18:14 několik bojů) |
| Boj během expedice jde | ⏳ neověřeno |
| `fights_for_xp` = dnešní výhry za xp (uživatel) | ✅ 2026-10-07: po výhře 0 → 1, po prohře beze změny |
