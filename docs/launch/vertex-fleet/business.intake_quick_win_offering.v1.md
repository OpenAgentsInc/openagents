# business.intake_quick_win_offering.v1 — vertex-fleet note

Promise state: **yellow** (unchanged — no green flip in this change).

## What this change builds

A canonical, typed **business quick-win receipt** contract that the registry's
green bar for this promise depends on. The promise's verification text says
green requires "a self-serve quick-win delivery loop and at least one
dereferenceable first paid business quick-win receipt (intake -> delivery ->
accepted outcome -> receipt)". Until now there was no defined shape for that
receipt and no machine-checkable bar for "first paid quick-win receipt".

New files (under `apps/openagents.com/workers/api/src/`):

- `business-quick-win-receipt.ts` — Effect-Schema lifecycle receipt with one
  line per state (`intake_recorded` -> `quick_win_scoped` ->
  `delivered_with_evidence` -> `outcome_accepted` -> `buyer_paid` ->
  `provider_settled`). The builder is pure/deterministic, evidences a state only
  when a concrete reference is supplied, flags settlement-implying states, and
  rejects any receipt that evidences a state while an earlier prerequisite is
  unevidenced (you cannot be paid for an outcome that was never delivered).
  `assertFirstPaidQuickWinReceipt` encodes the exact bar a future green flip
  must clear (`REQUIRED_PAID_QUICK_WIN_STATES`).
- `business-quick-win-receipt.test.ts` — 16 tests covering lifecycle ordering,
  the honesty invariant, the paid-receipt gate, and the public projection.

This composes with the existing live intake
(`business-signup-routes.ts`): a receipt's `signupId` ties a quick win back to a
recorded `/business` intake (`BusinessSignupRecord.id`).

## Which blocker this advances

`blocker.product_promises.business_first_paid_quick_win_receipt_missing` —
**advanced, NOT cleared.** The receipt format and its verifier now exist and are
tested, so a real `intake -> delivery -> accepted outcome -> receipt` run has a
concrete, honest target to emit. The blocker stays listed because no real paid
quick win has produced a dereferenceable receipt yet (no money has moved; the
paid credits/settlement loops it would reference are themselves not collectable
end-to-end — see `inference.gateway_credits_business.v1` and
`payments.autopilot_credits_purchase.v1`).

## Update 2026-06-20 — intake -> quick-win scope router

Advances `blocker.product_promises.business_quick_win_self_serve_delivery_missing`
(**advanced, NOT cleared**) by building the first automated segment of a
self-serve delivery loop: the step that today an operator does by hand — reading
a `/business` intake's free-text "what do you need help with" and deciding which
menu offering backs it and what "done" means.

New files (under `apps/openagents.com/workers/api/src/`):

- `business-quick-win-scope.ts` — pure/deterministic `scopeQuickWinFromIntake`
  that routes a `BusinessSignupRecord` (`signupId` + `helpWith`) to a backing
  offering promiseId, the menu availability, a delivery mode, and a
  definition-of-done checklist. It is honest by construction: NO route emits
  `self_serve` (the `self_serve` literal exists only so closing the blocker is a
  route-level data change with a verifier, not a rewrite), `needsOperator` is
  always true today, roadmap offerings route to `not_deliverable`, and an intake
  that matches nothing routes to `unmatched_operator_triage` with an open
  question instead of being force-fit. Its `quickWinScopedRef` feeds
  `buildBusinessQuickWinReceipt`'s `quick_win_scoped` line, joining the two
  pieces into one intake -> scope -> receipt chain.
- `business-quick-win-scope.test.ts` — 13 tests covering per-category routing,
  specificity ordering (batch before generic inference), the unmatched/blank
  fallback, the self-serve honesty invariant, determinism, and the
  scope->receipt handoff.

## What genuinely remains

- The rest of the self-serve delivery loop after scoping: actually driving a
  scoped quick win through delivery + acceptance WITHOUT an operator. This change
  automates only routing; `needsOperator` stays true for every route. Closing
  `business_quick_win_self_serve_delivery_missing` means flipping at least one
  route to `self_serve` backed by a proven hands-off delivery path.
- Persistence + a public route for scopes and receipts (both modules are pure
  contracts only; no D1 table or HTTP surface was added).
- A real first paid quick win that produces a receipt passing
  `assertFirstPaidQuickWinReceipt`
  (`blocker.product_promises.business_first_paid_quick_win_receipt_missing`),
  then a receipt-first upgrade per `proof.claim_upgrade_receipts.v1` before any
  green flip.
