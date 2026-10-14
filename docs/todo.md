# TODO

Co ještě máme v plánu pošéfit. Hotové věci odškrtnout (nebo smazat) a popsat v příslušném dokumentu v `docs/`.

- [x] Kolo štěstí (jen 1× denně zdarma, `docs/daily-rewards.md`)
- [x] Nákup ve zbrojírně (`docs/shops.md`, čeká na ověření)
- [x] Nákup v čarovném obchodě
- [x] Točení v obchodech
- [x] ~~Reklamy~~ – **NEPLÁNUJEME** (user 2026-10-11: žádná automatizace reklam, `AdvertisementsCompleted` se nikdy neposílá, reklamy se koukají ručně; kód `adtest.rs` smazán, `docs/shops.md`). Důvod: `trust_counter` je nejspíš antifraud metrika reklamní sítě. Experiment z 2026-10-10 (TestChar1, refresh obchodů zdarma) prošel, ale nepoužije se.
  - [x] ~~Pozorování TestChar1 po reklamách~~ – zrušeno spolu s automatizací reklam.
- [x] Questy u Goblin Gleemana + eventové úkoly (`docs/tasks.md`): truhly, atributy, cechovní upgrade, nákupy, Training Camp, skořápky jen když body chybí
- [ ] Event game loop (piva?)
- [x] Arena fight highlights – ukládání cool zápasů (`PlayerCombatLogMark`), naimplementováno 2026-10-09
  (`src/arena_highlights.rs`, `docs/arena-highlights.md`). Čeká na release build + živé ověření (ukládá se to
  opravdu? škálování bodů sedí?).
- [x] Cech – automatický vstup do nejlepšího cechu ze seznamu rychlého vstupu (Instructor > Treasure > síla), 1× denně přechod do výrazně lepšího (`docs/guild.md`)
- [x] Stáj (`docs/stable.md`)
- [x] Skip fights? – není co skipovat, server vrací výsledek souboje hned (`docs/architecture.md`)

