# Potions

## Rules (from the user, 2026-10-07)
- At most 3 different active potions. Types: STR, DEX, INT, CON, LUCK (10 / 15 / 25 %, 3 days) and Eternal Life
  (+25 % HP directly, 7 days).
- Priority: main attribute potion, CON potion, Eternal Life. Eternal Life is rare and usually costs mushrooms → buy it
  only when it is for gold. Without it the third best is Luck.
- Only for gold. In the gold order: after equipment upgrades, before tasks and spinning; may go below the shop reserve.
- The same potion can be drunk repeatedly, it extends the duration (stacking).
- A bigger potion of an active type: if no slot is free and the active one has at most its 3 days → replace it
  (especially the main attribute). A long stacked smaller one (e.g. 10 days left) is a harder question → keep it for now.
- Eternal Life for gold while Luck is active: buy it and keep it in the backpack, drink it when a slot frees up.

## Implementation (src/potions.rs, called from main.rs and shops.rs)
- Targets: [main attribute, CON, Eternal Life if active / in the backpack / for gold in a shop, otherwise Luck].
- Every pass of the main loop: a missing target potion and a free slot → drink the biggest one from the backpack (`UsePotion`).
- Shops (after equipment upgrades): missing target + free slot + nothing in the backpack → buy the biggest gold one and drink;
  smaller active of a stat target with ≤ 3 days left and a bigger one for gold → buy, `RemovePotion`, drink;
  Eternal Life for gold not active and not in the backpack → buy and keep. Max 4 potion purchases a day.
- `safe.rs`: `UsePotion` only for a potion in the backpack, `RemovePotion` only per `potions::removal_ok`
  (stat potion with ≤ 3 days left, never Eternal Life). Purchases via `BuyShop` (mushroom price 0).
- Potions occupying a slot that are not targets (e.g. Luck while Eternal Life waits) are never removed, they run out.

## Verification status
| What | Status |
|---|---|
| Active potions parsed (type, size, expiry) | ⏳ not verified |
| `UsePotion` from the backpack activates the potion | ⏳ not verified |
| `RemovePotion` frees the slot | ⏳ not verified |
| Drinking a bigger potion of an active type (replace vs. stack) | ⏳ not verified |
