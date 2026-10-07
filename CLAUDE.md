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
- Mise, pravidla a otevřené otázky: `docs/expedice.md` (strojově `src/missions.rs`). Neznámé mise/setkání bot nezastaví, ale zapíše do deníku (`unmapped`), pak se zeptat uživatele a doplnit.
- Výběr expedice: nejkratší, při shodě nezmapovaná, jinak nejsnazší na 40 (`Mission::ease`).
- Strategie (src/tavern.rs): zajistit 40 hrdinství (vč. bonusů/trestů na konci), pak jen klíče a truhly a nikdy neklesnout pod 40.
  Do té doby skóre = okamžitý zisk + budoucí hodnota (plakát, krok řetězu) vážená šancí, že ho stihneme do 10. kola.
- Odměny: houby > zlato > přesýpací hodiny. Přesýpací hodiny, pivo ani skip houbami NEPOUŽÍVAT.
- Průběh běhu s časy: `logs/prubeh.log` (výstup přes makro `report!`, bot lze pustit na pozadí a log sledovat).
- Deník `logs/expedice.jsonl`: po bězích vyhodnotit (pod 40 / přehnaně nad 40) a ladit strategii.
- Klasické questy (`StartQuest`/`FinishQuest`) jen pokud `tavern.available_tasks()` vrátí `Quests`.
