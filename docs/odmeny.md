# Denní odměny

## Pravidla (od uživatele)
- Jednou denně zatočit kolem štěstí, **jen zdarma** (volné točení). Nikdy za houby ani za šťastné mince.
- Vyzvednout denní odměnu za přihlášení (kalendář).
- Nic dalšího (truhly za úkoly apod. zatím ne).

## Implementace (src/daily.rs)
- Kalendář: `CollectCalendar`, když `specials.calendar.next_possible` už uplynul.
- Kolo: `SpinWheelOfFortune { payment: FreeTurn }`, jen když je známý `specials.wheel.next_free_spin` a uplynul (+30 s).
  Na whitelistu je jen `FreeTurn`; po točení smí další až s novým časem ze serveru (stejná pojistka jako aréna/podzemí).
- Nepovedená akce se zkusí znovu nejdřív za 30 min.

## Stav ověření
| Co | Stav |
|---|---|
| Kalendář se vyzvedne | ⏳ neověřeno |
| Volné točení kolem štěstí | ⏳ neověřeno |
