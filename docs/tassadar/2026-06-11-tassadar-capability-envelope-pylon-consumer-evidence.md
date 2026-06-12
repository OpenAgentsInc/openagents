# TassadarCapabilityEnvelope Consumer in Pylon Capability Reporting (W4.1)

> Status: implementation evidence, 2026-06-11. Issue
> `OpenAgentsInc/openagents#4750` (W4 step 1 of
> `docs/tassadar/RESEARCH_PLAN.md`; continuation item 3 of
> `docs/tassadar/2026-06-10-tassadar-percepta-audit.md`, addendum
> 2026-06-11). Serving and pricing claims remain disclosure-gated and
> are NOT touched by this work. Nothing here is registry promise copy.

## What landed

Pylon's executor-class capacity is now advertised with the same
no-overclaim posture as the GEPA capability envelope, consuming the
psionic provider-facing `TassadarCapabilityEnvelope` contract
(psionic `crates/psionic-provider/src/lib.rs`,
`TassadarCapabilityEnvelope::from_executor_capability_publication`:
an unverifiable publication is a typed error, never a declaration):

1. **Self-test-gated declaration (Pylon).** On `pylon provider
   go-online`, the device executes the pinned digest-known workload
   (`tassadar_poc.loop_sum_v1.numeric_fixture.v1`, 80 steps) through
   `@openagentsinc/tassadar-executor` and compares the trace digest
   byte-for-byte against the compile-pinned
   `f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b`.
   Only a verified self-test declares
   `capability.tassadar_poc.numeric_model_executor`, together with the
   minted receipt ref
   `receipt.tassadar_executor.self_test.v1.<digest16>` and the envelope
   profile: window version `window.tassadar_executor.exact_2p53.v1`
   (the checked 2^53 exactness window), legs
   `leg.tassadar_executor.{alm_numeric_execute,exact_trace_replay_full,exact_trace_replay_window}.v1`,
   replay class `exact_trace_replay.alm_numeric_ts.v1`. A failed or
   absent self-test declares nothing and surfaces
   `blocker.pylon.tassadar_executor_self_test_failed`. Stale executor
   claims/receipts are dropped on every go-online, so a device whose
   self-test stops passing also stops advertising. Public-safe evidence
   is written to `PYLON_HOME/tassadar-capability.json`.

2. **Client-side no-overclaim.** `presence register` / `presence
   heartbeat` strip an executor claim that lacks its self-test receipt
   before the request leaves the device
   (`publishableCapabilityRefs`). The 2026-06-10 configuration-asserted
   default (`PYLON_DEFAULT_CAPABILITY_REFS`, commit `2babfb939`)
   remains local config intent only; it can no longer reach the network
   unreceipted.

3. **Worker-side refusal (registration).** `POST /api/pylons/register`
   admits the executor capability only with a receipt ref matching
   `receipt.tassadar_executor.self_test.v1.[0-9a-f]{16}`. An
   unreceipted claim is refused with
   `refusal.public.pylon_capability.tassadar_executor_unreceipted`,
   stripped from the stored registration row, recorded on the
   registration event (`capabilityRefusalRefs`), and reported in the
   response (`tassadarCapabilityAdmission`). Orphaned receipt refs
   without the claim are also dropped.

4. **Dispatch-gate filter (assignment route).** The controlled
   assignment dispatch gate blocks executor-requiring assignments
   against registrations that hold the capability without the receipt:
   `blocker.public.pylon_dispatch.tassadar_capability_unreceipted`.
   This covers legacy rows registered before W4.1 (they stay
   undispatachable for executor work until the device re-runs
   go-online). The Artanis administrator-tick eligibility pre-filter
   applies the same predicate
   (`pylonCapabilityRefsEligibleForExecutorDispatch`), so the mind is
   never offered a candidate the gate would refuse — the ec2e8b281
   discipline extended to capability receipts.

5. **Capability matrix row derives from receipts (E6 intent).** The
   declared profile row
   (`openagents.tassadar_executor.capability_matrix_row.v1`) derives
   from the compile receipt
   (`receipt.tassadar_compile.model_digest.<digest16>`, the pinned
   model digest) and the replay receipt (the self-test receipt ref) —
   never free-form config strings. Shape is enforced twice: a
   dependency-free decoder in the shared package
   (`decodeTassadarCapabilityMatrixRow`, typed
   `TassadarCapabilityShapeError` on any non-receipt-derived field) and
   an Effect Schema on the Worker
   (`TassadarExecutorCapabilityMatrixRow`).

## Where it lives

- `packages/tassadar-executor/src/capability-envelope.ts` — receipt,
  envelope, declaration, matrix row, ref patterns, strip/has helpers
  (exported from the package root; dependency-free, Worker-bundle
  safe).
- `packages/tassadar-executor/src/self-test.ts` — pinned-fixture
  loader + `runPinnedTassadarExecutorSelfTest` (new `./self-test`
  subpath export; `node:fs` stays out of Worker bundles).
