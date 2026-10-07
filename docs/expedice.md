# Expedice v hospodě

Zdroje:
- oficiální FAQ: https://playa-games.helpshift.com/hc/en/4-shakes-fidget-1653988985/faq/281-expeditions---encounters/
  (cykly setkání, hrdinství kroků, limity),
- zkušenosti uživatele (bonusy za úkol od zadavatele mise),
- ověření z běhu bota (`logs/expedice.jsonl`).

Strojová podoba je v `src/missions.rs`, při změně upravit obojí.

## Průběh expedice
Klik na zadavatele → výběr ze 2 expedic → 5 kol (rozcestí, bez čekání) → boss → výběr odměny → čekání/doprava
→ „pokračovat“ → 5 kol → boss → čekání → konec. Celkem vždy **10 kol**.

Ověřeno z běhu:
- Po čekání server čeká na „pokračovat“ (`ExpeditionContinue`). Teprve potom pošle novou nabídku rozcestí.
- Po 2. bossovi bot žádnou nabídku odměny neviděl (`rewards` prázdné). Uživatel uvádí výběr odměny po obou bossech, zatím neověřeno.
- Bonusy „za kus“ (`+5/`) připíše server hned po výběru v 10. kole, ještě před bossem
  (Sword Trial: 19 → 15 za meč −4, pak +4 × 8 = 47).

## Pravidla
- Každé setkání přidá své hrdinství hned při výběru. Každé patří do nějakého cyklu (řetězu).
- **40 hrdinství = maximální odměna.** Víc nic nepřidá. Cíl: zajistit si 40 a pak farmit klíče a truhly.
- Limit = kolikrát se cyklus během expedice může zopakovat.
- „Trvalý konec“ = poslední krok zůstává v nabídce i po dokončení cyklu.
- Formát bonusu za úkol: `+5/` = za každý cílový předmět, připíše se na konci; `+10` = jednorázově při splnění.

## Cykly setkání

| Cyklus | Limit | Kroky (hrdinství) | sf-api názvy | Bonus za úkol |
|---|---|---|---|---|
| Kostlivci (Dummy) | – | Dummy 1.0 (1), 2.0 (2), 3.0 (3) | `Dummy1..3` | – |
| Klíč a truhla | 2× | klíč (0) → truhla (0, zlato / suroviny podle eventu) | `Key` → `Suitcase` | – |
| Hot Carnal Craving | – | kuřecí stehno (3, další rozcestí jen 2 možnosti), sele (5, další rozcestí jen 1 možnost) | `CupCake`, `Cake` | +3 / kus (sele) |
| Plakáty (Bounty) | 3× na typ | plakát (0), hledaný pak dá jednou +10 (plakát se spotřebuje) | `*Bounty` | – |
| Dragon Taming | 2× | návnada/princezna (−2) → drak (10) | `Bait` → `Dragon` | +5 / kus |
| Sanitary Emergency | 3× | toaletní papír (0), jen když ho chce zadavatel; potřeba 3× | `ToiletPaper` | +20, neúspěch −5 ⚠ |
| Extinguished Fire | 1×, trvalý konec | táborák (3) → fénix (5) → uhasený oheň (0) | `CampFire` → `Phoenix` → `BurntCampfire` | +4 / kus |
| Unicorn Whisperer | 1× | roh (1) → osel (3) → duha (5) → jednorožec (7) | `UnicornHorn` → `Donkey` → `Rainbow` → `Unicorn` | +10 |
| Podium Climber | 2× | malá překážka (−1) → velká překážka (−2) → stupně vítězů (15) | `SmallHurdle` → `BigHurdle` → `WinnersPodium` | +10 / kus |
| Revealing Lady | 1× | ponožky (0) → hromada šatů (0) → pár (12) | `Socks` → `ClothPile` → `RevealingCouple` | ? |
| The Sword Trial | 1×, trvalý konec | meč v kameni (6) → ohnutý meč (3) → zlomený meč (−4) ✱ | `SwordInStone` → `BentSword` → `BrokenSword` | +8 / kus |
| Bewitched Stew | 1× | kotel (2) → čarodějnice (−5) → čarodějný lektvar (15) | `Well` → `Girl` → `Balloons` | ? |
| Toxic Fountain Cure | 1×, trvalý konec | vílí fontána (8) → znečištěná fontána (−4) | `Prince` → `RoyalFrog` | ? |
| Build A Friend | 1× | ruka (−5) → nohy (−5) → tělo (−5) → Klaus (35) | `Hand` → `Feet` → `Body` → `Klaus` | ? |

Sanitary: FAQ píše „na konci +5, pokud máš 3 papíry, jinak −5“. Uživatel si je jistý +20 / −5, platí jeho verze (zatím neověřeno během běhu).

