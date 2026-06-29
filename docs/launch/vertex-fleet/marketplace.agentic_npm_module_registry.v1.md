# marketplace.agentic_npm_module_registry.v1 — vertex-fleet note

Promise: `marketplace.agentic_npm_module_registry.v1` (state: **planned** — unchanged).

## What this change advances

Blocker: `blocker.product_promises.agentic_npm_module_composition_runtime_missing`.

Episode 238 ("learning by construction", `docs/transcripts/238.md`) frames verified
programs becoming **composable modules** in an "agentic npm" registry. The
composition-runtime blocker says there is no runtime that assembles registry
modules into a composed program with verification-on-compose.

This change builds the smallest genuine missing piece: the composition-runtime
**core** — a pure, deterministic dependency resolver + verification-on-compose gate.

## What was built

- `apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.ts`
  - `resolveAgenticNpmComposition({ registry, requestedRootRefs })` resolves the
    transitive dependency closure (`dependsOn` edges), gates every resolved module
    on its exact-trace verification flags (`replayVerified` +
    `compositionVerified` + `linkCompatibilityVerified` — the same gate reused from
    `compute.tassadar_executor_poc.v1`), checks that every `requiresInterfaces`
    capability is provided within the resolved closure, detects missing modules and
    dependency cycles (Kahn topological sort), and emits a deterministic,
    content-addressed `AgenticNpmCompositionPlan` (sha256 `planDigest`).
  - It is **PURE / INERT**: it installs nothing, executes nothing, provisions no
    primitive, moves no money, reads no wallet, writes no receipt, meters nothing,
    and settles nothing. The `authority` block is all-`false`; `promiseState` is
    pinned to `planned`; refs pass a public-safe guard (no credential/wallet/
    payment/private material).
- `apps/openagents.com/workers/api/src/agentic-npm-composition-runtime.test.ts`
  - 8 tests: verified-DAG topological resolution, input-order determinism,
    verification-on-compose gating, missing-module, cycle detection, unsatisfied
    interface, satisfied interface, and unsafe-ref rejection.

## What genuinely remains (blocker NOT cleared)

The composition-runtime blocker stays **listed** — this builds the resolve+verify
core only. Still missing for the runtime: a live install/uninstall lifecycle,
actual module fetch/pin from a live registry, sandboxed execution of a composed
plan, and metering. Separate blockers remain untouched:
`agentic_npm_registry_not_live` (registry liveness) and
`agentic_npm_billing_settlement_missing` (billing/settlement). Any green flip
stays receipt-first and owner-signed per `proof.claim_upgrade_receipts.v1`.
