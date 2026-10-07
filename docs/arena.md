# Aréna

## Pravidla (od uživatele)
- Bojovat kdykoli je aréna **mimo cooldown**, i během expedice (hospoda aréně nevadí).
- **Nejvýš 10 výher denně**, pak už aréna nedává odměny. Výhry se počítají z `logs/arena.jsonl` (lokální datum),
  takže počet přežije restart. Pro ověření se zapisuje i `fights_for_xp` ze serveru.
  Na cooldownu by boj stál houbu: server příznak `use_mushroom` ignoruje a bojuje vždy (dokumentace sf-api u `Command::Fight`).
- Vyzvat nejslabšího ze 3 nabízených soupeřů.
- Síla soupeře = 100 % hlavní atribut + 80 % odolnost (CON) + 40 % štěstí (LCK) + 10 % každý vedlejší atribut.
  Např. lučištník: 100 % DEX + 80 % CON + 40 % LCK + 10 % STR + 10 % INT.
  Atribut = základ + bonusy z vybavení + bonus mazlíčků (`attribute_basis + attribute_additions + attribute_pet_bonus`).
- Hlavní atribut podle třídy (`Class::main_attribute` v sf-api): STR válečník, paladin, bitevní mág, berserker;
  DEX lučištník, assassin, lovec démonů, morový doktor; INT mág, druid, bard, nekromant.

## Pojistky (src/safe.rs)
- `Fight` jen s `use_mushroom: false` a jen když `next_free_fight` + 30 s rezerva už uplynul (podle čerstvého stavu).
- Hlídač hub: když po jakémkoli příkazu ubude hub, bot se okamžitě ukončí (exit 2).

## Průběh (src/arena.rs, smyčka v src/main.rs)
- Smyčka: aréna (když jde) → jedna expedice → znovu. Během expedice se aréna zkouší mezi kroky
  a čekání se zkrátí, když se aréna uvolní dřív. Když není co dělat, bot čeká do konce cooldownu (+30–120 s),
  po 10 výhrách 30 min.
- Bot běží, dokud ho nezastavíme.

## Stav ověření
| Co | Stav |
|---|---|
| Soupeři se načtou z `enemy_ids` (jinak `CheckArena`) | ⏳ neověřeno |
| `ViewPlayer` vrátí staty soupeře | ⏳ neověřeno |
| Cooldown arény (délka) | ⏳ neověřeno |
| Boj během expedice jde | ⏳ neověřeno |
| `fights_for_xp` = počet výher, nebo všech bojů? | ⏳ neověřeno |
| Den se resetuje o půlnoci místního času? | ⏳ neověřeno |
