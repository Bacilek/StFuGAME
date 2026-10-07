# Hlídka (městská stráž)

## Pravidla (od uživatele)
- Když je hospoda dojetá (ALU vyčerpané, žádná rozjetá expedice), automaticky jít na hlídku.
- Délka: maximum 10 h, ale hlídka má skončit v poslední hodině po půlnoci (00:00–00:59), protože o půlnoci
  se resetuje ALU a začínají znovu všechny úkoly (hospoda, nakupování, …), a bot nemá zbytečně čekat
  bez hlídky. Tedy min(10, hodiny do půlnoci zaokrouhlené nahoru).
  (Původně „skončit nejpozději o půlnoci“, změněno uživatelem 2026-10-07.)
- Během hlídky jde normálně aréna i podzemí.

## Implementace (src/guard.rs, smyčka v src/main.rs)
- `StartWork { hours }` když postava nic nedělá a hospoda je dojetá; `FinishWork` (výplata) po skončení hlídky (+10 s).
- `CancelWork` se nepoužívá (není na whitelistu).
- Hlavní smyčka se probudí ke konci hlídky (výplata) a o půlnoci (reset ALU).

## Stav ověření
| Co | Stav |
|---|---|
| Hlídka se spustí a skončí v očekávaný čas | ✅ 2026-10-07: start 17:06 na 6 h, konec 23:06 (mzda 208 s/h) |
| Výplata přes `FinishWork` | ⏳ neověřeno |
| Reset ALU o půlnoci místního času | ⏳ neověřeno |
