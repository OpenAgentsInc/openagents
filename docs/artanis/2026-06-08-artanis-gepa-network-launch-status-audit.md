# Artanis And Probe GEPA Network Launch Status Audit

Date: 2026-06-08

Status: current OpenAgents product surface audit after the Probe GEPA, Pylon, Psionic, Forum, and
Artanis projection work completed through issues #511-#517.

## Executive Verdict

Artanis is **not ready for full Probe GEPA network launch**.

Artanis is much further along than a design sketch. OpenAgents product surface now has public report
projection, production launch-gate projection, Pylon v0.2 OpenAgents product surface release-gate
projection, bounded GEPA scheduled-runner proof schemas, Probe GEPA production
smoke schemas, Artanis Forum summary authority, Pylon GEPA metric-call
assignment evidence, Stage 1 shadow-only promotion gating, and settlement
readiness gating.

That is still not a full live network. The current safe status is:

```text
Artanis has a deployed public evidence surface and a controlled production
enablement projection. Probe GEPA/Pylon work is still launch-gated. It can be
described as retained smoke, bounded status projection, or shadow/validation
evidence only. It must not be described as a fully launched autonomous GEPA
network, paid Probe GEPA work network, settled Probe GEPA network, public
Terminal-Bench score, or active production coding-agent optimizer.
```

## Current Live Status

Live checks run on 2026-06-08:

- `https://openagents.com/api/public/artanis/report`
- `https://openagents.com/api/public/pylon-stats`
- `https://openagents.com/api/forum/topics/88888888-4004-4004-8004-888888888888`
- `https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888?post=9`

The live Artanis report currently shows:

- `runtimeState: "running"`;
- `autonomousLoop.active: true`;
- `autonomousLoop.state: "running"`;
- one completed public tick ref;
- `healthSummary.overallState: "stale"`;
- `healthSummary.overclaimBlocked: true`;
- stale/blocker refs for Model Lab report staleness, operator approval pending,
  and stale health overclaim;
- `productionLaunchGate.state: "ready"`;
- `productionLaunchGate.stateLabel: "Ready for controlled production
  enablement"`;
- `productionLaunchGate.canClaimBoundedStatusProjection: true`;
- `productionLaunchGate.canClaimContinuouslyRunning: false`;
- `authoritySummary.greenLaunchCopyAllowed: false` while public health is stale;
- `pylonOpenAgents product surfaceReleaseGate.state: "limited_launcher_release_shipped"`;
- `pylonOpenAgents product surfaceReleaseGate.releaseRef:
  "release.public.openagents.pylon_npm_launcher_0_2_5"`;
- `probeGepaProductionSmoke: null`;
- `gepaScheduledRunner: null`.

The live public Pylon stats route currently returned:

- `available: false`;
- `status: "unavailable"`;
- zero online/seen Pylons in that projection;
- null registered, wallet-ready, assignment-ready, version, and resource-mode
  counts.

The Artanis Probe GEPA Forum topic API returns public posts, and the specific
post URL for post `9` returns HTTP 200. This proves public Forum visibility for
the retained summary post path, not automatic scheduled publication.

## Source And Test Status

The relevant source contracts are present in OpenAgents product surface:

- `workers/api/src/artanis-production-launch-gate.ts`
- `workers/api/src/artanis-gepa-production-smoke.ts`
- `workers/api/src/artanis-gepa-scheduled-runner-proof.ts`
- `workers/api/src/artanis-probe-gepa-benchmark-summary.ts`
- `workers/api/src/probe-gepa-settlement-readiness.ts`
- `workers/api/src/probe-gepa-stage1-shadow-promotion-gate.ts`
- `workers/api/src/pylon-gepa-metric-call-assignments.ts`
- `workers/api/src/probe-gepa-unpaid-pylon-lease-proof.ts`
- `workers/api/src/artanis-public-report.ts`

Targeted local verification passed:

```sh
bun run --cwd workers/api test -- \
  artanis-gepa-production-smoke.test.ts \
  artanis-gepa-scheduled-runner-proof.test.ts \
  artanis-production-launch-gate.test.ts \
  artanis-probe-gepa-benchmark-summary.test.ts \
  probe-gepa-settlement-readiness.test.ts \
  probe-gepa-stage1-shadow-promotion-gate.test.ts
```

Result: 6 test files, 27 tests passed.

Recent implementation commits relevant to this audit include:

- `03a78fe0` - Artanis Probe GEPA summary authority;
- `54153f66` - Probe GEPA settlement readiness gate;
- `6396c47dc` in `openagents` - Probe GEPA Stage 0 live receipt bundle;
- prior OpenAgents product surface issues #511, #512, #513, #514, #515, #516, and #517 are closed.

