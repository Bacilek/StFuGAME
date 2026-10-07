# Stable

## Rules (from the user, 2026-10-07)
- Never go on a mission without a mount.
- Prefer the main mount: griffin/dragon (tier 4) for **25 mushrooms** for 14 days. The only allowed exception to the mushroom rule.
- Buy only when needed: the character has no mount and is heading to the Tavern (has Thirst for Adventure). If the mount expires e.g. at 7:00
  and the Tavern is done, do not buy right away, but only with the new Thirst for Adventure. This saves mushrooms.
- Without 25 mushrooms: tiger/raptor (tier 3) for 10 g + 1 mushroom (allowed by the user; "should never be needed",
  mushrooms should only go up).

## Prices (sf-api `Mount::cost`)
| Mount | Tier | Price |
|---|---|---|
| cow | 1 | 1 g |
| horse | 2 | 5 g |
| tiger / raptor | 3 | 10 g + 1 mushroom |
| griffin / dragon | 4 | 25 mushrooms |

## Implementation (src/stable.rs)
- Right before starting an expedition: if `character.mount` is missing or `mount_end` has passed → `BuyMount { Dragon }`,
  without enough mushrooms `BuyMount { Tiger }`. If even the tiger is unaffordable, the bot reports `[stable] WARNING` and goes without a mount (should not happen).
- Safeguards (src/safe.rs): only `BuyMount` with the griffin or tiger is whitelisted; the purchase goes through only when the character has no mount;
  the mushroom watchdog allows a decrease only for this command and exactly by the mount's price (otherwise the bot exits immediately).
- A failed purchase is retried at the earliest after 30 min.

## Verification status
| What | Status |
|---|---|
| Buying the griffin (25 mushrooms, 14 days) | ⏳ not verified (the user bought it manually on 2026-10-07) |