Bonusy za úkol z FAQ nejdou ověřit (FAQ je neuvádí), stojí na údajích uživatele. Ověřují se během běhu.

✱ Ověřeno na serveru, FAQ uvádí 5 / 2 / −5.

Plakáty existují jen pro: kostlivce, draka, uhasený oheň, jednorožce, stupně vítězů, pár, zlomený meč, čarodějný lektvar,
znečištěnou fontánu a Klause. Objeví se jen ty, jejichž cíl je v aktuální expedici.

## Stav ověření (z běhu bota)
Bot ověřuje za běhu: body setkání vs. tabulka, změnu hrdinství po každém výběru a změnu po posledním bossovi.
Výsledky jsou v deníku (`checks`), nesoulady se hlásí jako `[kontrola] NESEDÍ`. Po bězích sem přepsat, co se potvrdilo.

| Co | Stav | Důkaz |
|---|---|---|
| Plakát +10 k hledanému | ✅ | 2026-10-07: kostlivec s plakátem 25 → 37 |
| Unicorn Whisperer +10 hned při splnění | ✅ | 2026-10-07: jednorožec 8 → 25 |
| Osel +3 (uživatel psal +2) | ✅ FAQ | server ukázal `Donkey(+3)` |
| Kuřecí stehno +3, další rozcestí jen 2 možnosti | ✅ | 2026-10-07: 37 → 40, další kolo 2 možnosti |
| Dragon Taming +5 / kus na konci | ✅ | 2026-10-07: 36 → 46 za 2 draky |
| Truhla se objeví po klíči | ✅ | 2026-10-07: Barkeeper kolo 1 klíč, kolo 2 truhla |
| Hot Carnal Craving: sele → další rozcestí 1 možnost, stehno → 2 | ✅ | 2026-10-07: Barkeeper kola 6 a 7 |
| Sword Trial: 6 / 3 / −4 (FAQ 5 / 2 / −5, uživatel −4 u zlomeného) | ✅ server | 2026-10-07: nabídka i připsané hrdinství |
| Revealing Lady: pár +12 | ❌ nesedí | 2026-10-07 (Barkeeper, pár nebyl cíl, bez plakátu): server ukázal +12, hrdinství 16 → 23 (+7) |
| Revealing Lady: bonus za úkol | ⏳ hypotéza +10 | 2026-10-07 (cíl, s plakátem): 3 → 30 (+27). Sedí „pár ve skutečnosti +7, plakát +10, bonus +10“; alternativa „pár +12, bonus +5“ nesedí na Barkeeper běh |
| Sword Trial +8 / kus (uživatel) | ✅ | 2026-10-07: 4 zlomené meče, +32 po výběru v 10. kole |
| Plakát platí jen jednou (spotřebuje se) | ✅ | 2026-10-07: druhý zlomený meč už bez +10 |
| Bewitched Stew: čarodějnice −5, lektvar +15 | ✅ | 2026-10-07: Sword Trial kola 6–7 |
| Ostatní bonusy za úkol, Sanitary +20 / −5 | ⏳ | čeká na ověření |

## Nezmapované
- Mise z sf-api, které FAQ nezná: hostinský (`Mugs` → `DraftBeer` → `Barkeeper`), mořský muž (`FishingRod` → `FishingBait` → `Merman`),
  jízda (`Chicken` → `Tiger` → `RidingStan`), zamilovaní (`Cupid` → `LovestruckShakes` → `LoveBirds`).
- Chybějící bonusy za úkol: Revealing Lady, Bewitched Stew, Toxic Fountain Cure, Build A Friend (bot zatím odhaduje +5 při splnění).

Bot se na neznámých věcech nezastavuje. Vypíše je a zapíše do deníku (`unmapped`).
U nezmapované mise počítá předměty ze stejné číselné desítky jako cíl za kroky řetězu (např. 151 → 152 → 153).

## Výběr expedice
1. Nejkratší (nejméně ALU).
2. Při stejné délce: nezmapovaná mise nebo mise s neověřeným bonusem, jinak nejvyšší snadnost.
   Snadnost = (hrdinství kroků + bonus + odvrácený trest) / počet kol. Je to hrubý odhad, ladit podle deníku.

## Deník
Bot zapisuje každou expedici do `logs/expedice.jsonl` (výběry, hrdinství, klíče, truhly, verdikt).
Verdikt: pod 40 = neúspěch, nad 45 = nejspíš přehnané hrdinství místo truhel.
Ladicí konstanty: `OPPORTUNITY_COST` a `feasibility()` v `src/tavern.rs`.
