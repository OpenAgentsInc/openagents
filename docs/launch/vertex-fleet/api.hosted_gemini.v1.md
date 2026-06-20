# api.hosted_gemini.v1 — launch worklog

State: **yellow** (unchanged — this work does NOT flip any promise state).

Claim: OpenAgents can verify paid Autopilot delegation through a hosted Gemini
closeout bridge in the route harness, but no public paid hosted Gemini inference
product is live.

## What this change builds

A real, reusable **production hosted Gemini executor binding** for the Autopilot
route harness:

- `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-executor.ts`
  — `createHostedGeminiWorkExecutor(config)` returns an `AutopilotWorkExecutor`
  that can be wired to `dependencies.executeReadyWork`. It derives a
  deterministic, public-safe execution closeout from the work projection for a
  `hosted_gemini` placement by driving an **injected** `HostedGeminiInferenceCaller`.
- Tests added to
  `apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts`
  (3 new cases, full file 40 pass): the armed binding delivers a paid hosted
  Gemini work order end-to-end through the route harness (closeout carries the
  model + response-digest proof refs and usage verification ref); the un-armed
  binding stays INERT (no delivery, provider never called); an unsafe inference
  ref aborts delivery instead of leaking.

Prior to this, the only thing that could satisfy `executeReadyWork` for a
`hosted_gemini` placement was a hand-written test fake. This is the genuine
production binding shape the harness was missing.

### Honest / inert by construction

- **Flag-gated, INERT by default**: when `config.enabled` is false the executor
  returns `undefined` — exactly the current production behaviour (no execution,
  no closeout). Wiring it in changes nothing until an operator arms it.
- **No secrets, no raw output**: the injected caller returns public-safe REFS
  only (response digest ref, optional usage ref, model ref). Every emitted ref
  is re-validated with the same public-safe guard the route uses
  (`publicSafeExecutionCloseoutRef`, now exported); any unsafe ref aborts the
  execution rather than leaking it.
- No settlement, no spend, no payout, no implied accepted work.

## Blocker advanced

- `blocker.product_promises.production_hosted_gemini_executor_binding_missing`
  — **partially advanced, NOT cleared.** The reusable production executor
  binding now exists and is proven wireable to the route harness, but it is not
  yet bound in the live worker dependency graph and there is no real deployed
  hosted Gemini inference caller behind it, nor a registered-agent production
  smoke. The blocker therefore REMAINS listed.

### 2026-06-20 update — hosted Gemini gateway metering lane

- `blocker.product_promises.public_paid_model_gateway_missing`
  — **partially advanced, NOT cleared.** Until now the `hosted_gemini` runner
  kind had **no metered pricing lane**: `pricingLaneForRunnerKind('hosted_gemini')`
  returned `null`, so a hosted Gemini placement exposed `activeLane: null` and
  zero pricing reason refs — i.e. no metering/pricing policy for the gateway.
  This change adds the missing buyer-funded, `usd_credits`-metered gateway lane:
  - `apps/openagents.com/workers/api/src/autopilot-work-pricing-policy.ts`
    — new `lane.autopilot_work.hosted_gemini_gateway` lane
    (`buyerDebitRequired: true`, `meterKind: 'usd_credits'`, per-unit charge),
    with `pricing.autopilot_work.hosted_gemini_metered` /
    `placement.reason.hosted_gemini_gateway_metered` reason refs now surfaced in
    hosted Gemini placement decisions.
  - `apps/openagents.com/workers/api/src/autopilot-work-pricing-policy.test.ts`
    (new, 4 cases): asserts the lane is buyer-funded + metered with a positive
    unit charge, that the metering reason refs surface, that every metered lane
    is buyer-funded (and free lanes unmetered), and that lanes are unique per
    runner kind.

  **Honest scope:** this defines the meter only. It does NOT create billing,
  entitlement, quota, provider policy, or settlement, and it is inert until the
  (still-missing) armed executor delivers hosted Gemini work. The gateway
  blocker REMAINS listed.

## What remains (for green)

- A real deployed `HostedGeminiInferenceCaller` (the hosted Gemini inference
  call) wired into the worker dependency graph behind an armed flag.
- A registered-agent production smoke proving a paid hosted Gemini work order
  delivered end-to-end.
- `blocker.product_promises.public_paid_model_gateway_missing` — the hosted
  Gemini metering lane now exists (2026-06-20 update above), but billing,
  entitlement, provider policy, quota, and settlement refs for a public paid
  model gateway still remain.
- Any green flip remains receipt-first and owner-signed per
  `proof.claim_upgrade_receipts.v1`.
