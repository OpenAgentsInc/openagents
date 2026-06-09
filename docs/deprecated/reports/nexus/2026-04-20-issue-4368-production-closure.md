# Issue 4368 Production Closure

Generated: 2026-04-20T22:13Z

## Result

Issue `#4368` has an accepted-work homework payout settled on production Nexus.
The live treasury placeholder lane is disabled, so the old recurring
placeholder payments are no longer part of the proof or the operator loop.

## Local Proof First

The local proof-runtime lane remains the primary iteration gate.

- Existing full local proof bundle:
  `docs/reports/nexus/issue-4368-local-closure-20260420202905/closure-summary.json`
- Current commit focused checks:
  `cargo fmt --check -p nexus-control`
- Current commit focused checks:
  `cargo test -p nexus-control wallet_snapshot_updates_receives_and_confirmed_payout_totals -- --nocapture`
- Current commit focused checks:
  `cargo test -p nexus-control public_stats_include_unconfirmed_dispatched_sats_in_visible_total -- --nocapture`
- Current commit focused checks:
  `cargo test -p nexus-control simulated_wallet_covers_local_proof_funding_and_dispatch -- --nocapture`
- Current commit focused checks:
  `cargo test -p nexus-control launch_homework_on_all_updated_online_pylons_and_pay_on_accept -- --nocapture`

## Production Build And Deploy

- Commit: `fc01e80c03e33d6653930a5dcf3beb83b94ecbeb`
- Image:
  `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:fc01e80c03e3`
- Build receipt:
  `docs/reports/nexus/20260420-220313-cloudbuild-image-fc01e80c03e3.json`
- Active host: `nexus-mainnet-1`
- Active image verified from Docker:
  `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:fc01e80c03e3`
- Deploy smoke result:
  placeholder payouts disabled and accepted-work pending payout count reached `0`.

## Homework Run

- Training run:
  `run.cs336.a1.cs336_a1_issue4368.20260420214836.03f7f73f`
- Window:
  `window.cs336.a1.cs336_a1_issue4368.20260420214836.03f7f73f.0001`
- Worker:
  `eb3944e253c9169e35638a7e9b639ba5a0d06eceb721e38f4d07e82d93779237`
- Assignment:
  `assign.run.cs336.a1.cs336_a1_issue4368.20260420214836.03f7f73f.window.cs336.a1.cs336_a1_issue4368.20260420214836.03f7f73f.0001.worker.1.attempt1`
- Accepted outcome:
  `accepted.training_window.window.cs336.a1.cs336_a1_issue4368.20260420214836.03f7f73f.0001`
- Closeout status: `reconciled`
- Accepted contributions: `1`
- Promotion ready: `true`

The production closeout was manually driven through the Nexus coordinator using
the local issue `#4385` proof runtime as the authority-side proof source. The
live worker assignment was real, but the worker did not autonomously complete
the homework during the wait window.

## Payout

- Payout class: `accepted_work`
- Amount: `10` sats
- Payment id: `019dace3-3bed-74f2-98d7-83b09bed9df0`
- Payout status: `confirmed`
- Reconciliation status: `settled`
- Accepted-work pending payout count: `0`
- Accepted-work confirmed payout count: `1`
- Accepted-work attention payout count: `0`
- Accepted-work sats paid in 24h: `10`

Production status after deploy reported wallet runtime `connected` with
hydration mode `cached_balance_after_sync_timeout`. That is expected: Spark full
sync still exceeds the bounded sync timeout, but the deployed fix confirms
tracked payout ids through cached balance plus bounded payment history scan.

## Notes

The public treasury status still contains historical placeholder payout totals
from before the placeholder lane was disabled. Those are not ongoing payments.
The `placeholder_payout_mode` remains `disabled`; homework accepted-work payout
is now the only production payout path used for this proof.
