# LDK Accepted-Work Production Proof

Date: 2026-05-18

Issues: OpenAgentsInc/openagents#4485, #4486, #4488, #4499, #4502, #4503, #4495

This report records the first clean production proof that a fresh Nexus
training run can move through the LDK-only paid-work path:

1. Nexus launches a targeted CS336 A1 training run for a Pylon with a
   registered LDK payout target.
2. The Pylon claims worker work and seals a contribution with the packaged
   Psionic runtime.
3. A validator Pylon claims and verifies the sealed contribution with the same
   packaged runtime.
4. Nexus reconciles the window as rewarded.
5. Treasury dispatches and settles the accepted-work payout over the LDK rail.

## Proof Summary

- OpenAgents commit deployed to hosted Pylons: `ad27f320b`
- Pylon binary artifact:
  `gs://openagentsgemini_cloudbuild/pylon-binaries/ad27f320ba6e8099b6522e1c13f60a2b634a5582/pylon`
- Pylon binary SHA-256:
  `e839dd7857f2e8f7ddaaabb32a17c7415c3d5b773107282047b381cb0f6e0e16`
- Packaged Psionic runtime revision: `55e4b66f`
- Psionic runtime archive:
  `gs://openagentsgemini_cloudbuild/psionic-runtime/55e4b66f/psionic-runtime-55e4b66f.tar.gz`
- Psionic runtime archive SHA-256:
  `2444877f67ed8f1d396b6a999dcb21272d99d2735bd7f65eda465e72f517108f`
- Hosted `psionic-train` binary SHA-256 after install:
  `76c60acaf0dc9837c5679d92e9b404339d59a6cbd47b9bc5c9d2c19a60d29b67`

Production proof run:

- Training run:
  `run.cs336.a1.ldk-proof-20260518151532`
- Accepted window:
  `window.cs336.a1.ldk-proof-20260518151532.0001`
- Worker Pylon:
  `pylon-gcp-1`
- Worker provider pubkey:
  `98bf27e2ea7b89451ce27323a7b4570b72141a7c13d0eb740672f6a3994e7bb4`
- Validator Pylon:
  `pylon-gcp-3`
- Validator provider pubkey:
  `0f46d623c9eb458bf0d04b3e7c5a055ecf89f01058e04684620625b76049b8f3`
- Contribution id:
  `cf7c70416d7265f948fa78ee1e2f94b7bf03ef5975449e8eb89d244816b300d0`
- Latest checkpoint:
  `checkpoint://psion/cs336_a1_demo/run.cs336.a1.ldk-proof-20260518151532/step-000004`
- Payout amount:
  `25 sats`
- Payout class:
  `accepted_work`
- Final payout status:
  `confirmed`
- Final reconciliation status:
  `settled`

The final run detail reported:

```text
scheduler_window_state: accepted
latest_window_status: reconciled
latest_closeout_status: rewarded
accepted_contributors: 1
model_progress_contributors: 1
validator_challenges_open: 0
validator_challenges_queued: 0
payout.status: confirmed
payout.reconciliation_status: settled
treasury.wallet_runtime_status: connected
treasury.payout_loop_health: idle
```

Treasury refresh for the same proof window reported:

```text
active_treasury_provider: ldk
active_treasury_rail: ldk
ldk_readiness.state: ready
ldk_readiness.registered_payout_target_count: 1
ldk_readiness.projected_channel_count: 1
ldk_readiness.projected_inbound_capacity_sats: 2000
ldk_readiness.projected_outbound_capacity_sats: 3843
wallet_balance_sats: 3843
wallet_lightning_balance_sats: 2817
accepted_work_payout_sats_paid_24h: 25
weak_device_accepted_work_payout_sats_paid_24h: 25
payouts_confirmed_24h: 1
```

## What Failed Before This Proof

The earlier proof run `run.cs336.a1.ldk-proof-20260518094050` reached worker
completion, but validator replay failed with:

```text
failed to resolve machine runtime identity:
failed to resolve psionic repo root: No such file or directory
```

Root cause: the hosted Pylon deploy updated `/usr/local/bin/pylon` but did not
replace `/var/lib/pylon/psionic`. Validators were still running against a stale
packaged Psionic runtime whose marker revision was
`09b71872b24a934228f61c28e65e3aa544025f54`.

Fix: package the clean Psionic runtime at `55e4b66f`, install it across all
hosted Pylons with `scripts/deploy/nexus/29-install-pylon-psionic-runtime.sh`,
and verify each host reports:

```text
runtime_surface_detected: true
psionic_repo_root: /var/lib/pylon/psionic
psionic_repo_source: env_override
.openagents-psionic-revision: 55e4b66f
```

This is now a required production gate. Updating the Pylon binary alone is not
enough for retained training lanes.

## Residual Attention Items

- Production has historical payout ledger attention for old non-LDK or
  malformed payment-target records. That does not block the new LDK proof, but
  it should remain visible until reconciled or explicitly retired.
- Only one Pylon identity currently has a registered LDK payout target. More
  hosted and user Pylons must complete Pylon v0.2 LDK target registration
  before broader paid-work launch.
- The CS336 training queue still has old retained-window backlog. Fresh
  targeted run proof should use `training intake --run-id <run_id>` for
  validators when the operator is proving one run instead of draining the whole
  historical queue.

## Closeout Principle

LDK production readiness is proven only by a fresh accepted-work closeout from
active binaries and a settled LDK payout record. Funding invoices, wallet
balances, or old payout rows are not sufficient proof by themselves.
