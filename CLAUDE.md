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
- Mezi akcemi náhodné prodlevy (simulace člověka), žádné spamování serveru.
- ZA ŽÁDNÝCH OKOLNOSTÍ neutrácet houby, dokud uživatel pravidlo výslovně nezmění. Všechny příkazy posílat přes `safe::send` (src/safe.rs), který pustí jen příkazy z whitelistu `is_allowed`. Nový příkaz přidat do whitelistu jen po ověření, že neutrácí houby. Nikdy nepovolit:
  - `BuyBeer` (pivo stojí houby), `TimeSkip::Mushroom` (v `FinishQuest { skip }` i `ExpeditionSkipWait`),
  - `GambleMushrooms`, `GuildLoadMushrooms`, cokoli s `Mushrooms` v názvu (např. `SocketUpgradeWithMushrooms`, `GemExtractWithMushrooms`),
  - a každý další příkaz, u kterého není jisté, že houby neutratí (radši odmítnout).
- Každou novou funkci přidávat zvlášť a nechat uživatele ji otestovat.
- Testuje se na vedlejším účtu.
- Po každé změně udělat git commit (česká commit zpráva). `.env` nikdy necommitovat.

## Hospoda
- Hospoda už nefunguje jako klasické výpravy (questy) „vyber jednu ze 3 a počkej“. Jsou v ní expedice, kde se během cesty vybírá z možností:
  - `ExpeditionStart { pos }` (výběr ze 2 expedic) → opakovaně `ExpeditionStage` z `tavern.expeditions.active()`:
    - `Encounters` → `ExpeditionPickEncounter { pos }`, `Boss` → `ExpeditionContinue`, `Rewards` → `ExpeditionPickReward { pos }`,
    - `Waiting` → počkat do `busy_until` a poslat `Update` (nikdy neskipovat houbami), `Finished` → konec.
- Strategie (src/tavern.rs):
  - expedice: přednostně se speciální odměnou (vejce, denní úkol), jinak nejlevnější v ALU,
  - setkání: cílový předmět (dokud úkol není splněný) > nejvyšší hrdinství (plakát „wanted“ = +10 k budoucímu hledanému) ; při splněném úkolu a hrdinství ≥ 40 klíč/truhla,
  - expedice má vždy 10 kol; v 10. kole nebrat přípravné věci (plakát, klíč, princezna = `Bait`),
  - princezna: -2, ale odemkne draka +10 → počítat jako +10 do budoucna,
  - speciální pravidla cíle: rozbitý meč (`BrokenSword`) dá při sebrání -4, ale na konci +8 za kus → počítá se do limitu 40,
  - odměny: houby > zlato > přesýpací hodiny,
  - přesýpací hodiny NEPOUŽÍVAT (ani pivo, ani skip houbami).
- Klasické questy (`StartQuest`/`FinishQuest`) jen pokud `tavern.available_tasks()` vrátí `Quests`.
