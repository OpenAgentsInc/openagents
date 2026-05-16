# Spark Touchpoint Inventory for LDK Transition

Issue: `OpenAgentsInc/openagents#4480`

Date: 2026-05-16

## Purpose

Nexus and Pylon are moving to an LDK-only active payment rail for new funding
and payout operations. Spark remains only for:

- historical receipt reads,
- old wallet/status inspection,
- explicitly enabled final-drain or recovery operations.

This inventory freezes the known Spark write paths and classifies the remaining
touchpoints so later LDK issues can replace or delete them in order.

## Search Commands Used

These were the primary inventory commands:

```sh
rg -l "Spark|spark|SPARK" apps crates packages scripts docs \
  --glob '!docs/reports/**' \
  --glob '!**/target/**' \
  --glob '!**/node_modules/**' \
  --glob '!**/dist/**' \
  --glob '!**/build/**'

rg -n "Spark|spark|SPARK" \
  apps/nexus-relay apps/nexus-control apps/pylon crates/spark \
  crates/openagents-provider-substrate scripts/deploy/nexus \
  docs/nexus-treasury.md docs/deploy \
  docs/2026-04-21-run-pylon-get-paid-for-training.md \
  docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md \
  --glob '!apps/nexus-control/var/**' \
  --glob '!**/target/**'

rg -n "payout_target|PayoutTarget|funding-target|funding_target|bolt11|invoice|treasury|payout" \
  apps/nexus-relay apps/nexus-control apps/pylon \
  crates/openagents-provider-substrate crates/openagents-kernel-core \
  scripts/deploy/nexus \
  --glob '!apps/nexus-control/var/**' \
  --glob '!**/target/**'

find apps/nexus-relay apps/nexus-control apps/pylon crates/spark \
  crates/openagents-provider-substrate scripts/deploy/nexus \
  -maxdepth 3 -type f | sort
```

The historical `docs/reports/**` receipt archive intentionally stays excluded
from the primary source/config inventory. Those reports are historical audit
evidence, not current write-path implementation.

## Default Write Freeze

The normal code path is now default-off for new Spark write material:

- Pylon no longer auto-creates a Spark payout destination.
- Nexus no longer accepts Spark payout-target registration in the active API.
- Nexus no longer creates live Spark-backed funding targets unless
  `NEXUS_TREASURY_PROVIDER=spark_final_drain` and
  `NEXUS_SPARK_FINAL_DRAIN_ENABLED=true` are set. The normal provider is
  `NEXUS_TREASURY_PROVIDER=ldk`.
- Simulated/local proof treasury remains available for tests and proof
  fixtures.

These gates are temporary. They are not a new Spark product mode. They exist so
operators can perform explicit final-drain or recovery work while LDK replaces
the active funding and payout paths.

## Classification Markers

- `legacy-read`: keep only so old state, receipts, and operator docs remain
  readable.
- `final-drain-only`: keep only behind explicit final-drain/recovery gates.
- `ldk-replace`: replace with LDK v0.2 implementation.
- `delete-after-cutover`: remove after LDK is live and Spark state is drained or
  archived.

## Touchpoint Table

