# Potions

## Rules (from the user, 2026-10-07)
- At most 3 different active potions. Types: STR, DEX, INT, CON, LUCK (10 / 15 / 25 %, 3 days) and Eternal Life
  (+25 % HP directly, 7 days).
- Priority: main attribute potion, CON potion, Eternal Life. Eternal Life is rare and usually costs mushrooms → buy it
  only when it is for gold. Without it the third best is Luck.
- Only for gold. In the gold order: after equipment upgrades, before tasks and spinning; may go below the shop reserve.
- The same size potion can be drunk repeatedly, it extends the duration (stacking). A bigger size drunk while a smaller
  one of the same type is active does NOT add to the remaining time — it replaces the active potion outright, so
  whatever days were left on the smaller one are lost (verified by the user 2026-10-09: drinking 3 medium (15 %, 3
  days each) over an active small (10 %) with ~9 days left did not give 9+9 days, it reset to 9 days of medium).
- A bigger potion of an active type, with a free backpack slot: no rush, just keep the bigger one in the stock and let
  the smaller one run its course — there's no cost to waiting here.
- A bigger potion of an active type, with the backpack full: swap it in right away regardless of how many days are
  left on the smaller active one (user 2026-10-09). Reasoning: the swap always costs the same lost days of the
  smaller potion once it happens, and delaying it buys nothing — if the bot/user keeps getting more of the smaller
  size in the meantime (tasks, shop, dailies), the countdown never drops, so waiting for "it's almost out anyway"
  can mean never swapping at all. Swap only triggers on backpack pressure, not on a days-left threshold.
- Eternal Life for gold while Luck is active: buy it and keep it in the backpack, drink it when a slot frees up.
- Do not buy only when a potion runs out: keep a stock of up to 4 potions in the backpack, also non-target types (better
  some than none; cheap, buying them spins the shop, they can be sold any time in either shop). When the backpack is full,
  get rid of them from the least important (non-target first): drink (if it stacks onto an active one) or sell.
- With a full backpack it is better to remove an active non-target potion and drink a better one than to keep it.
- Replacing a smaller active potion with a bigger one (same type) only with a full backpack — immediately once full,
  not gated by days left on the active one; otherwise store the bigger one.
- A free active slot with no target potion to fill it: drink the best non-target one from the backpack instead of
  leaving it there (user 2026-10-08: even a secondary attribute helps in a fight against another class; better
  active and later swapped out than rotting in the backpack or eventually sold).

## Implementation (src/potions.rs, called from main.rs and shops.rs)
- Targets: [main attribute, CON, Eternal Life if active / in the backpack / for gold in a shop, otherwise Luck].
- Importance (for what goes first): non-target 0 < third 1 < CON 2 < main 3 (+ size), Eternal Life highest.
- Every pass of the main loop (`run`): a missing target and a free slot → drink the biggest from the backpack (`UsePotion`);
  no target available for a free slot → drink the best non-target one instead (any type not already active);
  backpack full → `make_room` once.
- `make_room` (full backpack, also before a shop purchase and to trim the stock above 4):
  1. a target potion waits in the backpack and a removable active one is less important (non-target, or a smaller
     same-type stat potion; never Eternal Life) → `RemovePotion` it and drink the better one, regardless of days left,
  2. otherwise the least important potion in the backpack: drink it if it stacks onto an active one of the same type
     (not smaller); else if it is useful (main/CON/Luck) and a less important removable active potion exists (no active of its type, never Eternal Life), remove that active one and drink this instead of selling (user 2026-10-09; Luck counts as useful even with Eternal Life); else sell it (`SellShop`, any shop). Eternal Life is never sold.
- Shops (after equipment upgrades), in this order:
  missing target + free slot + nothing in the backpack → buy the biggest gold one and drink;
  full backpack + a bigger one for gold than the active stat target → buy, `RemovePotion`, drink (any days left);
  stock: the most important gold potion (any type) when the stock (all potions in the backpack) is below 4, or when it is
  more important than the least important one in the stock (then the stock is trimmed back to 4).
  Max 4 such purchases a day. Spinning: gold potions are always spin candidates (user 2026-10-09: even with a full stock of 4, they are the cheapest spin items); right after such a purchase `trim_stock` drinks/sells the least important potion, so the stock stays at 4.
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
