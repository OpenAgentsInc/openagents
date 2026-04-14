# Local Nexus + Pylon Paid Dry Run From Clean Worktree

Date: 2026-04-13  
Scope: one-machine dry run from a fresh `openagents` worktree using the real
`nexus-control` server, the real `pylon` worker, the real
`nexus_cs336_a1_demo` seeding and closeout path, the real local object-store
fetch path, and the hosted-treasury code path in synthetic-confirmed mode

## Runtime Root

Retained local root for the corrected proof:

```text
/tmp/local-nexus-pylon-e2e.fixed2.59pBZJ
```

Important proof files:

- `nexus-data/receipts.ndjson`
- `nexus-data/treasury-state.json`
- `pylon-run/state/runtime-state.json`
- `pylon-run/runs/run.cs336.a1.demo/manifests/invocation_manifest.json`
- `pylon-run/runs/run.cs336.a1.demo/supervisor/.../stdout.log`

## What This Dry Run Proved

This corrected rerun proves the path that matters for the next live rollout:

- a fresh local `Nexus` starts empty
- the `CS336 A1 Demo` run can be seeded into that empty authority
- a fresh local `Pylon` can initialize, register a payout target, admit itself,
  claim the worker lease, fetch the signed artifacts, and materialize the real
  invocation manifest
- the leased `psionic-train` worker run exits `succeeded`
- `nexus_cs336_a1_demo closeout` can close the window using the real cached
  lease state and real manifest digest
- the treasury loop records and confirms one accepted-work payout for that
  training contribution
- placeholder liveness payout can be fully disabled in policy while the
  accepted-work payout still dispatches and confirms

## Runtime Fixes This Rerun Needed

Two separate fixes mattered in this proof.

### 1. Worktree-aware Psionic discovery

When `pylon` ran from an `openagents` Git worktree, its default `Psionic` repo
discovery still resolved to a nonexistent `.worktrees/psionic` path.

That kept the node stuck at:

- `runtime_surface_detected = false`
- `contributor_supported = false`
- `tier0_presence`

The fix was:

- walk upward from `CARGO_MANIFEST_DIR`
- use the first ancestor that contains a sibling `psionic/Cargo.toml`
- keep `OPENAGENTS_PSIONIC_REPO` as the highest-priority override

### 2. Placeholder payout mode must be in the live binary, not just tests

The first local rerun after the treasury policy change still used a stale
`target/debug/nexus-control` binary.

Symptoms:

- `cargo test` had passed
- `/v1/treasury/status` still showed `policy_schema_version = 1`
- `placeholder_payout_mode` was missing from the status response
- placeholder payouts kept accruing even though the env was set to `disabled`

The fix was to rebuild the actual server binary:

```bash
cargo build -p nexus-control --bin nexus-control --bin nexus_cs336_a1_demo --manifest-path Cargo.toml
```

After that rebuild, the live local authority reported:

- `policy_schema_version = 2`
- `placeholder_payout_mode = "disabled"`
- `eligible_online_payout_targets = 0` until an accepted-work closeout existed

## Local Control-Plane Facts

Corrected local services:

- `nexus-control` on `127.0.0.1:51050`
- local object store on `127.0.0.1:51051`
- local Pylon admin listener on `127.0.0.1:51052`

Important local runtime requirements that mattered:

- the local object server had to stay up while `pylon` fetched signed artifact
  URLs
- `pylon` training coordination needed a local Nexus bearer token through
  `OPENAGENTS_PYLON_TRAINING_NEXUS_BEARER_TOKEN`
- `pylon serve` had to be restarted after `pylon online`; the first serve loop
  had cached an offline state and would not admit itself until restarted

## Real Lease Proof

The retained runtime state after intake shows the real lease:

- `assignment_id = assign.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.1.attempt1`
- `lease_id = lease.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.1.attempt1.rev1`
- `last_exit_code = 0`

The supervisor `stdout.log` recorded:

- `outcome = succeeded`
- `exit_code = 0`
- `lane_id = psion_cs336_a1_demo_v1`
- `backend_family = cpu`
- `topology_class = single_host_cpu_reference`
- `manifest_digest = caf20a3947f5af82aded2c1cfff46c5e1a3c877dda506ec98d9ad3c0c7e4e2fb`

## Closeout And Accepted-Work Payout Proof

The closeout command returned:

- `accepted_outcome_id = accepted.training_window.window.cs336.a1.demo.0001`
- `payout_eligible_closeouts = 1`

After the next treasury interval:

- `GET /api/training/summary` showed:
  - `accepted_closeouts = 1`
  - `payout_eligible_closeouts = 1`
  - run `latest_closeout_status = rewarded`
  - run `current_window_id = window.cs336.a1.demo.0002`
- `GET /api/stats` showed:
  - `nexus_payout_sats_paid_total = 120`
  - `nexus_accepted_work_payout_sats_paid_total = 120`
  - `nexus_placeholder_payout_sats_paid_total = 0`
  - `nexus_strong_lane_accepted_work_payout_sats_paid_total = 120`
  - `nexus_payouts_confirmed_24h = 1`
- `GET /v1/treasury/status` showed:
  - `policy_schema_version = 2`
  - `placeholder_payout_mode = "disabled"`
  - `eligible_online_payout_targets = 0`
  - `payout_sats_paid_total = 120`
  - `accepted_work_payout_sats_paid_total = 120`
  - `placeholder_payout_sats_paid_total = 0`

The treasury ledger summary recorded one payout record and classified it
correctly:

- `payout_record_count = 1`
- `accepted_work_confirmed_payout_count = 1`
- `recent_training_payouts[0].classification.payout_class = accepted_work`

The synthetic wallet balance moved from `100000` sats to `99880` sats.

## Truthful Claim From This Dry Run

This corrected rerun proves all of the following in one local stack:

- `Pylon` can do the real `CS336 A1 Demo` worker job
- `Nexus` can accept and reward that closeout
- the treasury can pay for accepted work
- placeholder liveness pay can be disabled at policy level at the same time

That is the local proof we needed before touching production.

## Remaining Local Nuisance

The local `pylon` loop still emitted a repeated non-blocking warning after the
successful worker exit:

- `automatic pylon training runtime receipt upload failed: failed to parse training manifest .../invocation_manifest.json`

That warning did not block:

- lease claim
- real worker launch
- successful worker exit
- window closeout
- accepted-work payout confirmation

It is still worth fixing because it adds operator noise and hides whether a
future receipt-upload failure is important or not.
