# Stav projektu (předávací dokument)

Aktualizováno: 2026-10-07 ~18:20. Při každém větším posunu přepsat.

## Kde jsme
- Bot běží u uživatele z ikony u hodin (release exe, nezávislý na Claude Code), postava **TestChar1** na **s31.sfgame.eu**
  (vedlejší testovací účet, přihlášení přes S&F účet / SSO).
- Hotové funkce: hospoda (expedice), aréna, podzemí, inventář, hlídka, denní odměny + kolo štěstí, stáj, ovládání ikonou.
  Přehled plánu: `docs/todo.md`. Rozhodnutí uživatele: `docs/precedenty.md`. Technika a záludnosti: `docs/architektura.md`.
- Stav postavy 2026-10-07 večer: level ~10, gryf do 21.10. 13:30, hlídka do 23:06 (pak 1 h do 00:06), aréna 3/10 výher,
  ALU na dnešek vyčerpané, plný inventář byl vyřešen prodejem.

## Čeká na nasazení
- Commit `d4db66e` (rovnice zbraní se všemi staty) není v běžícím release exe. Postup: uživatel v ikoně „Ukončit“ →
  `cargo build --release` → `Start-Process target\release\stfugame.exe -WorkingDirectory <projekt>`. Pak tuto sekci smazat.

## Co čeká na ověření (zkontrolovat v logu `logs/prubeh.log` a zapsat do příslušného docs)
| Co | Kdy | Kde zapsat |
|---|---|---|
| Výplata hlídky přes `FinishWork` | 2026-10-07 23:06 | `docs/hlidka.md` |
| Druhá hlídka 1 h (23:06 → 00:06) podle pravidla 00:00–00:59 | 23:06 | `docs/hlidka.md` |
| Reset ALU o půlnoci, start hospody po hlídce | 2026-10-08 00:06 | `docs/hlidka.md` |
| Odměna za přihlášení (`CollectCalendar`) | 2026-10-08 00:00 | `docs/odmeny.md` |
| Volné točení kolem štěstí 2. den | 2026-10-08 00:00 | `docs/odmeny.md` |
| Aréna/podzemí během expedice (během hlídky už ověřeno) | 2026-10-08 | `docs/arena.md`, `docs/podzemi.md` |
| Nákup gryfa ve stáji (25 hub) | až zvíře vyprší, nejdřív 2026-10-22 | `docs/staj.md` |
| Sanitary +20 / −5, bonusy Bewitched Stew, Toxic Fountain, Build A Friend | až padnou | `docs/expedice.md` |
| Revealing Lady: hypotéza „pár reálně +7, bonus +10“ | až padne znovu | `docs/expedice.md` |
| Odměna po 2. bossovi expedice (zatím nikdy neviděna) | každá expedice | `docs/expedice.md` |
| Kalibrace strategie expedic z deníku (`logs/expedice.jsonl`) | po víc bězích | `docs/expedice.md` |

## Otevřené otázky na uživatele
- Epické předměty se nikdy neprodávají → batoh se časem zaplní a podzemí stojí (v TODO).
- Nezmapované mise: hostinský (Mugs → DraftBeer → Barkeeper), mořský muž, jízda na tygrovi (Chicken → Tiger → RidingStan), zamilovaní.

## Deník expedic 2026-10-07 (shrnutí)
| Mise | Hrdinství | Verdikt |
|---|---|---|
| Dragon Taming | 46 | 40 splněno (začal uživatel) |
| Unicorn Whisperer | 45 | úspěch |
| Barkeeper (nezmapovaná) | 23 | pod 40 |
| The Sword Trial | 47 | 40 splněno |
| Revealing Lady | 54 | úspěch |
| The Sword Trial (zbytková) | 39 | pod 40 |
