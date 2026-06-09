# Spark Touchpoint Inventory for LDK Transition

Issue: `OpenAgentsInc/openagents#4480`

Date: 2026-05-16

## Purpose

Nexus and Pylon are LDK-only for active payment work. Spark remains only as:

- historical receipt reads,
- old wallet/status inspection outside the production deploy path,
- deprecated source material that must not be copied into Nexus/Pylon runtime.

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

The normal code path has no Spark write material:

- Pylon no longer auto-creates a Spark payout destination.
- Nexus no longer accepts Spark payout-target registration in the active API.
- Nexus no longer creates live Spark-backed funding targets.
- Nexus accepts only the LDK treasury provider.
- Simulated/local proof treasury remains available for tests and proof
  fixtures.

Do not add a Spark product mode, fallback, recovery flag, or drain flag.

## LDK-Only Deploy Guard

`scripts/deploy/nexus/test-ldk-deploy-invariants.sh` is the canonical guard
for normal Nexus/Pylon production paths. It runs in the standard Nexus image
build, warm-builder build, and deploy verification scripts.

The guard fails if:

- the staged Nexus build context includes Spark SDK/package dependencies;
- active Nexus/Pylon source reintroduces a Spark drain/provider symbol;
- active Nexus/Pylon scripts expose a Spark provider selector.

Allowed Spark references are limited to:

- historical reports and audits under `docs/reports/`;
- the excluded `crates/spark` source while deprecated non-Nexus surfaces still
  need old-state inspection;
- old receipt migration tests and stale fixtures that prove Spark targets are
  rejected or remain read-only;
- deprecated desktop wallet panes until that shell is removed or quarantined.

Those allowed references are not runtime compatibility paths. Do not expand
the allowlist; delete callers instead.

## Classification Markers

- `legacy-read`: keep only so old state, receipts, and operator docs remain
  readable.
- `ldk-replace`: replace with LDK v0.2 implementation.
- `delete-after-cutover`: remove after LDK is live and Spark state is drained or
  archived.

## Touchpoint Table

