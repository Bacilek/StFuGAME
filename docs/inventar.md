# Inventář

## Pravidla (od uživatele)
- Kdykoli přibude předmět na nasazení, hned rozhodnout, jestli je lepší než nasazený.
  Hodnota = stejný vzorec jako síla v aréně: 100 % hlavní atribut + 80 % CON + 40 % LCK + 10 % vedlejší (z atributů předmětu).
- Zbraně (zadání uživatele: brnění nebrat v potaz, pro zbraně vlastní rovnice):
  hodnota = průměrné poškození × (1 + M / 20), M = celkový hlavní atribut postavy s touto zbraní.
  Odvozeno ze simulace boje v sf-api (`simulate/damage.rs`): úder = zbraň × (1 + A / 10), A = max(M / 2, M − M_soupeře / 2),
  proti stejně silnému soupeři A = M / 2. Ostatní atributy zbraně (CON, LCK, vedlejší) tato rovnice nebere.
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
- Rovnice zbraní ignoruje CON/LCK/vedlejší atributy zbraně. Případně doplnit po domluvě s uživatelem.

## Stav ověření
| Co | Stav |
|---|---|
| Po `Equip` spadne starý předmět do batohu | ✅ 2026-10-07 (boty, zbraň) |
| Prodej přes `SellShop` přidá stříbro | ✅ 2026-10-07 (boty za 1 g 25 s) |
