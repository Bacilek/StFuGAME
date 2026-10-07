# StFuGAME

Bot pro Shakes & Fidget, který automatizuje denní úkoly (hospoda, nákupy, aréna, podzemí, …).

## Kontext
- Uživatel Rust neumí. Kód píše Claude, uživateli vysvětluje jen to nutné.
- Uživatel umí C#, vysvětlení ve srovnání s C# jsou vítána.
- Komunikace česky.

## Technologie
- Rust, knihovna `sf-api` (crates.io), async přes `tokio`, `.env` přes `dotenvy`.
- Reference implementace: github.com/tjira/rsfb (nekopírovat, jen jako vzor volání API).
- Přihlašovací údaje z `.env` (SF_USER, SF_PASS = S&F účet; SF_CHARACTER = jméno postavy; SF_SERVER volitelně), nikdy je nehardcodovat a nevypisovat do logů. Soubor `.env` nečíst.

## Pravidla
- Plán dalších funkcí: `docs/todo.md` (nové nápady připisovat, hotové odškrtnout). Kdykoli mě napadne velké herní TODO, připsat ho do sekce „Návrhy od Clauda“.
- Kdykoli si nejsem jistý jakýmkoli rozhodnutím (strategie, data, co bot smí), zeptat se uživatele a odpověď zapsat do `docs/precedenty.md`. Při podobné situaci se řídit precedenty.
- Mezi akcemi náhodné prodlevy (simulace člověka), žádné spamování serveru.
- ZA ŽÁDNÝCH OKOLNOSTÍ neutrácet houby, dokud uživatel pravidlo výslovně nezmění. JEDINÁ výjimka (uživatel 2026-10-07): pronájem zvířete ve stáji – gryf/drak za 25 hub, když hub není dost, tygr za 10 g + 1 houbu; jen když postava zvíře nemá a jde do hospody (viz `docs/staj.md`). Hlídač hub povolí úbytek jen u tohoto příkazu a přesně o jeho cenu. Všechny příkazy posílat přes `safe::send` (src/safe.rs), který pustí jen příkazy z whitelistu `is_allowed`. Nový příkaz přidat do whitelistu jen po ověření, že neutrácí houby. Nikdy nepovolit:
  - `BuyBeer` (pivo stojí houby), `TimeSkip::Mushroom` (v `FinishQuest { skip }` i `ExpeditionSkipWait`),
  - `GambleMushrooms`, `GuildLoadMushrooms`, cokoli s `Mushrooms` v názvu (např. `SocketUpgradeWithMushrooms`, `GemExtractWithMushrooms`),
  - a každý další příkaz, u kterého není jisté, že houby neutratí (radši odmítnout).
- Každou novou funkci přidávat zvlášť a nechat uživatele ji otestovat.
- Testuje se na vedlejším účtu.
- Po každé změně udělat git commit (česká commit zpráva) a push na origin (GitHub). `.env` nikdy necommitovat.

## Hospoda
- Hospoda už nefunguje jako klasické výpravy (questy) „vyber jednu ze 3 a počkej“. Jsou v ní expedice, kde se během cesty vybírá z možností:
  - `ExpeditionStart { pos }` (výběr ze 2 expedic) → opakovaně `ExpeditionStage` z `tavern.expeditions.active()`:
    - `Encounters` → `ExpeditionPickEncounter { pos }`, `Boss` → `ExpeditionContinue`, `Rewards` → `ExpeditionPickReward { pos }`,
    - `Waiting` → počkat do `busy_until` a poslat `Update` (nikdy neskipovat houbami), `Finished` → konec.
- Mise, pravidla a otevřené otázky: `docs/expedice.md`. Je to hlavně dokument pro Clauda: aktualizovat ho pokaždé, když se zjistí cokoli nového (z běhu, od uživatele, z FAQ).  (strojově `src/missions.rs`). Neznámé mise/setkání bot nezastaví, ale zapíše do deníku (`unmapped`), pak se zeptat uživatele a doplnit.
- Výběr expedice: nejkratší, při shodě nezmapovaná, jinak nejsnazší na 40 (`Mission::ease`).
- Strategie (src/tavern.rs): zajistit 40 hrdinství (vč. bonusů/trestů na konci), pak jen klíče a truhly a nikdy neklesnout pod 40.
  Do té doby skóre = okamžitý zisk + budoucí hodnota (plakát, krok řetězu) vážená šancí, že ho stihneme do 10. kola.
- Předměty cizích cyklů (ne hlavní mise) brát jen kvůli okamžitému hrdinství, ne kvůli budoucím krokům.
- Odměny: houby > zlato > přesýpací hodiny; u zbytkové expedice (ALU ≤ 3 min) houby > hodiny > zlato. Přesýpací hodiny, pivo ani skip houbami NEPOUŽÍVAT.
- Průběh běhu s časy: `logs/prubeh.log` (výstup přes makro `report!`, bot lze pustit na pozadí a log sledovat).
- Data misí nemusí být správně (ani od uživatele, ani z FAQ): bot je za běhu ověřuje (`checks` v deníku, `[kontrola] NESEDÍ`), po bězích aktualizovat tabulku „Stav ověření“ v docs/expedice.md a opravit `src/missions.rs`.
- Deník `logs/expedice.jsonl`: po bězích vyhodnotit (pod 40 / přehnaně nad 40) a ladit strategii.
- Klasické questy (`StartQuest`/`FinishQuest`) jen pokud `tavern.available_tasks()` vrátí `Quests`.

## Aréna
- Popis a stav ověření: `docs/arena.md` (aktualizovat při každém novém zjištění).
- Bojovat JEN mimo cooldown (jinak to stojí houbu, server příznak use_mushroom ignoruje), kdykoli, i během expedice.
- Nejvýš 10 výher denně (pak nejsou odměny), řídí se `arena.fights_for_xp` ze serveru.
- Vyzvat nejslabšího ze 3: síla = 100 % hlavní atribut + 80 % CON + 40 % LCK + 10 % vedlejší atributy.

## Podzemí
- Popis a stav ověření: `docs/podzemi.md`.
- Jeden boj kdykoli mimo cooldown (NIKDY za houby), i během expedice. S plným inventářem ne.
- Výběr: nejnižší level protivníka, při podobném levelu slabší staty.
- Cooldown hlídá `safe.rs`: po boji další až s novým časem ze serveru (`UpdateDungeons`).

## Inventář
- Popis a stav ověření: `docs/inventar.md`.
- Předmět na nasazení: lepší (vzorec z arény na atributy předmětu) nasadit, horší prodat. Epické NIKDY neprodávat.

## Hlídka
- Popis a stav ověření: `docs/hlidka.md`.
- Po dojetí hospody hlídka: min(10 h, hodiny do půlnoci zaokrouhlené nahoru) → konec 00:00–00:59. Aréna i podzemí během hlídky běží dál.

## Denní odměny
- Popis a stav ověření: `docs/odmeny.md`.
- Jen odměna za přihlášení (kalendář) a 1× denně volné točení kolem štěstí. Nikdy za houby ani šťastné mince.

## Stáj
- Popis: `docs/staj.md`. Zvíře kupovat až těsně před expedicí, když postava žádné nemá (ne hned po vypršení).

## Ovládání
- Bot se ovládá ikonou u hodin (src/tray.rs), zástupce „StFuGAME bot“ na ploše spouští `target
elease\stfugame.exe`. Podrobnosti `docs/ovladani.md`.
- Před release buildem musí být bot ukončen (zamčené exe). Nespouštět `cargo run`, když běží bot z ikony (jedna instance).