| Area | Files | Current behavior | Marker | Deletion condition |
| --- | --- | --- | --- | --- |
| Spark wallet wrapper crate | `crates/spark/Cargo.toml`, `crates/spark/src/lib.rs`, `crates/spark/src/wallet.rs`, `crates/spark/src/signer.rs`, `crates/spark/src/error.rs`, `crates/spark/tests/tree_node_status.rs` | Excluded legacy crate. Must not be copied into Nexus deploy staging. | `legacy-read` and `delete-after-cutover` | Delete after old Spark receipt/state inspection has a replacement archive path and deprecated desktop surfaces no longer require it. |
| Pylon wallet runtime | `apps/pylon/src/wallet_runtime.rs`, `apps/pylon/src/ledger.rs` | Opens local Spark wallet, creates Spark/Bitcoin receive addresses, creates Bolt11 invoices, pays invoices, records wallet ledger fields. | `ldk-replace`; ledger fields are `legacy-read` | Replace with LDK payment target and local wallet/account view. Keep old ledger fields only while old Pylon state must be inspected. |
| Pylon default online earning loop | `apps/pylon/src/lib.rs` | Previously auto-created a local Spark payout destination when a Pylon went online without a configured settlement destination. | `ldk-replace` | Replaced with LDK payment-target registration. There is no active Spark auto-create fallback. |
| Nexus treasury state and dispatch | `apps/nexus-control/src/treasury.rs` | Stores payout targets, payout records, receive records, historical wallet state, dispatch plans, old provider payment ids, and wallet error/degraded states. | `ldk-replace`; old records are `legacy-read` | Replace active operation rows and dispatch calls with LDK operation/receipt storage. Keep old rows read-only only where archived receipts still need inspection. |
| Nexus payout-target registration API | `apps/nexus-control/src/lib.rs`, `apps/nexus-control/src/treasury.rs`, `crates/openagents-provider-substrate/src/payout_target.rs` | Issues payout-target challenges and registers provider payment targets signed by provider identity. | `ldk-replace` | Replaced with Pylon v0.2 LDK payment-target registration. Spark registration is rejected in the active API. |
| Nexus funding-target API and CLI | `apps/nexus-control/src/lib.rs`, `apps/nexus-control/src/treasury.rs` | Creates LDK funding invoices through the provider boundary. | `ldk-replace` | Keep Spark creation out of the active API and deploy image. |
| Nexus deploy image and build context | `apps/nexus-relay/Dockerfile`, `scripts/deploy/nexus/stage-build-context.sh`, `apps/nexus-relay/deploy/Cargo.nexus.lock` | Previously shipped `openagents-spark`; current staged builds are guarded to reject it. | `delete-after-cutover` | Remove any returning caller immediately; the active build context must stay LDK-only. |
| Nexus recovery/watchdog scripts | `scripts/deploy/nexus/09-recover-treasury-wallet.sh`, `scripts/deploy/nexus/10-install-treasury-watchdog.sh`, `scripts/deploy/nexus/test-recover-treasury-wallet-shell-guards.sh` | Historical Spark-era recovery scripts. | `delete-after-cutover` | Replace with LDK node recovery/backup/watchdog paths and remove Spark wallet inspection from the normal operator path. |
| Nexus public stats/homepage projection | `apps/nexus-relay/src/homepage_template.html`, `apps/nexus-relay/src/durable.rs`, `apps/nexus-control/src/lib.rs`, `apps/nexus-control/src/economy.rs` | Projects treasury and payout totals, including Spark-backed payout and wallet degraded state fields. | `ldk-replace`; totals are `legacy-read` | Replace field descriptions and data source with LDK operation/receipt projection. Preserve historical totals in archived reports. |
| Pylon release/bootstrap/docs | `packages/pylon-bootstrap/**`, `docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md`, `docs/2026-04-21-run-pylon-get-paid-for-training.md`, `docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md` | Documents and releases Spark-backed Pylon earning setup. | `ldk-replace` | Update after Pylon v0.2 LDK target registration ships. |
| Treasury runbooks | `docs/nexus-treasury.md`, `docs/deploy/NEXUS_GCP_RUNBOOK.md`, `docs/deploy/NEXUS_TREASURY_FUNDING_INVOICE_RUNBOOK.md` | Documents LDK funding, payout, recovery, degraded states, and deploy guards. | `ldk-replace` | Keep Spark references historical only; do not include Spark recovery/drain instructions in active runbooks. |
| Legacy desktop wallet UI | `apps/autopilot-deprecated/src/spark_wallet.rs`, `apps/autopilot-deprecated/src/spark_pane.rs`, `apps/autopilot-deprecated/src/panes/wallet.rs`, related deprecated panes | Shows old Spark wallet balance, invoice, send, and replay surfaces. | `legacy-read` | Remove or quarantine when the deprecated desktop shell no longer needs old wallet inspection. |
| Provider admin payout mirror | `crates/openagents-provider-substrate/src/admin.rs` | Stores provider payout summaries from Nexus snapshots. Not Spark-specific except the upstream payload may reference Spark-era payment ids. | `legacy-read` | Keep as generic payout mirror; ensure future LDK payloads do not use Spark-specific labels. |

## Remaining Touchpoint Counts

Current source/config touchpoints by marker:

- `ldk-replace`: 8 areas
- `legacy-read`: 5 areas
- `delete-after-cutover`: 2 areas

The counts are areas, not raw `rg` matches. Raw matches include tests,
historical comments, and old receipt JSON.

## Invariants for Later Issues

- Do not add a Spark/LDK rail selector to user-facing product flows.
- Do not create new Spark payout target rows from default Pylon registration.
- Do not create new live Spark funding targets.
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