- `packages/tassadar-executor/src/lane.ts` — lane constants moved out
  of `index.ts` to break the import cycle.
- `apps/pylon/src/tassadar-capability.ts` — go-online declaration,
  merge, publishable-refs, evidence writer.
- `apps/pylon/src/index.ts` — go-online wiring (self-test before
  declaration, `tassadar` block in the JSON output).
- `apps/pylon/src/presence.ts` — publishable-ref stripping.
- `apps/openagents.com/workers/api/src/tassadar-capability-admission.ts`
  — admission, refusal/blocker refs, dispatch + tick predicates,
  matrix-row schema.
- `apps/openagents.com/workers/api/src/pylon-api-routes.ts` —
  registration admission + dispatch-gate blocker.
- `apps/openagents.com/workers/api/src/artanis-administrator-tick.ts`
  — receipted eligibility pre-filter.

## Live demonstration (production, no spend)

Run 2026-06-11, registry `2026-06-11.7` (Worker-side refusal/gate not
yet deployed; the demo exercises the Pylon-side path against the live
registry):

1. `PYLON_HOME=/private/tmp/pylon-4676-validator-home pylon provider
   go-online` executed the real self-test on the validator device and
   declared:
   `receipt.tassadar_executor.self_test.v1.f2995c4e3c959b42` (the
   first 16 hex of the pinned trace digest), window
   `window.tassadar_executor.exact_2p53.v1`, all three legs, replay
   class `exact_trace_replay.alm_numeric_ts.v1`, matrix row with
   `compileReceiptRef receipt.tassadar_compile.model_digest.3818f73f745992ee`.
2. `pylon presence register --base-url https://openagents.com`
   re-registered `pylon.4f4ef3d029e57674be98` (owner agent token from
   the local ignored env; not printed). Public row
   `GET /api/pylons/pylon.4f4ef3d029e57674be98` now reports
   `capabilityRefs` including both
   `capability.tassadar_poc.numeric_model_executor` and
   `receipt.tassadar_executor.self_test.v1.f2995c4e3c959b42`.
3. Re-registration rebuilt the row, resetting `walletReady`/heartbeat;
   both were restored honestly: a fresh `presence heartbeat`
   (status online) and `pylon wallet report-readiness` backed by a real
   MDK classification of the validator's funded wallet (313 sats,
   `walletReady: true`, `walletRef wallet.public.mdk.fb6119816c1b0a38b2a04e04`,
   plus a tip-readiness claim minted by the standard #4712 path).
4. `pylon.24819249b4634a4c9d5e` (the #4675 worker device, in active use
   by the in-flight #4678 lane) was deliberately left untouched; its
   row still carries the bare unreceipted capability and is exactly the
   legacy case the new dispatch-gate blocker holds at bay post-deploy.

## Verification

- `packages/tassadar-executor`: `bun test` — 9 pass (5 new envelope
  tests incl. forged-pin refusal and free-form-row rejection);
  `tsc --noEmit` clean.
- `apps/pylon`: `bun test` — 440 pass, 3 skip (5 new
  `tassadar-capability` tests: real self-test declaration, failing
  self-test refusal + blocker, stale-claim drop, presence strip,
  evidence file).
- `workers/api`: `bunx vitest run` on
  `pylon-api-routes.test.ts` (22 pass; 3 new: unreceipted refusal at
  registration, receipted admission, legacy-row dispatch blocker),
  `tassadar-capability-admission.test.ts` (7 pass),
  `tassadar-executor-trace-homework.test.ts` +
  `tassadar-replay-validator.test.ts` +
  `pylon-gepa-metric-call-assignments.test.ts` (15 pass).
- `bun run typecheck` (apps/openagents.com, all projects),
  `bun run check:architecture`, `bun run check:effect-topology`,
  `wrangler deploy --dry-run` — all pass.

## Named remainders

- **OpenAPI entry skipped:** the registration response now carries
  `tassadarCapabilityAdmission` and the dispatch gate can emit
  `blocker.public.pylon_dispatch.tassadar_capability_unreceipted`;
  `openagents-openapi.ts` was locked by in-flight #4678 work, so the
  OpenAPI documentation entry for both is deliberately not included
  here and owes a follow-up edit.
- **Worker deploy:** the refusal/gate code is committed but not
  deployed; the live dispatch-gate filter check against production
  becomes possible only after the next registry deploy.
- **Worker-device migration:** `pylon.24819249b4634a4c9d5e` should
  re-run `provider go-online` + `presence register` after deploy to
  trade its configuration-asserted capability for the receipted one.
- **Heartbeat capability refresh:** the Worker heartbeat schema does
  not carry capabilityRefs, so receipt rotation lands only via
  re-registration; if heartbeats ever update capabilities, the same
  admission must be applied there.
