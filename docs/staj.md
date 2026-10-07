# Stáj

## Pravidla (od uživatele, 2026-10-07)
- Nikdy nechodit na misi bez zvířete.
- Upřednostňovat hlavní zvíře: gryf/drak (tier 4) za **25 hub** na 14 dní. Jediná povolená výjimka z pravidla o houbách.
- Kupovat až když je potřeba: postava je bez zvířete a jde do hospody (má ALU). Když zvíře vyprší třeba v 7:00
  a hospoda je dojetá, nekupovat hned, ale až s novým ALU. Ušetří se tím houby.
- Když není 25 hub: tygr/raptor (tier 3) za 10 g + 1 houbu (uživatel povolil; „nemělo by to být nikdy potřeba“,
  houby by měly jen přibývat).

## Ceny (sf-api `Mount::cost`)
| Zvíře | Tier | Cena |
|---|---|---|
| kráva | 1 | 1 g |
| kůň | 2 | 5 g |
| tygr / raptor | 3 | 10 g + 1 houba |
| gryf / drak | 4 | 25 hub |

## Implementace (src/stable.rs)
- Těsně před startem expedice: pokud `character.mount` chybí nebo `mount_end` vypršel → `BuyMount { Dragon }`,
  při nedostatku hub `BuyMount { Tiger }`. Když není ani na tygra, bot hlásí `[stáj] POZOR` a jde bez zvířete (nemělo by nastat).
- Pojistky (src/safe.rs): na whitelistu jen `BuyMount` s gryfem nebo tygrem; nákup projde jen když postava zvíře nemá;
  hlídač hub povolí úbytek jen u tohoto příkazu a přesně o cenu zvířete (jinak bot okamžitě končí).
- Nepovedený nákup se zkusí znovu nejdřív za 30 min.

## Stav ověření
| Co | Stav |
|---|---|
| Nákup gryfa (25 hub, 14 dní) | ⏳ neověřeno (uživatel dnes koupil ručně) |
