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
- `apps/openagents.com/workers/api/src/partner-attribution-eligibility.ts` —
  the **pure decision -> ledger bridge** that was the documented next step. It
  takes a `PartnerQualifyingPaidEvent` plus candidate agreements, runs
  `decidePartnerAttribution`, and maps an `attributed` decision onto a
  ledger-ready `CreatePartnerPayoutEligibilityInput`
  (`resolvePartnerPayoutEligibilityInput`). `no_active_agreement` /
  `self_attribution` decisions return a skip branch that records NOTHING — the
  same short-circuit shape as `site-referral-payout-feed.ts` but with no
  last-touch inference. The paying customer is carried as `beneficiaryUserId`
  for a defense-in-depth self-payout guard, and the `agreementRef`/`policyRef`
  are surfaced for the caller to record the attribution basis. Still pure: no
  DB read, no money movement.
- `apps/openagents.com/workers/api/src/partner-attribution-eligibility.test.ts`
  — 6 tests (no-agreement skip, eligibility-input mapping, asset/amount/period
  passthrough, role precedence, self-attribution skip, expired-window skip).

### Follow-up (this run): the storage-backed feed is now built

The previously-deferred "storage-backed reader" step is now implemented as the
partner-rail analogue of `recordReferralPayoutForPaidEvent`:

- `apps/openagents.com/workers/api/migrations/0214_partner_agreements.sql` —
  the `partner_agreements` table backing the policy: EXPLICIT, currently-active
  agreements naming `customer_user_id`, `partner_ref`/`partner_user_id`, and
  `role`. The `role` CHECK excludes `referral` (owned by the referral rail), so
  cross-rail double-pay is unstorable. No payout destinations, invoices,
  preimages, or provider payloads live here.
- `apps/openagents.com/workers/api/src/partner-payout-feed.ts` —
  `readActivePartnerAgreementsForCustomer(db, customerUserId)` (D1-backed,
  read-only, bounded, safe-id-guarded) plus
  `recordPartnerPayoutForPaidEvent(db, event, deps?)`: reads the customer's
  explicit candidates, runs `resolvePartnerPayoutEligibilityInput`, and on an
  `eligible` decision calls `createPartnerPayoutEligibility` exactly once
  (idempotent on the event key). `no_active_agreement` / `self_attribution`
  record NOTHING — no inferred fallback. The reader and ledger writer are
  injectable so the rules stay testable without a live D1.
- `apps/openagents.com/workers/api/src/partner-payout-feed.test.ts` — 7 tests
  (no-agreement skip, eligibility mapping, design_partner>affiliate precedence,
  self-attribution skip, expired-window skip, default-reader query/binding map,
  malformed-id no-query guard).

### Follow-up (this run): the attribution basis is now persisted on the row

The previously-deferred "recording the attribution basis on the ledger row" step
is now built. The feed already surfaced the winning `agreementRef`/`policyRef`,
but the ledger row did not record them — so a credited partner payout could not
be audited back to the explicit agreement that authorised it.

- `partner-payout-ledger.ts` — `CreatePartnerPayoutEligibilityInput` now accepts
  optional public-safe `evidenceRefs` and `policyRefs`. They are validated with
  the same `assertSafeRefs` ref discipline (provider-secret / prohibited-pattern
  rejection), merged AFTER the required refs, and order-preservingly
  de-duplicated, so `PARTNER_PAYOUT_POLICY_REF` and the qualifying event ref are
  always present and never doubled.
- `partner-attribution-eligibility.ts` — the bridge now passes the winning
  `agreementRef` as `evidenceRefs` and the attribution `policyRef` as
  `policyRefs`, so every credited partner payout row names the explicit
  agreement + policy that authorised it (the audit distinction from the
  inferred-click referral rail is now persisted, not just computed).
- Tests: `partner-payout-ledger.test.ts` (+2: dedupe/required-ref merge, unsafe
  evidence-ref rejection), `partner-attribution-eligibility.test.ts` (+1: basis
  persisted; exact-match input updated), `partner-payout-feed.test.ts` (recorded
  input carries the basis). 27 tests pass across the three files.

### Follow-up (this run): the agreement WRITER + write-boundary guard is now built

The feed shipped a `partner_agreements` reader but no validated WRITER, so the
only way to seed the rows the policy depends on was a raw SQL insert that nothing
checked. A self-agreement, an inverted effective window, or a `referral`-role row
could land in storage and later be read back and credited. Now:

- `partner-attribution-policy.ts` — `assessPartnerAgreementSeed(seed)`, a pure
  validator that applies the SAME attribution invariants at the WRITE boundary
  that `decidePartnerAttribution` applies at read time: role attributability
  (the `referral` exclusion), self-agreement exclusion
  (`partnerUserId !== customerUserId`), and effective-window consistency
  (parseable start; end open-ended or strictly after start). Returns
  `seedable`/`rejected`; throws nothing (policy stays pure).
- `partner-payout-feed.ts` — `recordPartnerAgreement(db, input)`, the sanctioned
  writer: it rejects non-public-safe refs/ids, runs `assessPartnerAgreementSeed`,
  and only then `INSERT OR IGNORE`s into `partner_agreements` (idempotent on
  `agreementRef`, read-back verified). A policy-violating agreement is now
  unstorable, not just unread. New `PartnerAgreementValidationError`; DB faults
  wrap into the existing `PartnerPayoutLedgerStorageError`.
- Tests: `partner-attribution-policy.test.ts` (+5: seedable, referral-rejected,
  self-rejected, inverted-window, bad-iso), `partner-payout-feed.test.ts` (+5:
  seed+readback, idempotent no-second-insert, referral/self/unsafe-ref rejection
  before storage). 25 tests pass across the two files.

### Follow-up (this run): the agreement writer now has an operator HTTP surface

The writer (`recordPartnerAgreement`) and reader
(`readActivePartnerAgreementsForCustomer`) shipped and were tested at the storage
layer, but neither had an HTTP caller — so the only way to create the EXPLICIT
`partner_agreements` rows the attribution policy depends on was a raw SQL insert,
and operators had no sanctioned path to seed one. Without an agreement, no
partner is ever attributed (the no-fallback rule), so this was the load-bearing
call-site gap on the code side of the blocker.

- `apps/openagents.com/workers/api/src/partner-agreement-routes.ts` — an
  admin-gated route module modeled on `partner-payout-ledger-routes.ts`:
  - `POST /api/operator/partners/agreements` decodes a seed and calls
    `recordPartnerAgreement` (idempotent on `agreementRef`; a policy-violating
    seed — referral role, self-agreement, inverted window — is a 422 carrying the
    writer's reason; the writer remains the single source of truth for the
    attribution invariants). `role` accepts the full `PartnerPayoutRole` set so a
    `referral` seed yields the informative 422, not a generic schema error.
  - `GET /api/operator/partners/agreements?customerUserId=<id>` calls
    `readActivePartnerAgreementsForCustomer` for read-back verification.
  - Returns an operator projection that omits `customerUserId` (mirrors the
    `PartnerAgreement` DTO). This is NOT the public count-only projection surface.
  - Like the ledger routes, it does NOT touch `index.ts` / `worker-routes.ts`;
    coordinator wiring notes are in the module header.
- `apps/openagents.com/workers/api/src/partner-agreement-routes.test.ts` — 12
  tests (seed+project, idempotent replay, referral/self/inverted-window 422,
  malformed-body 400, create/list 401, list-by-customer, missing-query 400,
  route passthrough, 405 on unsupported method).

The agreement WRITER is now reachable end-to-end; `recordPartnerPayoutForPaidEvent`
(the paid-event feed) still awaits a production caller (see below).

### Follow-up (this run): the agreement routes are now SERVED by the worker

The previous run shipped `partner-agreement-routes.ts` with a
`makePartnerAgreementRoutes` factory but, like the ledger routes at the time,
left it unchained — so the operator endpoints existed in source but `index.ts`
never mounted them and no real request could reach the sanctioned agreement
WRITER. (The ledger routes have since been wired at `index.ts`; the agreement
routes were the last partner-rail module still dark.) Without a served writer,
the only way to seed the EXPLICIT `partner_agreements` rows the attribution
policy depends on remained a raw SQL insert, and the no-fallback rule meant no
partner could ever be attributed. This run closes that call-site gap:

- `apps/openagents.com/workers/api/src/index.ts` — three minimal edits mirroring
  the `partnerPayoutLedgerRoutes` wiring exactly:
  1. `import { makePartnerAgreementRoutes } from './partner-agreement-routes'`.
  2. construct `partnerAgreementRoutes` with the same
     `nowIso: currentIsoTimestamp` / `requireAdminApiToken` dependencies used by
     the ledger routes.
  3. chain `partnerAgreementRoutes.routePartnerAgreementRequest(request, env, ctx)`
     onto the `routeOmniRequest` `??` chain, immediately after the partner payout
     ledger route. The route returns `Effect | undefined`, so a non-matching path
     falls through to the rest of the chain unchanged.
- No new behaviour, schema, or money movement is introduced — this is pure
  call-site integration. `GET/POST /api/operator/partners/agreements` are now
  admin-gated and live. Migration `0214_partner_agreements.sql` must be applied
  before serving (already present in `migrations/`).
- `bunx tsc -p tsconfig.json --noEmit` reports 0 errors after the wiring.

### Follow-up (this run): the paid-event feed now has a PRODUCTION caller

The last load-bearing code gap on this blocker was that `recordPartnerPayoutForPaidEvent`
(the paid-event feed) had no production caller — every test exercised it with
injected deps, but no real payment path invoked it, so an explicit active
agreement could never actually mint a partner payout. The referral feed was
already wired into the real Stripe checkout-paid path
(`stripe-billing.ts:recordReferralPayoutForPaidEvent`); the partner feed was not.
This run wires it alongside, mirroring the referral pattern exactly:

- `apps/openagents.com/workers/api/src/stripe-billing.ts` —
  - `buildStripeCheckoutPartnerPayoutEvent(...)`, an exported PURE builder that
    maps a fulfilled USD credit checkout onto a `PartnerQualifyingPaidEvent`
    (`asset: 'usd'`, `qualifyingAmount = pack.amountCents`, payer as
    `customerUserId`, deterministic per-session `idempotencyKey`). Extracted as a
    pure function so the Stripe -> partner-rail contract is testable without a
    live Stripe client or D1.
  - The fulfillment path now calls `recordPartnerPayoutForPaidEvent(db, event)`
    in its own best-effort `try/catch`, right after the referral feed call. It is
    non-authoritative for billing (a feed failure never blocks fulfillment) and
    idempotent per checkout session. Because the qualifying amount is the USD
    purchase value, any eligibility is a USD-asset row whose role percentage
    never mints a withdrawable-Bitcoin liability; whether ANY partner is credited
    is still decided downstream by the no-fallback attribution policy (an EXPLICIT
    active agreement must cover the buyer), so the common no-agreement case
    records nothing.
- `apps/openagents.com/workers/api/src/partner-payout-stripe-wire.test.ts` — 5
  tests: builder maps a USD checkout exactly, deterministic idempotency key, and
  the produced event run through the feed yields no_active_agreement (no
  agreement), a USD eligibility (covering agreement), and self_attribution skip
  (buyer is the partner).

With this, the partner rail is reachable end-to-end from a real payment:
operator seeds an agreement (`POST /api/operator/partners/agreements`) -> a real
Stripe credit purchase by a covered customer mints an `eligible` partner payout
row -> operators walk it through approve/dispatch/settle.

## What genuinely remains (blocker NOT fully cleared — left listed)

- **Owner sign-off** on the payout percentages/caps in
  `PARTNER_PAYOUT_ROLE_POLICY` and on this attribution model (caveat
  `caveat.public.partner_payouts.partner_policy_not_owner_signed`). This is a
  product decision and is the load-bearing remainder of this blocker.
- **Call-site wiring**: both `recordPartnerAgreement` (admin routes chained into
  `index.ts`) AND `recordPartnerPayoutForPaidEvent` (now invoked from the real
  Stripe checkout-paid path in `stripe-billing.ts`, see the follow-up above) have
  production callers. The remaining wiring nuance: only the Stripe credit
  CHECKOUT path is fed; the auto-top-up credit path is not (matching the referral
  feed, which is likewise checkout-only). Extending coverage to auto-top-up — and
  to any non-Stripe Bitcoin revenue event — is a follow-up, not a blocker for the
  first real partner payout.
- `blocker.product_promises.partner_payout_settlement_not_wired` is cleared by
  the source-level dispatch coordinator.
  `blocker.product_promises.partner_first_real_payout_pending` remains until an
  owner-armed production payout settles with a public-safe receipt.

No promise state, registry green/yellow field, or state-transition field was
changed by this work.
