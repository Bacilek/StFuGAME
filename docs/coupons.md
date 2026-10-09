# Coupon codes

User 2026-10-09: a player gave a code (`N3UTR4L-EU31`: best mount for 14 days, Life potion, 100 lucky coins). The user wants it
on every character; chose full automation (precedents.md).

## How the game does it (captured from a browser Network tab, characters Mimimimi11, Pagan, Sanek)
1. Mushroom Dealer → Mushrooms → Redeem Coupon → "give me the bonus!" = **POST `https://coupon.playa-games.com/redeem`**,
   `application/x-www-form-urlencoded`: `coupon=<code>`, `paymentstring=<player id>_<n0>_<n68>_1`, `lang=en`. Answer: JSON `{"status":"ok"}`.
   Not a game command (`cmd.php`), no auth header seen.
2. The game's next `Poll` carries the new Mail entry: `pendingrewards.r:<msg id>/0/10/<CODE WITHOUT DASH>/<from>/<until>` (status 0 = unread,
   type 10 = Coupon; valid 7 days). Clicking it = `PendingRewardView:<msg id>` (status 1, `pendingrewardressources:5/100/33/14`,
   `pendingrewarditems:…` = 100 lucky coins (5), mount 14 days (33/14), a potion item), "Claim" = `PendingRewardClaim:<msg id>` (status 2).
3. `paymentstring`: player id (`ownplayersavecharacter[1]`), `ownplayersavecharacter[0]` (sf-api drops it, "secret id?"),
   `ownplayersavecharacter[len-2]` (562 on s31, same for every character) and a constant `1` (two samples agree:
   `27375_339294320_562_1`, `27812_7646100_562_1`).

## Implementation (src/coupons.rs, called from `daily::run`, at most every 30 min)
- Codes: `roster/coupons.txt` (one per line, `#` comments; local, gitignored). Progress per character:
  `roster/<character>/coupons.txt` (`code<TAB>redeemed|claimed|rejected:<status>`); delete a line to retry.
- `SimpleSession::char_save()` keeps the raw `ownplayersavecharacter` numbers of the latest response/login.
- Redeem once per code and character; a network/parse error is retried later, a `status` other than `ok` is final (`rejected:…`).
- Claim: every unclaimed Coupon-type Mail entry whose name matches a code (also for a code redeemed by hand earlier) → `ClaimablePreview` then
  `ClaimableClaim`. `safe.rs` allows both only for a Coupon-type mail that is in our own `gs.mail.claimables`. No mushrooms spent.
- Unit tests: `payment_string` against both captured samples.

## Verification status
| What | Status |
|---|---|
| POST format and `status: ok` | ✅ captured by the user on 3 characters |
| `paymentstring` composition | ✅ two samples; the trailing `1` assumed constant |
| Bot's own POST accepted by the service | ⏳ not run yet (needs a release build) |
| Claim via `ClaimablePreview` + `ClaimableClaim` | ⏳ not run yet (format captured from the browser) |
| What happens with a full backpack/stable (mount, potion item) | ⏳ unknown |
| Characters that already redeemed by hand get `rejected:<status>` | ⏳ status text unknown |
