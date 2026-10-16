# TODO

Co ještě máme v plánu pošéfit. Hotové věci se mažou (popsané jsou v příslušném dokumentu v `docs/`, stav v `docs/status.md`).
Naposledy pročištěno 2026-10-14. Nehotové ale pozastavené nápady jsou dole.

## Rozděláno / čeká na ověření
- [ ] Týdenní eventy pátek–neděle (`docs/events.md`): pravidla zapsána (XP event -> 10 piv denně, gold event sám -> žádná piva), kód zatím ne; čekáme na seznam dalších eventů.
- [ ] Pevnost (`docs/fortress.md`): základ i plán běží, pravidla od uživatele jsou v CLAUDE.md. Zbývá: ověřit po restartu, že se Fortress odemkla u TestChar1 (hledat `[fortress] State:` / `No fortress data`), a živě ověřit Academy rush, sacrifice vojáka, Barracks-aware trénink, `SF_ATTACK_ONLY` (user musí přidat účet hlavní postavy do `SF_ACCOUNTS`).
- [ ] Scrapbook hunts v Hall of Fame: vyhodnotit úspěšnost z `roster/<postava>/logs/hunts.log` po pár dnech běhu (kolikrát nikdo nenalezen, výhry, přírůstek itemů na výhru) – `docs/scrapbook.md`.
- [ ] Ikonky tabů v dashboardu (Gold, XP, Shrooms, Arena, Dungeon, Hall of Fame, Power, Win Rate, Scrapbook): v `roster/assets/` jsou jen sprity itemů; user musí říct, odkud UI ikony vzít (zachytit z klienta), nebo jestli stačí vlastní SVG.
- [ ] Dungeon se na hlavní postavě odemkl až na lvl 31 místo 30 (dřív klíč) – jen monitorovat, že se další dungeon získá.
- [ ] Release rebuild + živá kontrola věcí z 2026-10-14: TestChar1 `[tasks] Claiming` (truhly) a `[fortress] Error … (fortress.rs:NNN)`, `[main] Update at the start of the pass`, autostart po přihlášení do Windows, Filminy vstoupí do nového cechu.
- [ ] Lektvary: pití z batohu, nákup, výměna menšího za větší (`docs/potions.md`); hodiny za zlato při točení (batoh nebo počítadlo?)
- [ ] Truhly u Goblin Gleemana: porovnat řádky `[tasks] daily/event chest …` v logu (body + odměny) s tím, co ukazuje hra (`docs/tasks.md`)
- [ ] Otevřená otázka (`docs/status.md`): přejít s hodnotou itemů (inventář/obchody/gemy, ne Arena power) na kalibrované váhy ze simulace (`src/weaponsim.rs`, `docs/inventory.md`)?

## Návrhy od Clauda (nic se nedělá bez souhlasu uživatele)
- [ ] Smithy (vylepšuje vojáky): jen pokud soupeři mají tak silnou obranu, že nájezdy přestanou vynášet suroviny (user 2026-10-15). Vyžaduje Wall, Mages Tower a Archery Guild (levné, jednotky nikdy nevylepšujeme, chceme být napadáni) – koliduje s pravidlem „nikdy Wall“, před stavbou se zeptat.
- Eventy na XP / houby: zvážit utrácení hub za piva (chest "extra pivo" dává 11. pivo denně za houbu) – user 2026-10-12, zatím neřešit.
- [ ] Epické předměty plní batoh: bot je nikdy neprodá, inventář se časem zaplní a podzemí stojí. Kam s nimi?
- [ ] `WinFightsNoGear` – zatím nemapováno v `src/hunt.rs` (úkol se přeskočí); sundat celou výbavu naráží na plný batoh (viz výše).
- [ ] Záchod (sf-api: `tavern.toilet`) – házení předmětů do záchodu místo prodeje?
- [ ] Věž, mazlíčci, podsvětí – až budou na postavě odemčené
- [ ] Odměna po 2. bossovi expedice – bot ji zatím neviděl, ověřit ve hře
- [ ] Nezmapované mise expedic: hostinský, mořský muž, jízda na tygrovi, zamilovaní (`docs/expeditions.md`)
- [ ] Týdenní server-wide eventy (sf-api: `gs.specials.events.active`, `events.ends`) – bot je nikde nečte. Shopping Spree / Good Luck nevyžadují změnu kódu; Forge Frenzy (kovárna/sockety) a Tidy Toilet (záchod/pevnost) by byly nové featury.

## Zrušeno / nebude
- Automatizace reklam (user 2026-10-11): `AdvertisementsCompleted` se nikdy neposílá, reklamy se koukají ručně (`docs/shops.md`).

## Soutěž postav s kamarády
- Vše o soutěži (plán, soupiska, reporty) je jen lokálně v `roster/` (`roster/plan.md`, `roster/README.md`).
