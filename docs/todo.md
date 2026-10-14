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
- [ ] Zkontrolovat, že se Fortress odemkla: TestChar1 nikdy nelogoval nic o pevnosti, user tam viděl Barracks – po restartu hledat řádek `[fortress] State:` / `No fortress data`.
- [x] Plně dořešit upgrade path (hotovo 2026-10-14, `PLAN` + `pick_generic`): kde jsme skončili? Fortress lvl 6 → rush Academy → trochu Treasury (nepřehánět, Treasury je konstanta na zlato).
- [x] Oprava domněnky: HoK se vylepšuje i během stavby (ne během stavby Fortress) – upravit kód i `docs/fortress.md`.
- [x] Priorita zlata: pořadí potvrzeno userem 2026-10-14 (itemy > pevnost > točení > cech > atributy); pevnost se opakuje, proto ostatní utrácení nechává `fortress::gold_needed` navíc.
- [x] Gem Mine: pravidlo hotovo 2026-10-14 (zbýt min. 2× rezerva + zlato další stavby pevnosti, suroviny ignorovat, držet 1–2 gemy, slabé gemy do rychle vyměňovaných itemů; `docs/fortress.md`).
- [x] Barracks upgrade: počítat dopředu – když se blíží (např. za 20 min) dostavba/upgrade Barracks, zadat trénink max tolika vojáků, aby skončil dřív (trénink blokuje upgrade).

### Balíček B – Pevnost: útoky a vojáci (`src/fortress.rs`)
- [x] Plní vojáci + soupeř, na kterého se nevyplatí útočit ani po rerollu: obětovat 1 vojáka na prohraný útok (vynutí reroll soupeře), vojáka dotrénovat.
- [x] Suroviny pevnosti nesmí přetéct (overflow) – ESPECIÁLNĚ XP na Akademii.
- [x] Hlavní postava user: přidat do bota (hotovo: `SF_ATTACK_ONLY` v `.env`, user musí přidat účet do `SF_ACCOUNTS`) v režimu "jen Fortress attacks + Academy XP collect", nic jiného; samostatné on/off tlačítko (jen na přes noc). (Navazuje na `control.rs`/`roster`.)

### Balíček C – Časování smyčky (`main` loop, `control.rs`)
- [x] Časování smyčky sjednoceno 2026-10-14 (`src/schedule.rs`, `schedule::next`; čeká na release build + živé ověření): smyčka spí do nejbližší události, čekání na expedici se probouzí i pro pevnost, City Guard, půlnoc, denní odměny a reporty.

### Balíček D – Expedice a lektvary (`src/tavern.rs`, `src/potions.rs`)
- [x] Ověřit/nastavit pořadí odměn: houby > zlato > hodiny >>> suroviny pevnosti. (hotovo 2026-10-14, `tavern::choose_reward`)
- [x] Drobný zbytek Thirst for Adventure: upřednostnit zlato před hodinami (zrušit dosavadní preferenci hodin pro "leftover" expedici; upravit CLAUDE.md pravidlo Tavern/Rewards). (hotovo 2026-10-14)
- [x] Pjotr: má vypité medium INT potion (zbývá 1 den) a v inventu 2 další medium INT – proč si je nedá? Ověřit logiku, hlídat místo v batohu (bot musí upřednostnit zbavit se potionů před jinými věcmi / vypít stejný pro prodloužení). **OPRAVENO 2026-10-14 (user: stejný typ+velikost PRODLOUŽÍ, jen jiný typ/velikost přepíše čas):** `bag_step` pije z batohu stejný lektvar jako aktivní target hned.
- [x] Mrožik drží 2× Eternal Life potion?? Zjistit proč. **OPRAVENO 2026-10-14:** druhý EL v batohu se vypije a prodlouží aktivní (+7 dní); chybí-li aktivní EL, vymění nejslabší vypitý (existující `swap_secondary`).

