# Aréna

## Pravidla (od uživatele)
- Bojovat jen když postava nic nedělá (`CurrentAction::Idle`) a aréna je **mimo cooldown**.
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
- Smyčka: aréna (když jde) → jedna expedice → znovu. Když není co dělat, čeká do konce cooldownu arény (+30–120 s).
- Bot běží, dokud ho nezastavíme.

## Stav ověření
| Co | Stav |
|---|---|
| Soupeři se načtou z `enemy_ids` (jinak `CheckArena`) | ⏳ neověřeno |
| `ViewPlayer` vrátí staty soupeře | ⏳ neověřeno |
| Cooldown arény (délka) | ⏳ neověřeno |
