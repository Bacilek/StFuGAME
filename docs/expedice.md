# Expedice v hospodě

Zdroj: zkušenosti uživatele z testovací postavy. Strojová podoba je v `src/missions.rs`, při změně upravit obojí.

## Pravidla hry
- Expedice má vždy **10 kol** (rozcestí). Po 5. a 10. kole je boss a výběr odměny.
- **40 hrdinství = maximální odměna.** Víc nic nepřidá. Cíl: zajistit si 40 a pak farmit klíče a truhly.
- Plakát „wanted“ (`*Bounty`) dá 0, ale hledanému dá +10 (např. kostlivec +2 → +12).
- Formát bonusu: `+5/` = za každý cílový předmět, připíše se až na konci; `+10` = jednorázově při splnění.

## Známé mise

| Mise | Úspěch | Neúspěch | Kroky (hrdinství) | sf-api názvy | Snadnost* |
|---|---|---|---|---|---|
| Dragon Taming | +5 / kus | 0 | princezna (−2) → drak (+10) | `Bait` → `Dragon` | 6,5 |
| Extinguished Fire | +4 / kus | 0 | táborák (+3) → fénix (+5) → uhasený oheň (0) | `CampFire` → `Phoenix` → `BurntCampfire` | 4,0 |
| Hot Carnival Craving | +3 / kus | 0 | sele (+5) | `Cake` | 8,0 |
| Unicorn Whisperer | +10 | 0 | roh (+1) → osel (+2) → duha (+5) → jednorožec (+7) | `UnicornHorn` → `Donkey` → `Rainbow` → `Unicorn` | 6,3 |
| Podium Climber | +10 / kus | 0 | malá překážka (−1) → velká překážka (−2) → stupně vítězů (+15) | `SmallHurdle` → `BigHurdle` → `WinnersPodium` | 7,3 |
| Sanitary Experiment | +20 | −5 | toaletní papír (0), potřeba 3× | `ToiletPaper` | 8,3 |
| Broken Sword (neúplné) | +8 / kus | ? | … → rozbitý meč (−4) | `BrokenSword`, předchozí kroky neznámé | 4,0 |

\* Snadnost = průměrné hrdinství na kolo při splnění (kroky + bonus + odvrácený trest) / počet kol. Používá se při výběru mezi stejně dlouhými expedicemi.

Řetězy jdou vždy postupně: další krok se neobjeví, dokud nemáme předchozí (např. stupně vítězů ani velká překážka se neobjeví před malou překážkou).

Ostatní známé věci:
- princezna = `Bait` (potvrzeno),
- kostlivci = `Dummy1..3` (potvrzeno), existují 2 druhy (+2 a +3), hodnotu posílá server,
- klíč (`Key`), truhla (`Suitcase`). Truhla se bez klíče neobjeví,
- plakáty (`*Bounty`).

## Výběr expedice
1. Nejkratší (nejméně ALU).
2. Při stejné délce: nezmapovaná mise (ať ji zmapujeme), jinak nejvyšší snadnost.

## Nezmapované
Bot se na neznámých věcech nezastavuje. Nezmapované mise a setkání vypíše a zapíše do deníku (`unmapped`), pak je doplnit sem a do `src/missions.rs`.
U nezmapované mise se cílovému předmětu přičítá odhad +5 (`UNKNOWN_TARGET_GUESS`).

## Otevřené otázky
- Broken Sword: jaké jsou předchozí kroky (`SwordInStone`, `BentSword`?). Uživatel si pamatuje jen vágně.

## Deník
Bot zapisuje každou expedici do `logs/expedice.jsonl` (výběry, hrdinství, klíče, truhly, verdikt).
Verdikt: pod 40 = neúspěch, nad 45 = nejspíš přehnané hrdinství místo truhel.
Ladicí konstanty: `OPPORTUNITY_COST` a `feasibility()` v `src/tavern.rs`.
