# Architektura a záludnosti

## Moduly (src/)
| Soubor | Co dělá |
|---|---|
| `main.rs` | Start: najde složku s `.env`, pojistka jedné instance, tokio runtime, ikona. `run_bot` = přihlášení + smyčka `play`, znovupřihlášení po ztrátě session. `play`: odměny → inventář → aréna → podzemí → hospoda (1 expedice) → hlídka → čekání. |
| `tray.rs` | Ikona u hodin (tray-icon + smyčka zpráv Win32), menu Spustit/Zastavit/Otevřít log/Ukončit, mutex jedné instance, MessageBox. Jediné `unsafe` v projektu. |
| `safe.rs` | **Jediná cesta na server.** Whitelist příkazů, pojistky cooldownů (aréna, podzemí, kolo), hlídač hub (úbytek → okno + `exit(2)`), náhodné pauzy 2,5–7 s. |
| `tavern.rs` | Expedice: výběr expedice, setkání, odměn, čekání, ověřování dat (`checks`), deník. Během expedice volá i inventář/arénu/podzemí/stáj. Vrací `Outcome` po každé dokončené expedici. |
| `missions.rs` | Tabulka misí/cyklů (řetězy, hrdinství, bonusy, limity). Data z FAQ + od uživatele + ověřená ze serveru. |
| `journal.rs` | Deník expedic → `logs/expedice.jsonl` (výběry, hrdinství, klíče, truhly, verdikt, `checks`, `unmapped`). |
| `arena.rs` | Aréna: 3 soupeři přes `ViewPlayer`, síla podle vzorce, boj, log `logs/arena.jsonl`. Limit `fights_for_xp` < 10. `strength()` sdílí i podzemí a inventář. |
| `dungeons.rs` | Podzemí: `UpdateDungeons`, protivníci přes `Dungeons::current_enemy` (data sf-api), výběr, boj. |
| `inventory.rs` | Inventář: nasadit lepší / prodat horší, epické nechat. Zbraně podle poškození. |
| `guard.rs` | Hlídka: `StartWork`/`FinishWork`, délka do 00:00–00:59. |
| `daily.rs` | Odměna za přihlášení (`CollectCalendar`) a volné točení kolem štěstí. |
| `stable.rs` | Pronájem zvířete před expedicí (jediná výjimka z pravidla o houbách). |
| `report.rs` | Makro `report!` = println + zápis s časem do `logs/prubeh.log`. |

Každá herní funkce vrací `tavern::Outcome` (`Done` / `SessionLost`). `SessionLost` probublá do `run_bot`, ten se znovu přihlásí.

## Záludnosti sf-api 0.4.4 a serveru (zjištěno za běhu, 2026-10-07)
- **Zastaralý stav:** sf-api aktualizuje části stavu, jen když je server pošle. Nabídka rozcestí se po výběru často
  NEobnoví → po každém herním příkazu poslat `Update` a rozhodovat až podle čerstvého stavu. Stejná nabídka 2× po sobě → znovu `Update`.
- **`ExpeditionContinue` = `ExpeditionPickReward { pos: 0 }`** (pro server „vyber možnost 1“). Nikdy neposílat naslepo.
- **Po čekání (doprava po bossovi)** sf-api ukazuje starou nabídku 5. kola a server čeká na „pokračovat“. Pozná se podle
  `floor_stage == 4` (pole není veřejné, čte se přes serde: `serde_json::to_value(exp)["floor_stage"]`) → poslat `ExpeditionContinue`.
- **Bonusy „za kus“** expedice připíše server hned po výběru v 10. kole (ne po bossovi). Plakát (bounty) platí jen jednou.
- **Čas podzemí** obnoví jen `UpdateDungeons` (ne `Update`, ne boj). Bez něj by zastaralý čas pustil boj za houbu.
- **`use_mushroom` server ignoruje** (aréna, podzemí): boj na cooldownu stojí houbu vždy. Proto `safe::cooldown_free`:
  po našem boji smí další jen s NOVÝM časem ze serveru, pozdějším než náš boj.
- **Session** přes S&F účet (SSO) server ruší zhruba po 7–10 min (a při přihlášení na postavu v prohlížeči) → chyba
  `sessionid invalid` → `run_bot` se přihlásí znovu (limit 3 ztráty hned po sobě; session, která vydrží 5 min, nuluje počítadlo).
- **Hodnoty serveru mají přednost** před FAQ i daty od uživatele (např. Sword Trial 6/3/−4, FAQ 5/2/−5).
- **„Zrcadlový obraz“** v podzemí sf-api vede jako válečníka s levelem 0 → počítá se jako naše postava.
- `fights_for_xp` (aréna) = dnešní výhry za xp, server ho nuluje sám.
- Velikost enumů/iterace: `gs.dungeons.light.iter()` (EnumMap), nepřidávat `enum-map` jako závislost (jiná verze než v sf-api).

## Pracovní postup (vývoj)
- Testy: `cargo test` (nepotřebuje zastaveného bota). Build pro uživatele: `cargo build --release`.
- Release build vyžaduje **ukončeného** bota (exe je zamčené) → požádat uživatele o „Ukončit“ v ikoně, po buildu spustit:
  PowerShell `Start-Process target\release\stfugame.exe -WorkingDirectory <projekt>` (nebo uživatel zástupcem na ploše).
- Nespouštět `cargo run`, když běží bot z ikony (jedna instance → okno „Bot už běží“).
- Průběh bota sledovat přes `logs/prubeh.log` (např. Monitor s `tail -F` a filtrem na `UBYLY|NESEDÍ|Chyba|…`).
- Po každé změně: commit (česky) + push. GitHub občas vrací 500 → zkusit znovu později.
- Python heredoc pro úpravy souborů: pozor na `\r`, `\s` apod. v běžných řetězcích (jednou rozbilo CLAUDE.md) → raw stringy.
