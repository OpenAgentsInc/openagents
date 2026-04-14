# Local Nexus + Pylon Paid Dry Run

Date: 2026-04-13  
Scope: one-machine dry run using the real `nexus-control`, `pylon`, training
artifacts, scheduler, closeout path, and treasury loop

This run was used to prove the current Episode 223 stack locally before
touching production.

## What Was Proven

The local stack now does the full path that matters:

- `Nexus` seeds a named `CS336 A1 Demo` training run
- `Pylon` takes the lease through the normal training intake path
- `Pylon` caches the real manifest digest from the leased runtime state
- `Nexus` reconciles the window as `rewarded`
- the treasury loop records an accepted-work payout for that contribution
- the treasury loop confirms the payout in the same run

This is no longer a fake "close the window with invented digests" rehearsal.
The accepted closeout used the digest that the real local `Pylon` cached for
the active lease.

## Local Runtime Root

The retained local rehearsal root for this proof was:

```text
/tmp/local-nexus-pylon-e2e.clean.PyN5Ot
```

Important proof files inside that root:

- `pylon-run/state/runtime-state.json`
- `pylon-run/runs/run.cs336.a1.demo/manifests/run_manifest.json`
- `pylon-run/runs/run.cs336.a1.demo/manifests/invocation_manifest.json`
- `nexus-data/receipts.ndjson`
- `nexus-data/treasury-state.json`

## Dry-Run Shape

Local services:

- `nexus-control` on `127.0.0.1:51050`
- local object server on `127.0.0.1:51051`
- local Pylon admin surface on `127.0.0.1:51052`

Local treasury mode:

- `NEXUS_CONTROL_TREASURY_ENABLED=true`
- `NEXUS_CONTROL_TREASURY_DISPATCH_MODE=synthetic_confirmed`
- `NEXUS_CONTROL_TREASURY_SYNTHETIC_START_BALANCE_SATS=100000`
- `NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE=disabled`
- `NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW=120`
- `NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS=5`

The important constraint there is deliberate: placeholder liveness payouts were
disabled, so the only way this dry run could pay was via accepted work.

## Commands That Closed The Loop

Seed the demo lane:

```bash
./target/debug/nexus_cs336_a1_demo seed \
  --base-url http://127.0.0.1:51050 \
  --object-store-root /tmp/local-nexus-pylon-e2e.clean.PyN5Ot/object-store \
  --pylon-run-root /tmp/local-nexus-pylon-e2e.clean.PyN5Ot/pylon-run \
  --bucket-uri gs://local-cs336-demo
```

Close out the active worker contribution using the real cached lease digest:

```bash
./target/debug/nexus_cs336_a1_demo closeout \
  --base-url http://127.0.0.1:51050 \
  --node-pubkey 84bfad09fcd16665664da048320ba3b87b231a26d53feb7c031955183b881a4d \
  --assignment-id assign.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.1.attempt1 \
  --lease-id lease.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.1.attempt1.rev1 \
  --pylon-run-root /tmp/local-nexus-pylon-e2e.clean.PyN5Ot/pylon-run
```

That closeout returned:

- `accepted_outcome_id = accepted.training_window.window.cs336.a1.demo.0001`
- `payout_eligible_closeouts = 1`

## Proof Points

After closeout and one treasury interval, `GET /api/training/summary` showed:

- `accepted_closeouts = 1`
- `payout_eligible_closeouts = 1`
- run `latest_closeout_status = rewarded`
- window `window.cs336.a1.demo.0001` with:
  - `status = reconciled`
  - `accepted_outcome_id = accepted.training_window.window.cs336.a1.demo.0001`
  - `payout_eligible = true`

At the same time, `GET /api/stats` showed:

- `nexus_payout_sats_paid_total = 120`
- `nexus_accepted_work_payout_sats_paid_total = 120`
- `nexus_payouts_confirmed_24h = 1`
- `training_payout_eligible_closeouts = 1`

The receipt log then recorded the exact accepted-work payout pair:

- `treasury.payout.dispatched`
- `treasury.payout.confirmed`

with these important attributes:

- `payout_class = accepted_work`
- `payout_basis = aggregation_weight`
- `training_run_id = run.cs336.a1.demo`
- `window_id = window.cs336.a1.demo.0001`
- `contribution_id = contrib.cs336.a1.demo.0001`
- `assignment_id = assign.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.1.attempt1`
- `amount_sats = 120`

The synthetic treasury balance also moved from `100000` sats to `99880` sats
in `nexus-data/treasury-state.json`, which is the expected post-confirmation
balance for one accepted-work payout of `120` sats.

## Important Gotcha

The signed artifact-access path failed until the local GCS signer fixture was a
real service-account-shaped JSON object. The local signer file must include at
least:

- `client_email`
- `private_key`

Without those fields, the local signed-access route fails before `Pylon` can
download the seeded artifacts.

## Meaning

This closes the local proof gap for Episode 223 prep.

The remaining work after this report is not "can the code do it locally?" The
remaining work is:

- ship the same code to hosted `Nexus`
- update real Pylons onto the current build line
- verify one real accepted-work payout against the live treasury path
