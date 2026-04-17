# Episode 224 End-to-End Demo Gap Audit

Date: 2026-04-14
Updated: 2026-04-16
Owner repo: `openagents`
Related issues:

- `openagents#4352`
- `openagents#4368`
- `openagents.com#24`
- `psionic#943`

## Demo-Readiness Verdict

Current classification: `ready with operator launch path and checked-in autonomous closeout runtime`

Why:

- the bounded CS336 A1 lane is now relaunchable and verifiable from `psionic`
- Nexus now exposes both the public named-run detail read model and a narrow
  admin launch endpoint for the bounded demo lane
- the public run page and `/stats` linkout already exist in `openagents.com`
- `apps/pylon` now has a checked-in retained closeout path that can seal,
  finalize validator replay, reconcile, and observe payout state from retained
  worker output
- the remaining missing pieces are a website-side admin trigger in
  `openagents.com#24` and a fresh production run that proves accepted payout on
  the current live fleet

That means the system is no longer stuck at "proof exists but repeatable demo
not ready." It has a legitimate operator control path and a checked-in
post-worker runtime path. It is not yet fully "ready now" because the admin
click path still needs to be wired through the website repo and a fresh live
run still needs to prove the accepted payout path outside retained tests.

## Gap Table

| Layer | Already implemented | Missing / partial | Severity | Repo / owner | Exact recommended fix |
| --- | --- | --- | --- | --- | --- |
| Web UX | Public `/stats` board and named run page already exist in `openagents.com` | No website admin button yet for launching the bounded run | important | `openagents.com` | Add authenticated admin action that calls the Nexus launch endpoint and redirects to the returned run page |
| Backend / Nexus contract | Public `/api/training/runs/{trainingRunId}` snapshot now exists; bounded launch endpoint now exists at `/v1/admin/training/demo-runs/cs336-a1/launch` | Production env must wire `NEXUS_CONTROL_ADMIN_BEARER_TOKEN` and the website-side caller | important | `openagents` + `openagents.com` | Configure the bearer token on Nexus and the matching website secret, then call the launch route server-side |
| Training lane / Psionic | Canonical CS336 A1 demo lane contract exists and `psionic#943` added bounded-run verification | Live repeatability still depends on healthy manifests, admitted hosts, and current environment reachability | important | `psionic` | Keep the lane verifier in the operator loop and fail fast if manifest or environment resolution regresses |
| Pylon distribution | Retained dual-host proof exists for `run.cs336.a1.demo`; scheduler role-plan remains bounded to two workers; checked-in Pylon runtime can continue retained worker output through seal, validator finalize, reconcile, and payout observation | Fresh live proof still depends on real online Pylons being admitted, healthy, and actually running the updated binary | important | `openagents` + live operators | Use the new launch route only when current demo-capable nodes are online, admitted, and on the current release line |
| Payout / treasury / proof | Run-detail snapshot and public stats already expose treasury health, wallet runtime, contribution rows, and closeout/payout flags; Pylon now persists accepted and payout observation locally | Fresh production proof of accepted contribution plus payout receipt on the current fleet is still missing; UI must not imply completed payout before it is observed live | important | `openagents` + `treasury` | Keep degraded wallet / validation caveats visible, and only use paid language after the live run shows a real payout receipt |
| Operator testability | There is now a legitimate server-side launch path, a checked-in autonomous closeout runtime, and a new retained end-to-end truth test from worker completion to paid receipt | Website click path is still separate work and live-demo success still depends on node health | important | `openagents.com` + operators | Ship the site button and keep the operator runbook attached to the admin surface while the live post-worker smoke remains manual |

## Explicit Answers

### Is there currently a button in `openagents.com` that can launch the bounded CS336 run?

No, not yet in the current checked-in website state. That remains the owning
scope of `openagents.com#24`.

### If not, is there already a backend endpoint that can support that button?

Yes.

The backend endpoint is now:

- `POST /v1/admin/training/demo-runs/cs336-a1/launch`

It is intentionally narrow and returns the created or reused run plus the full
public run-detail snapshot.

### If not, what exact endpoint or command must be added?

Already added in this repo:

- `POST /v1/admin/training/demo-runs/cs336-a1/launch`

The remaining website work is to call it from an authenticated admin action.

### Is the existing run-detail page only a viewer, or can it support operations?

The named-run page is still a viewer.

Operations now live on the Nexus admin launch route. That separation is
deliberate: public proof reading stays public, while bounded run creation stays
behind server-side admin auth.

### Can the bounded CS336 A1 lane be relaunched safely and repeatedly?

Yes, with caveats.

It is now safe and repeatable at the contract level because:

- `psionic#943` hardened the bounded lane verifier
- this repo now seeds the canonical registry contracts and launches the run
  deterministically

The live result still depends on real hosts being healthy and admitted.

### Can multiple Pylons reliably receive the same named run and produce a fresh proof window?

Yes for the bounded worker-distribution path, and there is retained proof that
it already worked once on two hosts.

The remaining risk is operational, not architectural: online admitted nodes,
manifest reachability, and current fleet health still determine whether a fresh
live run succeeds on demand.

