# Nexus Treasury Wallet Recovery Runbook

Date: 2026-04-20

This note captures the recovery path needed after OpenAgents issue `#4368`
production deploys kept failing the payout smoke even though the local `#4385`
proof runtime passed. The live Nexus service can create and serve run/proof
state, but the production treasury wallet is not producing fresh completed
Spark sends.

## Observed Failure

The `6cc0723e8136` image was built from `main` and deployed to
`nexus-mainnet-1`. The rollback-gated smoke failed:

- service stayed `active`
- `inference_ready` recovered to `97`
- `recent_completed` stayed `0`
- `wallet_runtime_status` stayed `error`
- `wallet_last_error` stayed `treasury_funding_target_timeout:10000`
- `wallet_balance_sats` stayed `80`

The candidate logs are more specific than the public status:

- the Spark wallet was built successfully
- the wallet scanned leaves
- many leaves were ignored by the SDK as `SplitLocked` or `TransferLocked`
- the wallet disconnect later timed out
- the service rolled back to the previous image

That means the next honest production move is wallet recovery/inspection, not
another scheduler or Autopilot proof harness change.

## Shipped Recovery Path

The Nexus relay image now includes both:

- `/usr/local/bin/nexus-relay`
- `/usr/local/bin/nexus-control`

The recovery helper uses the `nexus-control treasury ...` CLI inside the same
registry image and with the same production env file mounted into the
container. It always stops `nexus-relay` before inspecting or swapping wallet
storage so the wallet database is not copied while the service is writing it.

Generate a report:

```bash
NEXUS_TREASURY_RECOVERY_ACTION=report \
DEPLOY_IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:<image-tag> \
bash scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

The script prints JSON and writes the report under the VM data disk. Review:

- `comparison.validation_passed`
- `comparison.wallet_identity_pubkey_match`
- `comparison.recommended_action`
- `current_storage.balance_sats`
- `rebuilt_storage.balance_sats`
- `current_storage.payment_totals`
- `rebuilt_storage.payment_totals`
- any `error` fields on either storage inspection

Cut over only after reviewing the report:

```bash
NEXUS_TREASURY_RECOVERY_ACTION=cutover \
NEXUS_TREASURY_RECOVERY_REPORT_PATH=/var/lib/nexus-relay/treasury/<report-dir>/recovery-report.json \
DEPLOY_IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:<image-tag> \
bash scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

There is also a guarded combined mode:

```bash
NEXUS_TREASURY_RECOVERY_ACTION=report-and-cutover \
DEPLOY_IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:<image-tag> \
bash scripts/deploy/nexus/09-recover-treasury-wallet.sh
```

The combined mode proceeds to cutover only when the generated report validates
and recommends `cutover_rebuilt_storage_after_service_stop`.

## Post-Recovery Proof

After a report or cutover, do not close `#4368` from the recovery command
alone. The acceptance gate is still:

1. deploy the fixed image from pushed `main`
2. pass the rollback-gated post-restart payout smoke
3. verify VM-local `/healthz`, `/api/stats`, and `/v1/treasury/status`
4. confirm a fresh completed Spark send after service start
5. trigger or observe the homework run path
6. record accepted-work payout receipt evidence

If the recovery report does not validate or does not recommend cutover, keep
`#4368` open and treat the report as the next debugging artifact.
