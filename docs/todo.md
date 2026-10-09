# TODO

Co ještě máme v plánu pošéfit. Hotové věci odškrtnout (nebo smazat) a popsat v příslušném dokumentu v `docs/`.

- [x] Kolo štěstí (jen 1× denně zdarma, `docs/daily-rewards.md`)
- [x] Nákup ve zbrojírně (`docs/shops.md`, čeká na ověření)
- [x] Nákup v čarovném obchodě
- [x] Točení v obchodech
- [x] Reklamy – rozhodnuto NEIMPLEMENTOVAT (`docs/shops.md`): `AdvertisementsCompleted` vrací `trust_counter`, nejspíš antifraud metrika reklamní sítě proti přesně tomuhle druhu automatizace. Riziko přesahuje jen houbičku.
  - Uživatel (2026-10-09): "někdy to zkusíme" – odloženo, ne zavrženo. Postup až na to dojde (jen na vedlejším účtu): (1) zachytit těla odpovědí `AdvertisementsCompleted` a `PlayerNewWares:x/2`, (2) najít příznak "reklama dnes dostupná" (klient ukazuje ikonu TV), (3) ověřit, že `PlayerNewWares:x/2` bez reklamy nestrhne houbičku, (4) uživatel přijme riziko `trust_counter`. Neimplementovat, dokud to uživatel výslovně nezadá.
- [x] Questy u Goblin Gleemana + eventové úkoly (`docs/tasks.md`): truhly, atributy, cechovní upgrade, nákupy, Training Camp, skořápky jen když body chybí
- [ ] Event game loop (piva?)
- [x] Arena fight highlights – ukládání cool zápasů (`PlayerCombatLogMark`), naimplementováno 2026-10-09
  (`src/arena_highlights.rs`, `docs/arena-highlights.md`). Čeká na release build + živé ověření (ukládá se to
  opravdu? škálování bodů sedí?).
- [x] Cech – automatický vstup do nejlepšího cechu ze seznamu rychlého vstupu (Instructor > Treasure > síla), 1× denně přechod do výrazně lepšího (`docs/guild.md`)
- [x] Stáj (`docs/stable.md`)
- [x] Skip fights? – není co skipovat, server vrací výsledek souboje hned (`docs/architecture.md`)

## Soutěž postav s kamarády
- Vše o soutěži (plán, soupiska, reporty) je jen lokálně v `roster/` (`roster/plan.md`, `roster/README.md`).

## Ověřit po nasazení
- [ ] Lektvary: pití z batohu, nákup, výměna menšího za větší (`docs/potions.md`); hodiny za zlato při točení (batoh nebo počítadlo?)
- [ ] Truhly u Goblin Gleemana: porovnat řádky `[tasks] daily/event chest …` v logu (body + odměny) s tím, co ukazuje hra (`docs/tasks.md`)

## Návrhy od Clauda
Napadlo mě během práce, k probrání s uživatelem (nic z toho se nedělá bez jeho souhlasu).

- [x] Free deal u Mushroom Dealera – naimplementováno 2026-10-08 (`daily::claim_welcome_pack`), zachyceno živě přes DevTools. Čeká na živé ověření (`docs/daily-rewards.md`).
- [x] Cechovní souboje: přihlásit se k útoku/obraně (`GuildJoinAttack`/`GuildJoinDefense`, zdarma)
- [ ] Vylepšování atributů za zlato (sf-api: `UpgradeSkill`) – jeden pevný klíč pro všechny classy, hlavní atribut zjistit z postavy (klíč ještě domluvit)
- [ ] Epické předměty plní batoh: bot je nikdy neprodá, inventář se časem zaplní a podzemí stojí. Kam s nimi?
- [ ] `WinFightsNoEpicsLegendaries`/`WinFightsNoGear` (Gleeman/event úkol, user 2026-10-09): sf-api je má v `TaskType`,
  `src/hunt.rs` je ale nemapuje (`wanted_hunt` zná jen `WinFightsAgainst`/`WinFightsBareHands`/`WinFightsNoChestplate`) –
  úkol se dnes jen přeskočí, nic nespadne. Odloženo na dobu, až bude mít postava epicy: sundání 5-10 slotů najednou
  najednou narazí na plný batoh (lektvary + věci k prodeji) mnohem snáz než dnešní 1-2 sloty u bare-hands, viz
  `docs/tasks.md`. Řešit zároveň s "Epické předměty plní batoh" výše – možná to vyřeší to samé rozšíření inventáře.
- [x] Denní odměna za přihlášení (kalendář, `docs/daily-rewards.md`)
- [ ] Záchod (sf-api: `tavern.toilet`) – házení předmětů do záchodu místo prodeje?
- [ ] Věž, mazlíčci, pevnost, podsvětí – až budou na postavě odemčené
- [ ] Odměna po 2. bossovi expedice – bot ji zatím neviděl, ověřit ve hře
- [ ] Nezmapované mise expedic: hostinský, mořský muž, jízda na tygrovi, zamilovaní (`docs/expeditions.md`)
- [ ] Týdenní server-wide eventy (sf-api: `gs.specials.events.active`, typ `Event`, s koncem `events.ends`) – bot je nikde nečte.
  2026-10-09 běží Epic Shopping Spree, Epic Good Luck, Forge Frenzy Festival, Tidy Toilet Time zároveň. Shopping
  Spree / Good Luck nevyžadují žádnou změnu kódu (jen vyšší šance na lepší věci, které beztak zpracováváme stejně).
  Forge Frenzy (kovárna/sockety) a Tidy Toilet (záchod/pevnost) se týkají funkcí, které bot vůbec neautomatizuje
  (viz "Záchod" a "Věž, mazlíčci, pevnost" výše) – případná automatizace by byla nová featura, ne reakce na event.
- [x] 2026-10-09: nová event témata od Gleemana (reset o půlnoci) krátce po startu hlásila `invalid chest` u všech
  postav – prázdný seznam úkolů, ale truhly s „0 bodů". Oprava v `src/tasks.rs` (`claim_chests`/`log_tasks`
  ignorují event sekci, dokud `event.tasks` není reálně naplněný), viz `docs/tasks.md`. Čeká na release build + restart.
