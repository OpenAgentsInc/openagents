# autopilot_sites.partner_payout_ledger.v1 — partner-attribution policy

**Promise state: red (unchanged — no flip).**

## Blocker advanced

`blocker.product_promises.partner_attribution_policy_missing`

The promise safeCopy named "a partner-attribution policy distinct from the
referral feed" as part product decision, part code. The payout *ledger*
(`partner-payout-ledger.ts`) already computes amounts and walks the lifecycle,
and the referral feed (`site-referral-payout-feed.ts`) *infers* its earner from
last-touch click attribution. What was missing was the decision that runs BEFORE
eligibility for the larger design-partner / affiliate payouts: which partner (if
any) a paid event is attributed to, and whether that attribution is allowed.

## What was built

- `apps/openagents.com/workers/api/src/partner-attribution-policy.ts` — a pure,
  side-effect-free `decidePartnerAttribution(event, candidateAgreements)`
  returning one of `attributed` / `no_active_agreement` / `self_attribution`.
  Product rules encoded (conservative, **owner sign-off still pending**):
  1. **Explicit-agreement-only** — no partner is credited without an active,
     customer-covering partner agreement; there is no last-touch inference and
     no inferred fallback (the distinction from the referral feed).
  2. **Referral exclusion** — the `referral` role is owned by the referral rail
     and refused here, so the same revenue is never double-paid across rails.
  3. **Role precedence** — design_partner > affiliate; exactly one partner is
     credited, earliest-effective agreement as a deterministic tie-break.
  4. **Active window** — `effectiveFrom <= event < effectiveUntil`.
  5. **Self-payout exclusion** — a partner cannot earn on their own purchase.
- `apps/openagents.com/workers/api/src/partner-attribution-policy.test.ts` —
  8 tests covering each rule (no-fallback, attribution, referral exclusion,
  precedence, tie-break, pre/post-window, self-payout).

## What genuinely remains (blocker NOT fully cleared — left listed)

- **Owner sign-off** on the payout percentages/caps in
  `PARTNER_PAYOUT_ROLE_POLICY` and on this attribution model (caveat
  `caveat.public.partner_payouts.partner_policy_not_owner_signed`).
- **Feed wiring**: a storage-backed reader that loads candidate
  `PartnerAgreement`s and maps an `attributed` decision onto
  `createPartnerPayoutEligibility` (mirrors `site-referral-payout-feed.ts`).
  Kept out of scope here so the product rules stay independently testable; the
  decision is pure on purpose.
- `blocker.product_promises.partner_payout_settlement_not_wired` and
  `blocker.product_promises.partner_first_real_payout_pending` are untouched.

No promise state, registry green/yellow field, or state-transition field was
changed by this work.
