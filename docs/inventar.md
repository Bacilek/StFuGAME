# Inventář

## Pravidla (od uživatele)
- Kdykoli přibude předmět na nasazení, hned rozhodnout, jestli je lepší než nasazený.
  Hodnota = stejný vzorec jako síla v aréně: 100 % hlavní atribut + 80 % CON + 40 % LCK + 10 % vedlejší (z atributů předmětu).
- Lepší → nasadit. Horší (nový i ten sundaný) → prodat. **Epické předměty nikdy neprodávat**, necháváme si je na později.

## Implementace (src/inventory.rs)
- Projde celý batoh (ne jen nové předměty), takže dorovná i starší stav. Bez akce nic neposílá na server.
- Předmět pro jinou třídu (nejde nasadit) → prodat, pokud není epický.
- Stejná hodnota = není lepší → prodat (pokud není epický).
- Unikátní předměty (`is_unique`) a ne-vybavení (lektvary apod.) se neřeší.
- Po každé akci `Update`, rozhoduje se vždy podle čerstvého stavu.
- Prodej: `SellShop` (sf-api vybere obchod a pozici samo), houby nebere. Nasazení: `Equip`, sundaný předmět spadne do batohu
  a v dalším kroku se vyhodnotí (prodá, pokud není epický).
- Epický = `Item::is_epic()` (model_id ≥ 50, zahrnuje i legendární).

## Otevřené
- Vzorec nebere v úvahu poškození zbraně ani brnění, jen atributy (podle zadání). Ověřit s uživatelem.

## Stav ověření
| Co | Stav |
|---|---|
| Prodej přes `SellShop` funguje a přidá stříbro | ⏳ neověřeno |
| Po `Equip` spadne starý předmět do batohu | ⏳ neověřeno |