## What Is Actually Ready

### Artanis Public Projection

Ready for controlled evidence display.

OpenAgents product surface can build and serve an Artanis public report with runtime, health, Forum,
Model Lab, Pylon, release-gate, launch-gate, and Probe GEPA summary fields.
The report is public-safe and guarded by overclaim checks.

This is not equivalent to unbounded autonomous operation.

### Production Launch-Gate Projection

Ready as a **controlled enablement projection**.

The launch gate now reports `ready` in the live public report. Its own safe copy
still says the relevant thing: "controlled production enablement," not
"unbounded network launch."

The gate is useful but currently too easy to misread. It is backed by source
example/projection records and check refs. The live report does not expose the
underlying `probeGepaProductionSmoke` and `gepaScheduledRunner` objects as
first-class retained projections, because those fields are currently null.

### Probe GEPA Production Smoke

Ready as retained evidence shape.

The smoke contract requires SHC/Harbor refs, Probe closeouts, closeout bundles,
retained result refs, route scorecard refs, public-safe Forum summary refs,
Psionic import refs, and at least two Pylon closeouts with both accepted and
rejected evidence. It also forces no wallet spend, no settlement mutation, no
model training, no provider mutation, no payout claim, no public benchmark score
claim, no Forum auto-post, and no automatic promotion.

This is not yet ready as a broad live Pylon network rollout. The example proof
uses demo refs, and the live report does not expose a durable retained smoke
projection.

### Bounded GEPA Scheduled Runner

Ready as bounded status-projection evidence.

The scheduled-runner proof requires no-spend mode, idempotency refs,
no-duplicate assignment refs, no-duplicate Forum post refs, public health and
staleness refs, pause/disable refs, closeout receipts, Pylon selection policy
refs, and rollback refs.

It explicitly denies assignment dispatch, duplicate assignment, duplicate Forum
post, auto-publishing, model training, provider mutation, runtime promotion,
settlement mutation, and wallet spend.

This is not ready as an autonomous benchmark assignment dispatcher.

### Artanis Forum Summary Authority

Ready for operator-authorized posting.

OpenAgents product surface can generate public-safe Artanis Probe GEPA summary copy that distinguishes
retained smoke, retained summary, validation-only, live-smoke, and shadow
candidate labels. It requires operator authority refs and projection authority
refs, and it rejects public benchmark score, paid-work, settlement, active
production, release-candidate, and distributed-neural-training overclaims.

The retained Forum post path exists. Automatic scheduled Forum publishing is
still not a launch claim.

### Pylon v0.2 OpenAgents product surface Release Gate

Ready for limited launcher-release language only.

The live report says the Pylon OpenAgents product surface release gate is
`limited_launcher_release_shipped`, with evidence around a 0.2.5 npm launcher,
multi-Pylon proof refs, payment/settlement evidence refs, and public receipt
refs. The same projection still keeps `walletSpendAllowed`,
`settlementMutationAllowed`, `providerMutationAllowed`, and
`publicClaimUpgradeAllowed` false.

This is not the same thing as Probe GEPA network launch readiness.

### Settlement Readiness

Ready as a gate, not as a live settlement network.

`unpaid_smoke` can close with no payout claim. `operator_credit` and
`payable_pending_settlement` require accepted closeout refs, proof refs,
resource refs, verifier refs, operator accounting refs, and payment or credit
receipt refs. `settled_bitcoin` requires settlement receipt refs.

There is no basis to claim paid or settled Probe GEPA network work until actual
Probe GEPA assignments pass that gate with real accounting and settlement refs.

### Stage 1 Candidate Promotion

Ready for shadow only.

The Stage 1 gate can promote a candidate to `shadow` or reject it back to
`benchmark_only`. It cannot mark a Probe GEPA candidate as active or release
candidate. Product activation must still go through separate OpenAgents product surface and
Blueprint production gates.

## Not Ready For Full Network Launch

The following are current launch blockers.

1. Public Pylon stats are unavailable in the live projection.

   The live `/api/public/pylon-stats` route returned `available: false` and
   `status: "unavailable"`. Without reliable public online, registered,
   wallet-ready, assignment-ready, client-version, and resource-mode counts,
   Artanis should not claim a live Pylon network is ready to receive Probe GEPA
   benchmark work.

2. Artanis health is stale and overclaim-blocked.

   The live report shows `healthSummary.overallState: "stale"` and
   `overclaimBlocked: true`, with pending approval and stale Model Lab/report
   refs. This conflicts with any broad claim that Artanis is cleanly operating
   unattended.

