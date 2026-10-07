# Expedice v hospodě

Zdroj: zkušenosti uživatele z testovací postavy. Strojová podoba je v `src/missions.rs`, při změně upravit obojí.

## Pravidla hry
- Expedice má vždy **10 kol** (rozcestí). Po 5. a 10. kole je boss a výběr odměny.
- **40 hrdinství = maximální odměna.** Víc nic nepřidá. Cíl: zajistit si 40 a pak farmit klíče a truhly.
- Plakát „wanted“ (`*Bounty`) dá 0, ale hledanému dá +10 (např. kostlivec +2 → +12).
- Formát bonusu: `+5/` = za každý cílový předmět, připíše se až na konci; `+10` = jednorázově při splnění.

## Známé mise

| Mise | Úspěch | Neúspěch | Kroky (hrdinství) | sf-api názvy |
|---|---|---|---|---|
| Dragon Taming | +5 / kus | 0 | princezna (−2) → drak (+10) | `Bait` → `Dragon` |
| Extinguished Fire | +4 / kus | 0 | táborák (+3) → fénix (+5) → uhasený oheň (0) | `CampFire` → `Phoenix` → `BurntCampfire` |
| Hot Carnival Craving | +3 / kus | 0 | sele (+5) | `Cake` |
| Unicorn Whisperer | +10 | 0 | roh (+1) → osel (+2) → duha (+5) → jednorožec (+7) | `UnicornHorn` → `Donkey` → `Rainbow` → `Unicorn` |
| Podium Climber | +10 / kus | 0 | malá překážka (−1) → velká překážka (−2) → stupně vítězů (+15) | `SmallHurdle` → `BigHurdle` → `WinnersPodium` |
| Sanitary Experiment | +20 | −5 | toaletní papír (0), potřeba 3× | `ToiletPaper` |
| Broken Sword (neúplné) | +8 / kus | ? | … → rozbitý meč (−4) | `BrokenSword`, předchozí kroky neznámé |

Ostatní známé věci: klíč (`Key`), truhla (`Suitcase`), kostlivec (`Dummy1..3`, předpoklad), plakáty (`*Bounty`).

## Otevřené otázky
- Je princezna opravdu `Bait` a kostlivec `Dummy`? Ověřit z výpisu.
- Podium Climber: jdou překážky po sobě, nebo stačí jedna z nich?
- Broken Sword: jaké jsou předchozí kroky (`SwordInStone`, `BentSword`?).

## Deník
Bot zapisuje každou expedici do `logs/expedice.jsonl` (výběry, hrdinství, klíče, truhly, verdikt).
Verdikt: pod 40 = neúspěch, nad 45 = nejspíš přehnané hrdinství místo truhel.
Ladicí konstanty: `OPPORTUNITY_COST` a `feasibility()` v `src/tavern.rs`.