| Area | Files | Current behavior | Marker | Deletion condition |
| --- | --- | --- | --- | --- |
| Spark wallet wrapper crate | `crates/spark/Cargo.toml`, `crates/spark/src/lib.rs`, `crates/spark/src/wallet.rs`, `crates/spark/src/signer.rs`, `crates/spark/src/error.rs`, `crates/spark/tests/tree_node_status.rs` | Wraps Breez Spark SDK, creates Spark/Bolt11 invoices, reads balances/payments, sends Spark address payments. | `final-drain-only` and `legacy-read` | Delete after LDK provider handles new funding/payouts and old Spark receipt/state inspection has a replacement archive path. |
| Pylon wallet runtime | `apps/pylon/src/wallet_runtime.rs`, `apps/pylon/src/ledger.rs` | Opens local Spark wallet, creates Spark/Bitcoin receive addresses, creates Bolt11 invoices, pays invoices, records wallet ledger fields. | `ldk-replace`; ledger fields are `legacy-read` | Replace with LDK payment target and local wallet/account view. Keep old ledger fields only while old Pylon state must be inspected. |
| Pylon default online earning loop | `apps/pylon/src/lib.rs` | Previously auto-created a local Spark payout destination when a Pylon went online without a configured settlement destination. | `ldk-replace` | Replaced with LDK payment-target registration. There is no active Spark auto-create fallback. |
| Nexus treasury state and dispatch | `apps/nexus-control/src/treasury.rs` | Stores payout targets, payout records, receive records, Spark wallet state, dispatch plans, Spark payment ids, and wallet error/degraded states. | `ldk-replace`; old records are `legacy-read` | Replace active operation rows and dispatch calls with LDK operation/receipt storage. Keep old rows read-only until final migration/drain report is complete. |
| Nexus payout-target registration API | `apps/nexus-control/src/lib.rs`, `apps/nexus-control/src/treasury.rs`, `crates/openagents-provider-substrate/src/payout_target.rs` | Issues payout-target challenges and registers provider payment targets signed by provider identity. | `ldk-replace` | Replaced with Pylon v0.2 LDK payment-target registration. Spark registration is rejected in the active API. |
| Nexus funding-target API and CLI | `apps/nexus-control/src/lib.rs`, `apps/nexus-control/src/treasury.rs` | Creates treasury Spark address, Spark invoice, compatibility Bolt11 invoice, and wallet snapshot. | `ldk-replace` | Replace with LDK invoice creation. Current live Spark creation is default-off behind `NEXUS_SPARK_FINAL_DRAIN_ENABLED=true`; simulated proof path remains. |
| Nexus deploy image and build context | `apps/nexus-relay/Dockerfile`, `scripts/deploy/nexus/stage-build-context.sh`, `apps/nexus-relay/deploy/Cargo.nexus.lock` | Builds and ships `openagents-spark` into the Nexus relay/control image. | `delete-after-cutover` | Remove after Nexus no longer links Spark for active or final-drain behavior. |
| Nexus recovery/watchdog scripts | `scripts/deploy/nexus/09-recover-treasury-wallet.sh`, `scripts/deploy/nexus/10-install-treasury-watchdog.sh`, `scripts/deploy/nexus/test-recover-treasury-wallet-shell-guards.sh` | Recovers or watches Spark treasury wallet storage and runtime health. | `final-drain-only` | Replace with LDK node recovery/backup/watchdog paths, then keep only the archived Spark recovery report workflow if needed for old receipts. |
| Nexus public stats/homepage projection | `apps/nexus-relay/src/homepage_template.html`, `apps/nexus-relay/src/durable.rs`, `apps/nexus-control/src/lib.rs`, `apps/nexus-control/src/economy.rs` | Projects treasury and payout totals, including Spark-backed payout and wallet degraded state fields. | `ldk-replace`; totals are `legacy-read` | Replace field descriptions and data source with LDK operation/receipt projection. Preserve historical totals in archived reports. |
| Pylon release/bootstrap/docs | `packages/pylon-bootstrap/**`, `docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md`, `docs/2026-04-21-run-pylon-get-paid-for-training.md`, `docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md` | Documents and releases Spark-backed Pylon earning setup. | `ldk-replace` | Update after Pylon v0.2 LDK target registration ships. |
| Treasury runbooks | `docs/nexus-treasury.md`, `docs/deploy/NEXUS_GCP_RUNBOOK.md`, `docs/deploy/NEXUS_TREASURY_FUNDING_INVOICE_RUNBOOK.md` | Documents Spark funding, payout, recovery, degraded states, and SDK pinning. | `final-drain-only` and `ldk-replace` | Rewrite primary runbook around LDK. Retain Spark sections only as a final-drain appendix until LDK-16 closes. |
| Legacy desktop wallet UI | `apps/autopilot-deprecated/src/spark_wallet.rs`, `apps/autopilot-deprecated/src/spark_pane.rs`, `apps/autopilot-deprecated/src/panes/wallet.rs`, related deprecated panes | Shows old Spark wallet balance, invoice, send, and replay surfaces. | `legacy-read` | Remove or quarantine when the deprecated desktop shell no longer needs old wallet inspection. |
| Provider admin payout mirror | `crates/openagents-provider-substrate/src/admin.rs` | Stores provider payout summaries from Nexus snapshots. Not Spark-specific except the upstream payload may reference Spark-era payment ids. | `legacy-read` | Keep as generic payout mirror; ensure future LDK payloads do not use Spark-specific labels. |

## Remaining Touchpoint Counts

Current source/config touchpoints by marker:

- `ldk-replace`: 8 areas
- `final-drain-only`: 4 areas
- `legacy-read`: 5 areas
- `delete-after-cutover`: 1 area

The counts are areas, not raw `rg` matches. Raw matches include tests,
historical comments, and old receipt JSON.

## Invariants for Later Issues

- Do not add a Spark/LDK rail selector to user-facing product flows.
- Do not create new Spark payout target rows from default Pylon registration.
- Do not create new live Spark funding targets unless the final-drain flag is
  explicitly set.
- LDK is the only active target for new funding invoices, Pylon payment-target
  registration, and accepted-work payout dispatch.
- Old Spark receipts and old Spark wallet state must remain inspectable until
  the final migration/drain report is checked in.

## Follow-Up Issues

- `#4481` should add the typed LDK treasury provider boundary.
- `#4482` should add LDK-compatible operation and receipt rows.
- `#4487` should replace Pylon Spark payout-target registration.
- `#4492` should remove Spark from new operations after LDK funding and payout
  paths are live.
- `#4493` should write the final Spark migration and drain report.
