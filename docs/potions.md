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
- Do not buy only when a potion runs out: keep a stock of up to 4 potions in the backpack, also non-target types (better
  some than none; cheap, buying them spins the shop, they can be sold any time in either shop). When the backpack is full,
  get rid of them from the least important (non-target first): drink (if it stacks onto an active one) or sell.
- With a full backpack it is better to remove an active non-target potion and drink a better one than to keep it.
- Replacing a smaller active potion with a bigger one only with a full backpack; otherwise store the bigger one.

## Implementation (src/potions.rs, called from main.rs and shops.rs)
- Targets: [main attribute, CON, Eternal Life if active / in the backpack / for gold in a shop, otherwise Luck].
- Importance (for what goes first): non-target 0 < third 1 < CON 2 < main 3 (+ size), Eternal Life highest.
- Every pass of the main loop (`run`): a missing target and a free slot → drink the biggest from the backpack (`UsePotion`);
  backpack full → `make_room` once.
- `make_room` (full backpack, also before a shop purchase and to trim the stock above 4):
  1. a target potion waits in the backpack and a removable active one is less important (non-target, or a stat potion with
     ≤ 3 days left; never Eternal Life) → `RemovePotion` it and drink the better one,
  2. otherwise the least important potion in the backpack: drink it if it stacks onto an active one of the same type
     (not smaller), else sell it (`SellShop`, any shop); Eternal Life is never sold.
- Shops (after equipment upgrades), in this order:
  missing target + free slot + nothing in the backpack → buy the biggest gold one and drink;
  full backpack + smaller active stat target with ≤ 3 days left + a bigger one for gold → buy, `RemovePotion`, drink;
  stock: the most important gold potion (any type) when the stock (all potions in the backpack) is below 4, or when it is
  more important than the least important one in the stock (then the stock is trimmed back to 4).
  Max 4 such purchases a day. Spinning: gold potions are spin candidates while the stock is below 4 (kept, not sold).
- `safe.rs`: `UsePotion` only for a potion in the backpack, `RemovePotion` only per `potions::removal_ok`.

- Stock purchases confirmed 2026-10-08 (`[shops] Potion: buying … for the stock (0 in the backpack)`, count rising).

## Verification status
| What | Status |
|---|---|
| Active potions parsed (type, size, expiry) | ✅ 2026-10-08 (targets correctly picked STR/CON/LCK for the class) |
| `UsePotion` from the backpack activates the potion | ✅ 2026-10-08 (`[shops] buying and drinking` → `[potions] Drinking … from the backpack`) |
| `RemovePotion` frees the slot | ⏳ not seen yet (needs a full backpack + a smaller active potion) |
| Drinking a bigger potion of an active type (replace vs. stack) | ⏳ not seen yet |
| Drinking the same type while active extends it (stacking) | ✅ 2026-10-07 per the user; bot behaviour not verified |
| Selling a potion via `SellShop` | ✅ possible in either shop per the user; bot not verified |
