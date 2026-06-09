# Issue 4368 Production Reality Proof

Date: 2026-04-20

This report replaces the earlier manual-coordinator closeout caveat with a fresh
production run driven by clean local Pylon nodes against `nexus.openagents.com`.
The run used the local proof-runtime path as the development baseline, then
used production Nexus only for final confirmation.

## Local proof baseline

- Local proof bundle already recorded:
  `docs/reports/nexus/issue-4368-local-closure-20260420202905/closure-summary.json`
- Local proof script: `scripts/pylon/issue-4368-local-closure.sh`
- The local proof covered the #4385 simulated authority path, stale recovery,
  replacement refusal, placeholder-payout-disabled behavior, and homework-only
  accepted-work payment behavior before this production run.

## Production runtime

- Public health: `https://nexus.openagents.com/healthz` returned `ok: true`.
- VM service: `nexus-relay` was `active`.
- Active image:
  `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:fc01e80c03e3`
- Current VM release link:
  `/opt/nexus-relay/releases/226f1ff857c75dcf8e62628f3eece7b8b8d01bb7`
- Runtime evidence file:
  `target/issue-4368-reality/20260420T222320Z/nexus-vm-runtime.txt`

## Fresh run

- Network:
  `trainnet.cs336.a1.reality.20260420t222320z`
- Run slug:
  `cs336_a1_reality.20260420t222320z`
- Training run:
  `run.cs336.a1.cs336_a1_reality_20260420t222320z.20260420222726.99d42cc0`
- Accepted window:
  `window.cs336.a1.cs336_a1_reality_20260420t222320z.20260420222726.99d42cc0.0001`
- Worker:
  `episode224-clean2`
  `a49673e52c37899f30ba424d6af714c1658c16137d78a2146401c6d2b326c27b`
- Validator:
  `episode224-clean1`
  `af0d800f07ca5df2a2f64a98239f6f819676c812d18828c841a340a577848e6c`
- Launch request/response:
  `target/issue-4368-reality/20260420T222320Z/homework-launch.request.json`
  `target/issue-4368-reality/20260420T222320Z/homework-launch.response.json`

The launch matched exactly one worker Pylon and no stale/default worker:
`episode224-clean2`. The launch used `pay_only_on_accept: true` and
`amount_sats: 1`.

## Autonomous closeout path

The worker and validator were not manually closed out through coordinator
admin calls.

Worker path:

- `episode224-clean2` claimed the fresh lease.
- The worker materialized the runtime manifest.
- The worker executed the local retained Psionic training runtime.
- The worker produced checkpoints, status packets, `sealed_window_bundle.json`,
  and `closeout_bundle.json`.
- The worker synced terminal receipts through Pylon with
  `OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN` sourced from
  `gcloud auth print-access-token` because ADC was unavailable in this shell.

Validator path:

- `episode224-clean1` claimed the validator challenge.
- The validator downloaded/staged the worker artifacts.
- The validator ran replay and produced validator score/verdict artifacts.
- The validator finalized and reconciled through Pylon.
- Validator final status:
  `stage: paid`, `acceptance_state: rewarded`,
  `payout_state: confirmed`, `payout_receipt_id:
  019dad0c-e701-7d23-96b6-1efebf1f9609`.

Final local Pylon evidence:

- Worker:
  `target/issue-4368-reality/20260420T222320Z/clean2.status.after-status-fix.json`
- Validator:
  `target/issue-4368-reality/20260420T222320Z/clean1.status.after-status-fix.json`
- Final run detail:
  `target/issue-4368-reality/20260420T222320Z/run-detail.final.json`

Both clean Pylon status reports showed `recent_issues: null` and
`pending_closeout_objects: null` after the local status projection fix in this
commit.

## Accepted contribution and payout

- Contribution:
  `9c92cfbbde3abaee8b3527a8cda1b7a9db8ee3ec47e9808de2aa5c2066c4728b`
- Accepted outcome:
  `accepted.training_window.window.cs336.a1.cs336_a1_reality_20260420t222320z.20260420222726.99d42cc0.0001`
- Accepted checkpoint:
  `checkpoint://psion/cs336_a1_demo/run.cs336.a1.cs336_a1_reality_20260420t222320z.20260420222726.99d42cc0/step-000004`
- Payout amount:
  `1` sat
- Payout payment id:
  `019dad0c-e701-7d23-96b6-1efebf1f9609`
- Payout status:
  `confirmed`
- Payout reconciliation status:
  `settled`
- Matching payout count for this run:
  `1`
- Accepted-work pending payout count:
  `0`

Treasury evidence:

- Dispatch poll:
  `target/issue-4368-reality/20260420T222320Z/treasury.after-payout.json`
- Confirmed final:
  `target/issue-4368-reality/20260420T222320Z/treasury.confirmed-final.json`

Treasury still reports degraded health because full wallet sync timed out and
the service used cached balance plus bounded payment scan, but that degraded
state did not block this accepted-work payment. The payment was dispatched,
confirmed, and settled. Placeholder payouts remained disabled.

## Verification

Commands run after the status-projection fix:

```text
cargo fmt --check -p pylon
cargo test -p pylon training_closeout_progress_issue_ignores_worker_progress_after_rewarded_closeout -- --nocapture
cargo build -p pylon --bin pylon
```

Observed final production facts:

- `healthz` returned healthy.
- The active Nexus VM service was `active`.
- A fresh homework run launched on a unique network.
- Exactly one clean worker matched and claimed work.
- A clean validator claimed and finalized validation.
- The contribution was accepted for aggregation.
- The run produced a rewarded accepted outcome.
- Treasury produced exactly one accepted-work payout for the run.
- The accepted-work payout was confirmed and settled.
- No placeholder/liveness payout was paid for this run.

## Status

Issues #4368 and #4409 are complete based on this production proof.
