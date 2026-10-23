# TODO

Co ještě máme v plánu pošéfit. Hotové věci se mažou (popsané jsou v příslušném dokumentu v `docs/`, stav v `docs/status.md`).
Naposledy pročištěno 2026-10-14. Nehotové ale pozastavené nápady jsou dole.

## Rozděláno / čeká na ověření
- [ ] Týdenní eventy pátek–neděle (`docs/events.md`): pivo při XP eventu hotové (čeká na release + ověření); Gleeman piva zrušena; čekáme na seznam dalších eventů.
- [ ] Pevnost (`docs/fortress.md`): základ i plán běží, pravidla od uživatele jsou v CLAUDE.md. Zbývá: ověřit po restartu, že se Fortress odemkla u TestChar1 (hledat `[fortress] State:` / `No fortress data`), a živě ověřit Academy rush, sacrifice vojáka, Barracks-aware trénink, `SF_ATTACK_ONLY` (user musí přidat účet hlavní postavy do `SF_ACCOUNTS`).
- [ ] Scrapbook hunts v Hall of Fame: vyhodnotit úspěšnost z `roster/<postava>/logs/hunts.log` po pár dnech běhu (kolikrát nikdo nenalezen, výhry, přírůstek itemů na výhru) – `docs/scrapbook.md`.
- [ ] Ikonky tabů v dashboardu (Gold, XP, Shrooms, Arena, Dungeon, Hall of Fame, Power, Win Rate, Scrapbook): v `roster/assets/` jsou jen sprity itemů; user musí říct, odkud UI ikony vzít (zachytit z klienta), nebo jestli stačí vlastní SVG.
- [ ] Dungeon se na hlavní postavě odemkl až na lvl 31 místo 30 (dřív klíč) – jen monitorovat, že se další dungeon získá.
- [ ] Release rebuild + živá kontrola věcí z 2026-10-14: TestChar1 `[tasks] Claiming` (truhly) a `[fortress] Error … (fortress.rs:NNN)`, `[main] Update at the start of the pass`, autostart po přihlášení do Windows, Filminy vstoupí do nového cechu.
- [ ] Lektvary: pití z batohu, nákup, výměna menšího za větší (`docs/potions.md`); hodiny za zlato při točení (batoh nebo počítadlo?)
- [ ] Truhly u Goblin Gleemana: porovnat řádky `[tasks] daily/event chest …` v logu (body + odměny) s tím, co ukazuje hra (`docs/tasks.md`)
- [ ] Otevřená otázka (`docs/status.md`): přejít s hodnotou itemů (inventář/obchody/gemy, ne Arena power) na kalibrované váhy ze simulace (`src/weaponsim.rs`, `docs/inventory.md`)?

- [ ] Čarodějnice / Witch (user 2026-10-18; odemyká se na levelu 66; zatím jen plán, nic neimplementovat bez zadání):
  - [ ] Mechanika: zjistit, jak Witch ve hře funguje (sf-api `gs.witch`, příkazy, odemčení, cooldowny), zachytit požadavky; zapsat do nového `docs/witch.md`
  - [ ] Throw into cauldron: házení předmětů do kotle (peníze, progress) – co se hodí, cena, odměny
  - [ ] Enchant: očarování předmětů – ceny, výběr předmětu, bez hub
  - [ ] Juice: lektvar/džus – co dělá, ceny, kdy vařit
  - Zadání (user 2026-10-19, vše v `docs/witch.md`): Witch denně vykupuje 1 typ itemu za 2× shopovou cenu → veškerý prodej toho typu (výměna vybavy, protáčení obchodů) směřovat k ní; z obětovaných itemů (nejspíš sdílené po serveru) se 24 h vaří nový enchantment; v Enchant tabu jde na každý typ itemu dát jeden pevný enchantment (nevybíráme, platíme goldy za aplikaci).
  - [x] Seznam enchantmentů od usera zapsán do `docs/witch.md`
  - [x] Fáze 1 (čtení): `src/witch.rs`, vlastní parsování `witchshop` (2026-10-19); ověřit živě po release buildu
  - [ ] Ověřit capture a dnešní typ itemu (sf-api `witch.required_item`), viz `docs/witch.md`
  - [x] Fáze 2 kotlík (místo prodeje), fáze 3 enchantování, fáze 4 protáčení obchodů s typem čarodějnice: hotovo 2026-10-19/20, enchant + kotlík ověřeny živě (viz `docs/witch.md`)
  - [ ] **Fáze 5: zdarma pivo z opasku (Thirsty Wanderer)** – 1 pivo denně zdarma, jen pokud byl opasek s enchantem nasazený už na začátku dne (user 2026-10-19). Čeká, až se enchant na opasek odemkne (unlocky jsou celoserverové; dnes jen prsten + brnění) a až user řekne, jak vypadá bezplatné pivo v hospodě. Potřebuje výjimku v CLAUDE.md + `safe.rs` (jen ověřeně bezplatné `BuyBeer`; houbičkový hlídač zůstává pojistkou) a nejdřív test na jedné postavě.
  - Pravidlo: nikdy za houby (viz CLAUDE.md); každý příkaz až po ověření do `safe.rs`.

