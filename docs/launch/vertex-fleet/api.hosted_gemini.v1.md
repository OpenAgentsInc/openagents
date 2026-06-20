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

## What remains (for green)

- A real deployed `HostedGeminiInferenceCaller` (the hosted Gemini inference
  call) wired into the worker dependency graph behind an armed flag.
- A registered-agent production smoke proving a paid hosted Gemini work order
  delivered end-to-end.
- `blocker.product_promises.public_paid_model_gateway_missing` — billing,
  entitlement, provider policy, quota, metering, and settlement refs for a
  public paid model gateway (untouched by this change).
- Any green flip remains receipt-first and owner-signed per
  `proof.claim_upgrade_receipts.v1`.