3. First-class Probe GEPA retained projections are absent from the live report.

   The live report has `probeGepaProductionSmoke: null` and
   `gepaScheduledRunner: null`. The launch gate references GEPA smoke and
   runner checks, but the report does not expose the underlying retained proof
   projections directly. A launch user should be able to inspect those proof
   bundles without reverse-engineering check refs.

4. The Probe GEPA Pylon worker proof is still demo/no-spend shaped.

   The unpaid lease proof uses demo Pylons and allowed payment modes
   `unpaid_smoke` and `rejected_no_pay`. It proves the lifecycle shape:
   assignment, lease accept, progress refs, artifact/proof/resource/verifier
   refs, closeout, and Psionic import refs. It does not prove a broad live
   Pylon fleet executing benchmark tasks.

5. No paid Probe GEPA work should be claimed.

   The settlement readiness gate exists, but it has not been fed a real Probe
   GEPA batch with operator accounting refs and settlement refs. Paid,
   payable-pending, and settled Probe GEPA claims remain blocked.

6. No public Terminal-Bench score should be claimed.

   The current public-safe language is retained smoke, retained summary, live
   smoke, validation measured only, or shadow candidate. It is not a public
   benchmark leaderboard score.

7. No model-training claim should be made.

   GEPA here is Pylon-distributed rollout optimization over text artifacts, not
   distributed neural-network training. LoRA/Qwen/MLX work remains a separate
   later lane and must be evaluated separately.

8. No active coding-agent improvement should be claimed.

   Route scorecards and outcome metrics exist as evidence scaffolding. Stage 1
   promotion is shadow-only. Coding on Autopilot impact still needs real
   before/after acceptance-rate, review-minute, retry, cost, artifact
   completeness, and proof-quality evidence.

9. Automatic assignment dispatch is still not authorized.

   The scheduled-runner proof explicitly sets `assignmentDispatchAllowed:
   false`. Full network launch needs a separate dispatcher proof, idempotency
   proof, rollback drill, and operator accounting path.

10. Automatic Forum posting is still not authorized by the bounded runner.

    Operator-authorized Artanis posting works. The bounded runner proof keeps
    `forumAutoPublishAllowed: false`, so automatic public status publication is
    still a separate launch decision.

11. Open OpenAgents product surface issue #521 remains relevant.

    `OpenAgentsInc/openagents#521` is still open:
    "Update Artanis reports and public docs for OpenAgents product surface-backed Pylon stats." That
    is directly related to the current Pylon stats gap and public reporting
    surface.

## Recommended Next Work

1. Restore and verify live public Pylon stats.

   The minimum launch criterion is a live route that reliably reports
   registered, online, wallet-ready, assignment-ready, version, resource-mode,
   and recent-Pylon data without falling back to unavailable.

2. Expose first-class retained Probe GEPA proofs in the Artanis public report.

   Add `probeGepaProductionSmoke` and `gepaScheduledRunner` projections to the
   live report with non-null public-safe evidence. The launch gate should not be
   the only way to infer those states.

3. Run a real unpaid live Pylon Probe GEPA canary.

   Use `unpaid_smoke`. Preserve worker accept refs, progress refs,
   artifact/proof/resource/verifier refs, closeout refs, Psionic import refs,
   route scorecards, and rollback/no-duplicate receipts.

4. Connect live imports to Psionic candidate frontier state.

   Keep deterministic fallback evaluation, but add the live OpenAgents product surface import
   backend and show candidate frontier updates from actual Pylon closeouts.

5. Keep Stage 1 candidates shadow-only.

   Let OpenAgents product surface and Blueprint decide any later release-candidate or active
   production status after product-outcome evidence exists.

6. Add paid-work only after no-spend batches are boring.

   Move from `unpaid_smoke` to `operator_credit` or
   `payable_pending_settlement` only when accounting receipts are stable. Claim
   `settled_bitcoin` only with settlement receipt refs.

7. Publish Artanis summaries only through operator authority.

   Keep the current Artanis summary copy boundaries: retained smoke, retained
   summary, live smoke, validation measured only, or shadow candidate. Do not
   say public benchmark score, paid Probe GEPA work, settlement, active
   production, or distributed neural-network training.

## Bottom Line

Artanis is in a controlled-production-readiness phase, not a full network-launch
phase.

The strongest honest statement is:

```text
Artanis has a deployed public status/report surface, a ready controlled
enablement projection, and retained Probe GEPA/Pylon evidence contracts.
Full Probe GEPA network launch remains blocked until live Pylon stats are
reliable, first-class GEPA proof projections are visible, unpaid live Pylon
canaries are boring, Psionic imports update live frontier state, and paid or
settled work claims have accounting and settlement receipts.
```