- [ ] Pevnost (Fortress, odemčená od levelu 25, `docs/fortress.md`): pravidla od uživatele zapsaná (nikdy Wall; Gem Mine až od levelu 125; upgrade soustředit na Fortress + Laborers' Quarters). Základ naimplementován 2026-10-11 (`src/fortress.rs`: sběr surovin, dokončení + stavba Fortress / Woodcutter's Hut / Laborers' Quarters). Čeká na živé ověření a na odpověď ohledně dalších budov (Quarry atd.); později Gem Mine od levelu 125, jednotky, protiútoky.

## Backlog z 2026-10-14 (balíčky, pořadí k domluvě)
Zapsáno z poznámek uživatele po 2 dnech bez session. Seskupeno podle toho, co spolu souvisí (jeden průchod = jeden commit/balíček).
Rozhodnutí (už platí, zapsat do `docs/precedents.md` při implementaci): sekundární atributy se nevylepšují vůbec (ani těch 10 %); rezerva pro upgrady atributů 5× → 10×; u drobného zbytku Thirst for Adventure bereme zlato místo hodin; pořadí odměn z expedic: houby > zlato > hodiny >>> suroviny pevnosti (útoky dávají mnohem víc); HoK jde vylepšovat i při stavbě (jen asi ne při stavbě samotné Fortress) – oprava dosavadní domněnky.

### Balíček A – Pevnost: plán a pořadí (`src/fortress.rs`, `docs/fortress.md`)
- [ ] Zkontrolovat, že se Fortress odemkla (check).
- [ ] Plně dořešit upgrade path: kde jsme skončili? Fortress lvl 6 → rush Academy → trochu Treasury (nepřehánět, Treasury je konstanta na zlato).
- [ ] Oprava domněnky: HoK se vylepšuje i během stavby (ne během stavby Fortress) – upravit kód i `docs/fortress.md`.
- [ ] Priorita zlata: jak moc upřednostňujeme fortress goldy před ostatními výdaji a v jakém pořadí (dohledat, ukázat userovi); rezerva atributů 5× → 10×.
- [ ] Gem Mine: kdy těžit (gem stojí 150 g, těžba trvá jen 50 min) – rozhodnout pravidlo.
- [ ] Barracks upgrade: počítat dopředu – když se blíží (např. za 20 min) dostavba/upgrade Barracks, zadat trénink max tolika vojáků, aby skončil dřív (trénink blokuje upgrade).

### Balíček B – Pevnost: útoky a vojáci (`src/fortress.rs`)
- [ ] Plní vojáci + soupeř, na kterého se nevyplatí útočit ani po rerollu: obětovat 1 vojáka na prohraný útok (vynutí reroll soupeře), vojáka dotrénovat.
- [ ] Suroviny pevnosti nesmí přetéct (overflow) – ESPECIÁLNĚ XP na Akademii.
- [ ] Hlavní postava user: přidat do bota v režimu "jen Fortress attacks + Academy XP collect", nic jiného; samostatné on/off tlačítko (jen na přes noc). (Navazuje na `control.rs`/`roster`.)

### Balíček C – Časování smyčky (`main` loop, `control.rs`)
- [ ] Jak se časuje kontrola postavy? Konstantní interval (10 min) vs. časovač na nejbližší událost (expedice končí za 8 min → vzbudit za 8 min). Optimalizovat i s ohledem na fortress útoky a odměny Akademie (plná kapacita už po ~1 h na začátku). Nejdřív popsat userovi současný stav.

### Balíček D – Expedice a lektvary (`src/tavern.rs`, `src/potions.rs`)
- [ ] Ověřit/nastavit pořadí odměn: houby > zlato > hodiny >>> suroviny pevnosti.
- [ ] Drobný zbytek Thirst for Adventure: upřednostnit zlato před hodinami (zrušit dosavadní preferenci hodin pro "leftover" expedici; upravit CLAUDE.md pravidlo Tavern/Rewards).
- [ ] Pjotr: má vypité medium INT potion (zbývá 1 den) a v inventu 2 další medium INT – proč si je nedá? Ověřit logiku, hlídat místo v batohu (bot musí upřednostnit zbavit se potionů před jinými věcmi / vypít stejný pro prodloužení).
- [ ] Mrožik drží 2× Eternal Life potion?? Zjistit proč.

### Balíček E – Předměty, staty, gemy (`src/inventory.rs`, `docs/inventory.md`)
- [ ] Smysluplné počítání statů s gemy: např. 44 CON item + 66 CON gem > 46 STR item; item s prázdným slotem – jak hodnotit potenciální gem (navazuje na precedent 2026-10-12 "empty slot counts a part"). Na nižších levelech se obměňuje často, do budoucna ne.
- [ ] Lepší porovnání zbraní: u spousty postav zbraň s Luck / sekundárními (useless) staty, protože se bere hlavně damage – zohlednit hodnotu statů.
- [ ] Zrušit upgrade sekundárních atributů (`src/attributes.rs`) – ani 10 %.

### Balíček F – Arena / Hall of Fame / Scrapbook
- [ ] Jak dobře funguje útočení v Hall of Fame na doplnění scrapbooku? Kdy, kolikrát, úspěšnost – vyhodnotit z logů.
- [ ] Pagan včera nejel Arenu – proč? (log)
- [ ] Proč je Hall of Fame chart "day 6" propadák u většiny postav?

### Balíček G – GUI / dashboard (`src/dashboard.html`, `src/app.rs`)
- [ ] Dlaždice postav: timestamp snapshotu/stavu.
- [ ] "Why up" guild bonusy: zkrátit zprávu na "(x%/y%)"; "huge x upgrade" → "+x%", nic víc.
- [ ] Přejmenovat taby: "Gold", "XP", "Shrooms", "Arena", "Dungeon", "Hall of Fame", "Power", "Win Rate", "Scrapbook"; ikonky ze hry (min. Gold, XP, Shrooms, Hall of Fame, Scrapbook) – nejdřív zjistit, co se dá ve hře/assetech najít, pak implementovat.
- [ ] Autostart aplikace s Windows (user 2026-10-14: ano, autostart; pozor na pravidlo: postavy se spouští jen podle poslední volby v `roster/switches.json`).
- [ ] NOVÝ Fortress tab v aplikaci: dlaždice postav jako v ostatních tabech – levely budov, čas snapshotu, suroviny (kolik z kolika), plán co dál, vojáci atd. (user 2026-10-14)

### Balíček H – Monitoring / ověřování (bez kódu)
- [ ] Dungeon se na hlavní postavě odemkl až na lvl 31 místo 30 (dřív bylo potřeba najít klíč od dungeonu, teď si user klíče nevšiml) – jen monitorovat na postavách, že se další dungeon získá (user 2026-10-14).
- [ ] TestChar: dělá daily quests? Ověřit v logu.

## Soutěž postav s kamarády
- Vše o soutěži (plán, soupiska, reporty) je jen lokálně v `roster/` (`roster/plan.md`, `roster/README.md`).

## Ověřit po nasazení
- [ ] Lektvary: pití z batohu, nákup, výměna menšího za větší (`docs/potions.md`); hodiny za zlato při točení (batoh nebo počítadlo?)
- [ ] Truhly u Goblin Gleemana: porovnat řádky `[tasks] daily/event chest …` v logu (body + odměny) s tím, co ukazuje hra (`docs/tasks.md`)

## Návrhy od Clauda
- Eventy na XP / houby: zvážit utrácení hub za piva (chest "extra pivo" dává 11. pivo denně za houbu) – user 2026-10-12, zatím neřešit.
Napadlo mě během práce, k probrání s uživatelem (nic z toho se nedělá bez jeho souhlasu).

- [x] Free deal u Mushroom Dealera – naimplementováno 2026-10-08 (`daily::claim_welcome_pack`), zachyceno živě přes DevTools. Čeká na živé ověření (`docs/daily-rewards.md`).
- [x] Cechovní souboje: přihlásit se k útoku/obraně (`GuildJoinAttack`/`GuildJoinDefense`, zdarma)
- [x] Vylepšování atributů za zlato (`UpgradeSkill`; úkoly + přebytek nad 5× rezervou, `docs/tasks.md`, `src/attributes.rs`)
- [ ] Epické předměty plní batoh: bot je nikdy neprodá, inventář se časem zaplní a podzemí stojí. Kam s nimi?
- [x] `WinFightsNoEpicsLegendaries` – naimplementováno 2026-10-10 (`hunt::Hunt::NoEpics`, `docs/tasks.md`); v logu 2026-10-11 splněno 5/5 u více postav.
- [ ] `WinFightsNoGear` – zatím nemapováno v `src/hunt.rs` (úkol se přeskočí, nic nespadne); sundat celou výbavu najednou naráží na plný batoh, viz "Epické předměty plní batoh" výše.
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