### Balíček E – Předměty, staty, gemy (`src/inventory.rs`, `docs/inventory.md`)
- [x] (už hotovo 2026-10-12, `docs/inventory.md`) Smysluplné počítání statů s gemy: např. 44 CON item + 66 CON gem > 46 STR item; item s prázdným slotem – jak hodnotit potenciální gem (navazuje na precedent 2026-10-12 "empty slot counts a part"). Na nižších levelech se obměňuje často, do budoucna ne.
- [x] (hotovo 2026-10-14, `inventory::damage_points`; čeká na release build + kontrolu `inventory.log`) Lepší porovnání zbraní: u spousty postav zbraň s Luck / sekundárními (useless) staty, protože se bere hlavně damage – zohlednit hodnotu statů.
- [x] Zrušit upgrade sekundárních atributů (`src/attributes.rs`) – ani 10 %.

### Balíček F – Arena / Hall of Fame / Scrapbook
- [ ] Jak dobře funguje útočení v Hall of Fame na doplnění scrapbooku? Kdy, kolikrát, úspěšnost – vyhodnotit z logů.
- [x] Pagan "nejel Arenu" (2026-10-14): jezdil každý den (10 výher 11.–14.), řádek 13.10. v `history.csv` měl `arena_wins`=0 (server čítač `fights_for_xp` se už vynuloval). Opraveno: `arena::xp_wins_today` (z deníku) + max; řádek opraven ručně.
- [x] HoF "day 6" (13.10.): vysvětleno, žádná chyba – rank je relativní (honor roste ~100–180/den, ale ostatní hráči serveru rostou rychleji, rank se u Květoše/Wecrose/Sanka/Mimimimi11 zhoršil o ~500–900). Nic k opravě.

### Balíček G – GUI / dashboard (`src/dashboard.html`, `src/app.rs`)
- [x] Dlaždice postav: timestamp snapshotu/stavu (hotovo 2026-10-14, `app.html`).
- [x] "Why up": guild "(x%/y%)", upgrade jen "+x %" (hotovo 2026-10-14, `roster::changes`).
- [ ] Taby přejmenovány (hotovo 2026-10-14, `METRICS` v `dashboard.html`); ZBÝVAJÍ ikonky ze hry (nejdřív zjistit, co je v assetech): "Gold", "XP", "Shrooms", "Arena", "Dungeon", "Hall of Fame", "Power", "Win Rate", "Scrapbook"; ikonky ze hry (min. Gold, XP, Shrooms, Hall of Fame, Scrapbook) – nejdřív zjistit, co se dá ve hře/assetech najít, pak implementovat.
- [x] Autostart s Windows (hotovo 2026-10-14, `app::ensure_autostart`, klíč HKCU Run, jen release exe; user 2026-10-14: ano, autostart; pozor na pravidlo: postavy se spouští jen podle poslední volby v `roster/switches.json`).
- [x] NOVÝ Fortress tab v aplikaci: dlaždice postav jako v ostatních tabech – levely budov, čas snapshotu, suroviny (kolik z kolika), plán co dál, vojáci atd. (user 2026-10-14)

### Balíček H – Monitoring / ověřování (bez kódu)
- [x] (ověřeno ve zdroji 2026-10-14: sf-api `UpgradeableFighter` počítá vložené gemy do atributů pro simulaci, server totals/`attrs_total` je obsahují, snapshot batohu ukládá `Gem` itemy a stav gem slotu v popisu; Gem Mine zásoba mimo batoh se neukládá) Gemy ve snapshotech a výpočtech (user 2026-10-14): ověřit, zda snapshot postavy / inventáře (`now.json`, `days/<date>.json`, karta) ukládá gemy – jak v itemech (socket slotech), tak volné gemy mimo sloty (inventář / Gem Mine); a zda win-rate simulace (`tournament.rs`) a výpočet power (`arena::power`, `roster::derived`) s gemy počítají (bonus atributů z gemů v itemech). Navazuje na Balíček E (gemy v hodnocení itemů).
- [ ] Dungeon se na hlavní postavě odemkl až na lvl 31 místo 30 (dřív bylo potřeba najít klíč od dungeonu, teď si user klíče nevšiml) – jen monitorovat na postavách, že se další dungeon získá (user 2026-10-14).
- [ ] TestChar1 daily quests: úkoly plní (Arena 10/10, Druid 3/3), ALE truhly za 5 a 10 bodů zůstaly při 12 bodech neotevřené (`quests.json` 16:57) a v logu nejsou žádné `[tasks]` řádky. Po restartu hledat `[tasks] Claiming` u TestChar1; podezření: průchod se na TestChar1 často vrací dřív (Tavern větev `continue`) než dojde na `tasks::run`.

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