- [ ] Pets (user 2026-10-20; odemyká se od lvl 75 po jedné expedici, hlavní postava po půlnoci 2026-10-21; plán a řetězce pets v `docs/pets.md`, nic neimplementováno):
  - [x] Fáze 1 (čtení): `src/pets.rs` loguje surová data (2026-10-21); zbývá dekódovat konec bloku (časovače, protivník, ovoce), viz `docs/pets.md`
  - [ ] Fáze 2: odměna z expedice houby > ovoce > zlato, sbírat ovoce
  - [x] Fáze 2 (2026-10-23): krmení main peta podle řetězce + hodinový habitat dungeon (`pets::run`, whitelist v `safe.rs`), čeká na ověření v běhu
  - [x] Fáze 3a (2026-10-23, čeká na ověření v běhu): Tavern loví chybějící pety podle `docs/pet-conditions.md` (`src/petcatch.rs`: výběr expedice, výběr odměny vejce/ovoce, noční ALU se šetří na ráno, `UnlockFeature 21/<id>`)
  - [ ] Fáze 3b: pet PvP (5×/den, alokace habitatů), habitat fights mimo event, ověřit předpoklady petcatch (roční období)
  - [ ] Fáze 4: PvP 5×/den a útoky na trenéry (ovoce)

## Návrhy od Clauda (nic se nedělá bez souhlasu uživatele)
- [ ] (2026-10-18, zatím neřešit) Spinning může přes den brát gold, který by fortress potřebovala na DALŠÍ buildy (spin šetří jen cenu jednoho následujícího buildu). Pokud přijde varování "build čeká na gold", přehodnotit priority (např. šetřit gold na víc buildů dopředu).
- [ ] **Assassin: počítá se gem v off-hand (Shield slot) dvojnásobně jako ve zbrani?** (user 2026-10-16, neví) – zjistit: Assassinovi dát stejný gem do off-handu a porovnat `derived.damage` v `roster/<postava>/days/*.json` před/po (nebo user v klientu porovná damage). Do té doby se Shield slot v `gems::weapon_socket` NEbere jako zbraň. Pokud ano, rozšířit `Socket.weapon` na Shield u Assassina.
- [ ] Smithy (vylepšuje vojáky): jen pokud soupeři mají tak silnou obranu, že nájezdy přestanou vynášet suroviny (user 2026-10-15). Vyžaduje Wall, Mages Tower a Archery Guild (levné, jednotky nikdy nevylepšujeme, chceme být napadáni) – koliduje s pravidlem „nikdy Wall“, před stavbou se zeptat.
- [ ] Expedice – min-max (user 2026-10-16, analýza v `docs/expedition-analysis.md`, report `node scripts/expedition_stats.js`): HOTOVO (čeká na ověření po release buildu): (A) stehno −1,1 a (B) prasátko/stehno penalizace „příští kolo má méně voleb“ (`tavern::restriction_cost`, 0 v posledním kole, u Hot Carnival 0), logování surových ID neznámých encounterů (`rawid.rs` → `unmapped` v deníku, „raw id N“). ZBÝVÁ: (D) po pár expedicích s novým logováním (po release buildu) namapovat Gone With the Wind (podzim, sezónní) podle raw ID z `unmapped` v deníku; LoveBirds je Valentýn (únor) – nejde ověřit dřív, ids 171–173 už sf-api zná, bonus/body zjistit až z dat tehdy; (C+F HOTOVO, čeká na ověření: `tavern::chain_feasibility` = binomická šance 0,65/kolo místo konstant 0,7/0,35, takže delší řetěz (Build A Friend) se pozdě nezačíná); potom porovnat `--since` s baseline 88,4 % (≥ 40). Truhla nedává XP → její odměnu netrackujeme, 40 hrdinství má vždy přednost před klíčem/truhlou (user 2026-10-16).
- [x] Tier list expedic (user 2026-10-16, hotovo): výběr expedice = Gleeman lokace > tier (S>A>B z oficiálního Discordu, `Mission::tier`) > kratší > ease. Čeká na ověření v běhu.
- Eventy na XP / houby: zvážit utrácení hub za piva (chest "extra pivo" dává 11. pivo denně za houbu) – user 2026-10-12, zatím neřešit.
- [ ] Epické předměty plní batoh: bot je nikdy neprodá, inventář se časem zaplní a podzemí stojí. Kam s nimi?
- [ ] `WinFightsNoGear` – zatím nemapováno v `src/hunt.rs` (úkol se přeskočí); sundat celou výbavu naráží na plný batoh (viz výše).
- [ ] Záchod (sf-api: `tavern.toilet`) – házení předmětů do záchodu místo prodeje?
- [ ] Věž, mazlíčci, podsvětí – až budou na postavě odemčené
- [ ] Odměna po 2. bossovi expedice – bot ji zatím neviděl, ověřit ve hře
- [ ] Nezmapované mise expedic: hostinský, mořský muž, jízda na tygrovi, zamilovaní (`docs/expeditions.md`)
- [ ] Týdenní server-wide eventy (sf-api: `gs.specials.events.active`, `events.ends`) – bot je nikde nečte. Shopping Spree / Good Luck nevyžadují změnu kódu; Forge Frenzy (Blacksmith: 2× suroviny, 15 místo 5 bodů za rozebrání, splintery z expedic) a Tidy Toilet (záchod: 2 oběti denně, 2× aura) by byly nové featury – popis v docs/events.md, čeká na captures.

## Zrušeno / nebude
- Automatizace reklam (user 2026-10-11): `AdvertisementsCompleted` se nikdy neposílá, reklamy se koukají ručně (`docs/shops.md`).

## Soutěž postav s kamarády
- Vše o soutěži (plán, soupiska, reporty) je jen lokálně v `roster/` (`roster/plan.md`, `roster/README.md`).
