# Tassadar Executor — Pylon v0.3 Release Readiness Audit

Date: 2026-06-10

Registry version at audit time: `2026-06-10.12`
(`compute.tassadar_executor_poc.v1` green, transition receipt
`promise_transition_99b561e9-74f1-4c9a-90cc-cd7c0aea13bd`).

Question under audit: **is the Tassadar executor-trace lane poised to be
part of the Pylon v0.3 release, and what — if anything — is still needed
for inclusion?**

Short answer: **yes, it is poised — the execution code already lives in
the v0.3 release-candidate source and the v0.3 release gate passes with
it as of this audit** (after one packaging fix made during the audit,
recorded below). Inclusion needs five concrete items, none of them
architectural: the npm publish story it shares with `@openagents/nip90`,
a default capability declaration, packaged-network smoke coverage, one
admission-hardening nit, and a copy-discipline line in the launch gates.

## 1. What "in the release" would mean

`@openagentsinc/pylon@0.3.0-rc1` publishes `src/` plus the runtime
package; the `pylon` binary is `src/index.ts`. The Tassadar executor
lane touches the package in exactly two ways:

- `src/assignment.ts` carries the executor-trace gate
  (`executeTassadarAssignment`): recognize the dispatched workload,
  execute it through `@openagents/tassadar-executor`, carry the trace
  digest into closeout refs, reject digest mismatches with a typed
  blocker, surface typed execution refusals. This file ships in the
  published `files` list today — **the lane is in the RC source by
  default, not by addition.**
- `package.json` depends on `@openagents/tassadar-executor:
  workspace:*` — the shared TypeScript executor whose load-bearing test
  reproduces the psionic Rust executor's trace digest byte-for-byte on
  the committed fixture.

Everything else the proof of concept used is server-side (assignment
job kind, operator dispatch script, the worker-as-validator replay
route, the training-verification challenge lifecycle) and is already
deployed in production. The Pylon package needs none of it at runtime;
workloads arrive embedded in the assignment payload.

## 2. Evidence that the v0.3 surface already works live

The green PoC ran on this RC source against production
(epic #4687, steps #4689–#4694):

- a real registered Pylon polled, accepted, executed, and closed out a
  live-dispatched executor-trace assignment with
  `artifact.tassadar_poc.trace_digest.f2995c4e…` — byte-identical to the
  psionic Rust fixture digest (closeout
  `assignment.closeout.7e7ebbf204c7b7688d07af55`);
- the production worker re-executed the workload as a separate validator
  device and produced a Verified `exact_trace_replay` challenge receipt
  (`training.verification.challenge.81760553-…`) plus a Rejected receipt
  on a tampered digest;
- one operator-funded paid closeout settled over real Lightning to the
  Pylon's admitted payout target (1,000 sats; payer 2,173→1,173,
  receiver 0→980; payment completed, preimage held privately).

310 Pylon tests pass, including the five executor-trace gate tests
(digest match, digest-mismatch rejection, non-tassadar passthrough,
typed refusal, transit-shape restoration).

## 3. The release gate: failed, fixed, passing

The decisive readiness check was running the v0.3 release gate
(`bun run release:gate`) on current `main`. **It failed** at the local
package install smoke: the packed Pylon could not resolve
`@openagents/tassadar-executor` (404 — not on npm, and the smoke's
override table only covered `@openagents/nip90`). This was a real
release blocker introduced by the PoC dependency.

Fixed during this audit, following the established nip90 pattern:

- `packages/tassadar-executor/package.json` is now publishable
  (`private: false`, `publishConfig.access: public`; `files` already
  includes `src` and `fixtures`);
- `apps/pylon/scripts/smoke-local-package-install.sh` packs the executor
  tarball and adds it to the install-smoke `overrides` alongside nip90.

With those two changes the full release gate passes: unit/runtime tests,
bootstrap/status/inventory/operator smokes, dashboard smoke, package
dry-run, and the local package install smoke.

## 4. What is still needed for inclusion

In order of hardness:

1. **The npm publish story (shared with nip90; blocking the actual
   publish, not the gate).** The published v0.3 package declares
   `workspace:*` dependencies on `@openagents/nip90` and now
   `@openagents/tassadar-executor`; neither is on npm. A consumer
   installing from the registry would hit the same 404 the gate caught.
   The release must either (a) publish both packages to npm first —
   both now carry public publishConfig — or (b) rewrite/vendor the
   dependencies at publish time. This question predates Tassadar (nip90
   poses it already) and belongs to the CI/release-gate issue (#4654);
   Tassadar simply joins the existing answer.
2. **Default capability declaration.** Dispatch is gated on the
   registration carrying `capability.tassadar_poc.numeric_model_executor`;
   in the PoC this required manual config (`--capability-ref` /
   `config.capabilityRefs`). Since the executor genuinely ships inside
   every v0.3 package, the capability is true by construction and should
   be auto-declared in the packaged defaults (one-line change in the
   bootstrap/runtime capability defaults, mirroring how go-online
   auto-adds the NIP-90 provider and labor capabilities). Without this,
   packaged contributors are silently dispatch-ineligible.
3. **Packaged-network smoke coverage (#4656).** The live v0.3 network
   smoke from the packaged binary should gain an executor-trace leg
   (no-spend dispatch → poll → execute → closeout with digest), so the
   release proof exercises this lane from the artifact users actually
   install, not from the repo checkout. The PoC dispatch script and the
   worker route make this a small addition.
4. **Admission hardening (nit).** The PoC lease carried empty
   `requiredCapabilityRefs` inside the codingAssignment payload, so any
   Pylon would admit it; the capability gate was enforced only at
   operator dispatch. For release, the dispatch script should mirror the
   capability ref into the lease payload so Pylon-side admission
   (`hasRequiredCapabilities`) also enforces it.
5. **Copy discipline in the launch gates.** The launch-gates doc's
   blocked-copy list ("Paid Pylon work settles Bitcoin", "Pylons sell
   compute capacity live") stays blocked as general copy. The only
   executor copy the release may carry is the promise's scoped safeCopy
   (one workload family, one Pylon, dated, receipts cited). Recommend
   adding that line explicitly to `launch-gates-no-overclaim.md` as the
   allowed scoped claim so release notes cannot drift into
   generalization.

Two non-blocking follow-ups worth tracking with the lane rather than the
release: unify the two job-kind constants (the pylon gate matches both
`tassadar_executor_trace` and `tassadar_executor_trace_homework`; one
shared constant — importable from the executor package — removes the
drift class that caused the PoC's cancelled first closeout), and the
registration-ownership gap (payment-receipt/settlement-status event
posts require the registration's owning agent token; packaged
contributors who register with `OPENAGENTS_AGENT_TOKEN` set are
unaffected, but onboarding docs should say so).

## 5. Verdict

The lane is **release-poised**: the execution path is in the RC source,
tested, live-proven against production, and the release gate is green
with the packaging fix from this audit. It is **not yet
release-complete**: items 1–3 above are required before
`@openagentsinc/pylon@0.3.0` can honestly include the lane (item 1 being
a pre-existing v0.3 question Tassadar inherits rather than creates), and
items 4–5 are cheap hardening that should land with them. Nothing about
the lane requires changing what v0.3 *is* — it slots into the existing
assignment, capability, smoke, and no-overclaim machinery exactly as
those systems were designed to absorb a new work class.

## Source refs

- `apps/pylon/src/assignment.ts` (executor-trace gate)
- `apps/pylon/tests/tassadar-assignment.test.ts`
- `packages/tassadar-executor/` (executor, fixtures, replay CLI)
- `apps/pylon/scripts/release-gate.sh`,
  `apps/pylon/scripts/smoke-local-package-install.sh`
- `apps/pylon/docs/launch-gates-no-overclaim.md`,
  `apps/pylon/docs/release-install-smokes.md`
- `apps/openagents.com/workers/api/src/tassadar-executor-trace-homework.ts`,
  `tassadar-replay-validator.ts`, `scripts/tassadar-poc-dispatch.ts`
- Issues: #4687 (epic), #4689–#4694 (sequence), #4654/#4655/#4656
  (release cluster)
- Promise: `compute.tassadar_executor_poc.v1` (green, 2026-06-10.12)
