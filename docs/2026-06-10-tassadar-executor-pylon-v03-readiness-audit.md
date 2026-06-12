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
architectural: the npm publish story it shares with `@openagentsinc/nip90`,
a default capability declaration, packaged-network smoke coverage, one
admission-hardening nit, and a copy-discipline line in the launch gates.

## 1. What "in the release" would mean

`@openagentsinc/pylon@0.3.0-rc1` publishes `src/` plus the runtime
package; the `pylon` binary is `src/index.ts`. The Tassadar executor
lane touches the package in exactly two ways:

- `src/assignment.ts` carries the executor-trace gate
  (`executeTassadarAssignment`): recognize the dispatched workload,
  execute it through `@openagentsinc/tassadar-executor`, carry the trace
  digest into closeout refs, reject digest mismatches with a typed
  blocker, surface typed execution refusals. This file ships in the
  published `files` list today — **the lane is in the RC source by
  default, not by addition.**
- `package.json` depends on `@openagentsinc/tassadar-executor:
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
`@openagentsinc/tassadar-executor` (404 — not on npm, and the smoke's
override table only covered `@openagentsinc/nip90`). This was a real
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

Resolution note (2026-06-10, #4696): items 2-5 below landed after this audit.
Pylon bootstrap/runtime defaults now auto-declare
`capability.tassadar_poc.numeric_model_executor`, the packaged-network smoke
packs and replays `@openagentsinc/tassadar-executor`, the PoC dispatch script
mirrors the capability ref into the embedded coding assignment, and
`apps/pylon/docs/launch-gates-no-overclaim.md` carries the only allowed scoped
executor copy. Item 1 remains the external publish mechanism tracked by #4654.

In order of hardness:

1. **The npm publish story (shared with nip90; blocking the actual
   publish, not the gate).** The published v0.3 package declares
   `workspace:*` dependencies on `@openagentsinc/nip90` and now
   `@openagentsinc/tassadar-executor`; neither is on npm. A consumer
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

## 5. Artanis: the agent-driven operation this lane was built for

Extension, 2026-06-10: a review of the Artanis surface (44 docs under
`docs/artanis/`, ~30 `artanis-*` modules in the
worker) against the executor lane, with speculation labeled as such.

### 5.1 What Artanis is today

Artanis is the autonomous operator-agent layer of the product surface:
a durable tick/claim/closeout **loop ledger** (`agent_artanis`, one
active loop per scope, idempotent ticks, blockers, approval
requirements, closeout receipts, Forum publication intents, next-tick
scheduling — `artanis-loop.ts`, autonomous-loop contract doc), a
worker-cron **scheduled runner** that is config-gated
(`config.artanis.scheduledRunnerEnabled`,
`runner_backend.public.artanis.worker_cron`), typed **action kinds**
with risk classes (`pylon_triage`, `training_launch`, `wallet_spend`,
`forum_publication`, … — risky kinds can never be marked safe and
require approval + authority receipts), an **operator console and
steering contract**, **approval gates**, a **Forum publication queue**
with delivery verification, a **public-report authority split**, and
the **Nexus-Pylon admin adapters** — which are exactly the
accepted-work-payout and settlement-bridge routes the PoC used (their
request schemas already carry `artanisDispatchRef` and
`artanisRunRef` fields). The 2026-06-08 launch-status audit's verdict
stands: Artanis has a deployed evidence surface and controlled
enablement projection, but is not a launched autonomous network.

The structural observation that motivates this section: **the green PoC
was a human (well, one agent) performing the Artanis loop by hand.**
Register/triage the Pylon → dispatch the assignment → accept the
closeout on digest evidence → drive the verification challenge → bridge
the settlement → post the Forum report under copy gates. Every step I
executed manually has a typed Artanis surface waiting to own it. The
PoC is therefore not just evidence for the promise — it is a completed
dry run of one full Artanis tick, with receipts shaped the way the loop
ledger expects.

### 5.2 Why executor-trace is Artanis's best first work class (speculation)

Artanis's central constraint is that risky actions require approval:
the loop can act autonomously only where verdicts are mechanical. Most
work classes fail this — grading a GEPA rollout or accepting a coding
artifact embeds judgment. Executor-trace work is the exception by
construction: the acceptance predicate is a digest comparison, the
verification class is deterministic replay, and rejection carries the
exact tampered step. It is the one work class where the **entire**
dispatch → execute → verify → accept span can be `safe`-classed actions
under the existing risk rules, with exactly one `approval_required`
action remaining — `wallet_spend` at payout — which is precisely where
the owner's standing spend-enable posture already draws the line. The
cheapest verification grade and the strictest autonomy gate meet at the
same boundary, and that is not a coincidence; it is the
work-that-proves-itself thesis expressed in Artanis's type system.

Mapped onto the existing vocabulary, a standing executor-trace loop
would look like:

| PoC manual step | Artanis surface that owns it | risk class |
| --- | --- | --- |
| Pylon registration/heartbeat/capability triage | `pylon_triage` action; health/staleness monitor | safe |
| Assignment dispatch (no-spend) | continual job template → work routing target `pylon` | safe |
| Closeout acceptance on digest match | tick closeout receipt, gated on the artifact digest ref | safe (mechanical predicate) |
| Worker replay + challenge lifecycle | verification action against `/api/operator/tassadar/replay` + the #4674 queue | safe |
| Paid closeout + settlement bridge | Nexus-Pylon adapters (already Artanis-shaped) | `wallet_spend`, approval_required |
| Forum report | publication queue + delivery verification, copy-gated to the promise safeCopy | `forum_publication`, gated |

### 5.3 The always-on floor

The executor lane gives Artanis a queue that never empties: psionic's
own conformance needs — fixture sweeps across the five executor legs,
bounded-harness replays, cross-language digest parity on every executor
release — are real, useful, continuously available work ("we are our
own first buyer," per the lane essay). A standing Artanis loop
dispatching that backlog to idle Pylons at no-spend, with sampled paid
closeouts under capped `wallet_spend` approvals, is the smallest honest
version of an autonomous work network: every tick produces
digest-pinned receipts, every acceptance is replayable, and the
comparative-economics evidence packets Artanis already models get their
cleanest possible unit (cost per verified trace). The weak-device
validator lane (#4676) slots in as Artanis-scheduled replay challenges
— validator work as paid assignments, scheduled by the same loop that
dispatched the work being checked.

### 5.4 What tying it together would take (concrete, beyond v0.3)

None of this belongs in the v0.3 release; all of it builds on surfaces
that exist:

1. An **executor-trace continual job template** — shipped under #4697 as
   `executor_trace_replay`, binding Tassadar payload schema refs, workload
   refs, the executor capability ref, and a zero-sats default spend cap to the
   continual-learning template ledger. The original note was: the template-kind
   enum (`adapter_validation`, `benchmark_eval_rerun`, …) wants a
   seventh kind (or `adapter_validation` reused) binding the dispatch
   script's payload shape to a template with spend caps and workload
   refs.
2. A **tick wiring** from the scheduled runner — shipped under #4697 as a
   config-gated executor-trace tick that records no-spend Pylon dispatch refs,
   exact-replay verdict refs, deterministic closeout receipts, and a queued
   Forum intent. The loop still grants no direct dispatch, settlement, spend,
   or Forum publishing authority.
3. **Approval-gate plumbing for the paid sample** — shipped under #4697 as a
   `wallet_spend` approval requirement and pending approval gate whose
   authority ref is the operator spend-enable; settlement-bridge receipt refs
   remain required before any paid sample can close.
4. **Copy gates** — shipped under #4697 by pinning the queued Forum intent
   body to the `compute.tassadar_executor_poc.v1` promise safeCopy. The
   public-report authority split doc governs, and the launch-status audit's
   no-overclaim posture extends unchanged.
5. The two PoC residuals become Artanis prerequisites: registration
   ownership (the loop's agent must own the Pylon registrations it
   posts settlement events for) and the hosted-MDK programmatic-payout
   switch (or the agent-wallet adapter registered in the payment
   authority) for payouts that do not route through a local bridge.

## 6. Verdict

The lane is **release-poised**: the execution path is in the RC source,
tested, live-proven against production, and the release gate is green
with the packaging fix from this audit. It is **not yet
release-complete** until item 1 above, the shared npm publish mechanism
tracked by #4654, is resolved for `@openagentsinc/pylon@0.3.0`; items
2–5 landed under #4696 and now make the lane honest inside the release
candidate source and packaged-network smoke. Nothing about
the lane requires changing what v0.3 *is* — it slots into the existing
assignment, capability, smoke, and no-overclaim machinery exactly as
those systems were designed to absorb a new work class. Beyond the
release, section 5's #4697 follow-up now makes this lane Artanis's first
typed scheduled-runner work class. Live autonomous launch remains separately
gated, but the no-spend dispatch, replay-verdict, paid-sample approval, and
safeCopy publication-intent records now exist under Artanis's own risk rules,
with spend approval remaining exactly where the owner's posture already puts
it.

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
  (release cluster), #4696 (v0.3 inclusion), #4697 (Artanis work class),
  #4676 (validator lane)
- Promise: `compute.tassadar_executor_poc.v1` (green, 2026-06-10.12)
- Artanis: `apps/openagents.com/workers/api/src/artanis-loop.ts`,
  `artanis-scheduled-runner.ts`, `artanis-continual-learning-templates.ts`,
  `artanis-work-routing.ts`, `artanis-approval-gates.ts`,
  `artanis-nexus-pylon-adapters.ts`, `artanis-forum-publication.ts`;
  `docs/artanis/2026-06-06-autonomous-loop-contract.md`,
  `2026-06-08-artanis-gepa-network-launch-status-audit.md`,
  `2026-06-08-artanis-public-report-authority-split.md`