The post-worker gap is narrower than it was in the April 14 snapshot. The
checked-in `apps/pylon` runtime now drives retained worker output into seal,
validator finalize, reconcile, and payout observation. What is still missing is
a fresh production run that proves those same steps on the live updated fleet.

### Are payouts actually visible, or only implied?

They are no longer only implied in checked-in runtime behavior. `Pylon` now
persists acceptance and payout observation locally and can advance a retained
assignment to `paid` in the new autonomous truth test.

Visible now:

- treasury health
- wallet runtime status
- degraded reason
- closeout status
- payout eligibility flags
- contribution acceptance state
- payout receipt id when the authority surfaces one

Not yet honest to claim:

- fresh production settled payout completion on demand
- full live-fleet reconciliation or validation closure without watching the
  real run

### What caveats must remain visible to keep the demo honest?

These caveats must stay on screen:

- treasury degraded or wallet runtime unhealthy
- sealed window not yet reconciled
- validator backlog or validation pending
- payout eligibility does not equal payout settled
- live demo success still depends on real online admitted Pylons

## Transcript 222 Promise Status

Against `docs/plans/transcript-222-launch-truth-contract.md`, the current state
is:

- presence claims are supported by the live public stats fields
- assigned-contributor claims are supported by the launch response and run
  detail snapshot
- accepted-contributor and model-progress claims are now supported by the
  checked-in post-worker authority path, but they are still only honest as live
  public claims after a fresh run actually reaches accepted outcome state
- payout-linked claims remain conditional on observed payout receipts, not
  payout eligibility alone

## Implementation Shipped In This Repo

Files:

- `apps/nexus-control/src/lib.rs`
- `apps/pylon/src/lib.rs`
- `docs/kernel/compute-training-authority.md`
- `docs/pylon/distributed-training-launch-status.md`
- `scripts/release/check-pylon-episode-223-cs336-a1-local.sh`
- `scripts/release/check-pylon-transcript-222-canary.sh`

Backend/runtime changes:

- re-landed the public named-run detail endpoint
- added the bounded admin launch route
- added Nexus-side bearer-token auth for the admin launch route
- made the launch path seed missing CS336 demo registry contracts
- made fresh launches expose the initial window id immediately instead of
  waiting for the first lease claim
- added validator-claim target bindings so a validator Pylon can materialize
  replay inputs from the authority response alone
- made `Pylon` persist validator claim records and closeout progress in
  retained runtime state
- made retained worker output trigger seal, validator finalize, reconcile, and
  payout observation automatically during terminal sync
- promoted the new autonomous closeout truth test into the retained release
  scripts instead of treating manual reconcile as the release gate

Tests added or exercised:

- `admin_cs336_demo_launch_route_requires_admin_bearer_token`
- `admin_cs336_demo_launch_route_registers_contracts_creates_run_and_reuses_active_run`
- `validator_challenge_claim_returns_target_bindings_for_homework_contributions`
- `pylon_autonomously_closes_homework_assignment_from_worker_completion_to_paid_receipt`
- `training_terminal_sync_seals_window_from_retained_worker_artifacts`
- `training_run_detail_endpoint_surfaces_sealed_window_contributions_and_caveats`
- `training_operator_summary_and_stats_surface_live_run_state`

## Operator Runbook

Current operator-valid path:

1. Ensure Nexus is configured with `NEXUS_CONTROL_ADMIN_BEARER_TOKEN`.
2. Ensure at least two admitted demo-capable Pylons are online for
   `trainnet.cs336.a1.demo`.
3. Launch the bounded run with:

```bash
curl -X POST \
  -H "Authorization: Bearer $NEXUS_CONTROL_ADMIN_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  https://nexus.openagents.com/v1/admin/training/demo-runs/cs336-a1/launch \
  -d '{"reuse_existing_run":true}'
```

4. Read the response:
   - `training_run_id` is the canonical page id
   - `launch_state` tells you whether a run was created or an active one was reused
   - `run_detail.featured_window_id` is the first expected window shape
5. Open the public proof page for that run on `openagents.com`.
6. Watch for:
   - assigned contributors on the run
   - contribution rows from multiple hosts
   - featured window status moving through active, sealed, and reconciled
     states
   - accepted contribution rows
   - payout receipt id only if it is actually observed
   - treasury and validation caveats remaining explicit

Acceptable payout-state language for the demo:

- `payout eligible`
- `payout queued`
- `payout confirmed`
- `validation pending`
- `treasury degraded`
- `wallet runtime error`

Not acceptable:

- `paid` unless settlement is actually confirmed
- `reconciled` unless the window is reconciled

## Still Not Working

- `openagents.com` still needs the actual admin trigger button and server-side caller in `openagents.com#24`
- the launch path is currently operator/server-side, not browser-native
- live repeatability still depends on healthy admitted nodes, current release binaries, and current manifest reachability
- a fresh production run that proves accepted contribution plus payout receipt on the live fleet is still missing
- treasury degradation and live reconciliation gaps remain real and must stay visible
- payout visibility is truthful enough for "beginnings of paid distributed training" only when the run actually surfaces the receipt, not when it is merely eligible
