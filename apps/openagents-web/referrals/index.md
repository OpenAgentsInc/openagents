# OpenAgents Referral & Promo Credit Terms (v1)

OpenAgents promotional credit lets developers try hosted OpenAgents inference and reward friends who join. Promo credit is strictly a non-withdrawable, non-transferable service discount. Paid Bitcoin/Lightning credit in satoshis remains separate and is the only money on OpenAgents.

## 1. Credit Amounts

- **GitHub Connect Bonus ($10.00 promo credit):** Connect a GitHub account during onboarding (one-time per numeric GitHub ID).
- **Invited User Bonus ($10.00 promo credit):** Sign up via a referral link (`/r/<code>`) or redeem a code within 7 days of signup, plus eligible GitHub. Stacks with GitHub Connect for $15.00 total.
- **Inviter Bonus ($5.00 promo credit):** Unlocks when your invitee connects an eligible GitHub account and completes hosted inference turns on 2 distinct UTC days within 30 days of redeeming.
- **Lifetime Kickback (1% promo credit):** Whenever an attributed invitee settles a paid Bitcoin/Lightning credit top-up, 1% of the settled USD value (`floor(topup_micros * 100 / 10000)`) is granted as promo credit to the inviter.
- **Inviter Milestones:** One-time promo credit bonuses at 5 activated invitees ($10.00), 25 activated invitees ($50.00), and 100 activated invitees ($250.00).

## 2. GitHub Eligibility

- GitHub account age >= 90 days.
- At least 3 public non-fork repositories, OR 5+ followers, OR 1+ public organization.
- One permanent claim per numeric GitHub ID across OpenAgents. Unlinking or deleting an account does not reset eligibility.

## 3. Spend Order & Expiry

- Applies to promo-eligible models only (`google/gemini-4-neon`, `openagents/router-decision`).
- Spends before paid Bitcoin credit, consuming oldest expiry first.
- Daily spend cap: $5.00 per UTC day; excess falls through to paid Bitcoin credit.
- Expiry: 90 days for bonuses and milestones; 365 days of inactivity for kickback lots.
- Non-withdrawable, non-transferable, no cash value, never usable for payouts.

## 4. Fair Use, Caps & Clawbacks

- No self-referrals, reciprocal referrals, shared GitHub accounts, or multi-account farming.
- Rolling 30-day inviter cap: 20 activated invitees by default.
- Confirmed abuse may result in voiding pending grants and clawing back unspent promo credit only. Paid Bitcoin credit is never clawed back for referral enforcement.
- OpenAgents may pause or modify program parameters or budgets at any time; each grant records its terms version.
