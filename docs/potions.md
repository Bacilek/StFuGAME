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
- Do not buy only when a potion runs out: keep a stock of up to 4 potions in the backpack. When the backpack is full,
  get rid of them from the least important: drink (if it stacks onto an active one) or sell.
- Replacing a smaller active potion with a bigger one only with a full backpack; otherwise store the bigger one.

## Implementation (src/potions.rs, called from main.rs and shops.rs)
- Targets: [main attribute, CON, Eternal Life if active / in the backpack / for gold in a shop, otherwise Luck].
- Every pass of the main loop: a missing target potion and a free slot → drink the biggest one from the backpack (`UsePotion`).
- Shops (after equipment upgrades), in this order:
  missing target + free slot + nothing in the backpack → buy the biggest gold one and drink;
  backpack full + smaller active stat target with ≤ 3 days left + a bigger one for gold → buy, `RemovePotion`, drink;
  Eternal Life for gold not active and not in the backpack → buy and keep;
  stock (target potions in the backpack) < 4 → buy the most important target type available for gold (biggest) and keep.
  Max 4 potion purchases a day.
- Backpack full (`make_room`, used by the shop purchase before giving up): the least important potion in the backpack
  (non-target < third < CON < main, smaller first, Eternal Life last) is drunk if its type is active and it is not
  smaller than the active one (stacking), otherwise sold (`SellShop`); Eternal Life is never sold.
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
| Drinking the same type while active extends it (stacking) | ✅ 2026-10-07 per the user; bot behaviour not verified |
| Selling a potion via `SellShop` | ⏳ not verified |
