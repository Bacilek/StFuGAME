# TODO

Co ještě máme v plánu pošéfit. Hotové věci odškrtnout (nebo smazat) a popsat v příslušném dokumentu v `docs/`.

- [x] Kolo štěstí (jen 1× denně zdarma, `docs/daily-rewards.md`)
- [x] Nákup ve zbrojírně (`docs/shops.md`, čeká na ověření)
- [x] Nákup v čarovném obchodě
- [x] Točení v obchodech
- [ ] Reklamy – protočení obchodu za reklamu 1× denně v každém obchodě, až je vše za houbičky (`docs/shops.md`). Příkazy známe (`AdvertisementsCompleted:5`, `PlayerNewWares:2/2` pro Magic Shop), čeká se na záchyt z Weapon Shopu + odpovědi serveru (kde server říká, že je reklama dostupná). Bez toho nespouštět (riziko houbičky).
- [x] Questy u Goblin Gleemana + eventové úkoly (`docs/tasks.md`): truhly, atributy, cechovní upgrade, nákupy, Training Camp, skořápky jen když body chybí
- [ ] Event game loop (piva?)
- [x] Cech – automatický vstup do nejlepšího cechu ze seznamu rychlého vstupu (Instructor > Treasure > síla), 1× denně přechod do výrazně lepšího (`docs/guild.md`)
- [x] Stáj (`docs/stable.md`)
- [x] Skip fights? – není co skipovat, server vrací výsledek souboje hned (`docs/architecture.md`)

## Denní report postav (roster/, jen lokálně)
- [ ] Každý den ~23:50 pro každou postavu složka `roster/<nick>/`: `card.html` (karta postavy místo screenshotu – level, atributy, výbava, lektvary, mount, cech, zlato, houby), `days/<datum>.md` (staty, největší úspěch dne, problémy), `history.csv` (řádek za den)
- [ ] Vždy na konci dne: **pořadí v Hall of Fame** a **level** (do denního záznamu, history.csv i leaderboardu)
- [ ] Průběžně během dne `notes.log` (úspěchy: level up, epic, truhla s houbami…; problémy: unmapped, MISMATCH, odmítnutý příkaz, chyby, zaseknutí, nejistoty) → ve 23:50 souhrn do `roster/issues.txt` (všechny postavy, otázky na uživatele)
- [ ] `roster/leaderboard.md`: denní pořadí všech postav (level, síla, pořadí HoF, čest), za 14 dní graf vývoje
- [ ] Závěrečný turnaj: simulace soubojů každý s každým (sf-api simulation)
- [ ] Denní účet hub (příjem/výdej a za co), stav bota (výpadky, relogin, chyby), datum založení postav
- [ ] Nejdřív odladit na TestChar1, pak pro všechny postavy (vyžaduje více postav v jednom procesu)

## Ověřit po nasazení
- [ ] Lektvary: pití z batohu, nákup, výměna menšího za větší (`docs/potions.md`); hodiny za zlato při točení (batoh nebo počítadlo?)
- [ ] Truhly u Goblin Gleemana: porovnat řádky `[tasks] daily/event chest …` v logu (body + odměny) s tím, co ukazuje hra (`docs/tasks.md`)

## Návrhy od Clauda
Napadlo mě během práce, k probrání s uživatelem (nic z toho se nedělá bez jeho souhlasu).

- [ ] Free deal u Mushroom Dealera (`ShopCheckout` jen pro položky za 0, `docs/daily-rewards.md`) – zachytit až na další (nové) postavě, až se deal objeví: přesné params `ShopCheckout` (Request URL) a odpověď `ShopCatalog` do `my_input`
- [x] Cechovní souboje: přihlásit se k útoku/obraně (`GuildJoinAttack`/`GuildJoinDefense`, zdarma)
- [ ] Vylepšování atributů za zlato (sf-api: `UpgradeSkill`) – jeden pevný klíč pro všechny classy, hlavní atribut zjistit z postavy (klíč ještě domluvit)
- [ ] Soutěž s kamarády: ~10 postav (každá jiná classa, stejný svět, nejspíš 10 účtů) běží 14 dní, pak porovnat sílu. Potřeba: více postav v jednom procesu (stav cooldownů ze `static` do postavy, logy pro každou postavu zvlášť), nákup atributů, ověřit všechny classy v inventáři/Aréně (Assassin 2 zbraně, Warrior štít, …), běh 14 dní bez přerušení (autostart, obnova po pádu). Domluvit: jak měřit „nejsilnější“.
- [ ] Epické předměty plní batoh: bot je nikdy neprodá, inventář se časem zaplní a podzemí stojí. Kam s nimi?
- [x] Denní odměna za přihlášení (kalendář, `docs/daily-rewards.md`)
- [ ] Záchod (sf-api: `tavern.toilet`) – házení předmětů do záchodu místo prodeje?
- [ ] Věž, mazlíčci, pevnost, podsvětí – až budou na postavě odemčené
- [ ] Odměna po 2. bossovi expedice – bot ji zatím neviděl, ověřit ve hře
- [ ] Nezmapované mise expedic: hostinský, mořský muž, jízda na tygrovi, zamilovaní (`docs/expeditions.md`)
