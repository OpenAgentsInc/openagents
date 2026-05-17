# Spark Removal Closeout

Issue: `OpenAgentsInc/openagents#4500`

Date: 2026-05-16

## Summary

Spark is not an active Nexus or Pylon payment rail. The final product state is
LDK-only for Nexus funding, accepted-work payouts, and Pylon payout-target
registration. There is no Spark provider mode, Spark drain mode, Spark funding
path, Spark payout path, or Spark fallback in the active Nexus provider parser.

No Spark drain was performed during this closeout. That is deliberate. Keeping a
Spark drain mode in the production runtime would preserve the old failure path.
The active codebase no longer exposes a spendable Spark wallet surface to drain
from, and the old Spark-only payout targets are stale/ineligible for new paid
work.

## Current Source Truth

The active source state is:

- `TreasuryLightningProviderKind` accepts only `ldk`.
- `NEXUS_TREASURY_PROVIDER=spark` and every other non-LDK provider value are
  invalid.
- Normal funding target creation goes through the LDK provider boundary.
- Normal payout dispatch goes through the LDK provider boundary.
- Nexus rejects `spark_address` payout-target registration.
- Pylon startup does not create a Spark destination.
- Nexus deploy staging omits `crates/spark` and must reject Spark SDK packages
  in the staged build context.

Historical Spark strings still appear in audit documents, archived reports,
deprecated desktop surfaces, old receipt migration tests, and stale state
fixtures. Those are not operator instructions and must not be copied into active
deployment or payment workflows.

## Live Reachability At Closeout

During this issue, public Nexus returned Cloudflare `530` for:

- `https://nexus.openagents.com/healthz`
- `https://nexus.openagents.com/api/stats`
- `https://nexus.openagents.com/v1/treasury/status`

Because the public Nexus endpoint was unreachable, this issue did not claim a
fresh live balance reading. The earlier historical report recorded a hosted
treasury balance, but that value was a Nexus treasury status value, not proof of
separate recoverable Spark funds.

The closeout decision is based on source and runtime interface truth: the active
Nexus code no longer contains a Spark payment provider. If an external Spark
wallet exists outside the active Nexus/Pylon runtime, it is outside this repo's
production path and must not be reintroduced into Nexus to recover it.

## Drain Decision

Remaining Spark funds are classified as not recoverable through the active
OpenAgents production runtime.

Rationale:

- There is no active Spark provider path to create invoices or sends.
- There is no active Spark drain provider value.
- Production Nexus should not be rebuilt with Spark SDK dependencies to chase
  hypothetical residual funds.
- Old Spark-only worker targets cannot receive accepted-work payouts.
- Reintroducing Spark to chase residual funds would violate the LDK-only target
  state and recreate the latency/failure mode that prompted the migration.

No movement receipt exists because no money movement was executed. Future
operator work should focus on LDK treasury funding, channel liquidity, payout
smoke, backup/restore, and public Nexus reachability.

## Historical Receipt Handling

Old Spark-era receipts remain readable only as historical records. The retained
read path exists to audit previous payout rows without enabling Spark writes:

- migrated old payout rows in provider-neutral operation history;
- historical receipt JSON and reports under `docs/reports/nexus/`;
- deprecated desktop/source references that are not part of Nexus deploy
  staging.

Do not add new Spark migration code. If historical receipts need to be exported,
export them from the existing provider-neutral records and archive the export.

## Deletion Checklist

Spark-related code and documentation may be deleted or quarantined when the
following are true:

- staged Nexus deploy context has no Spark SDK packages;
- active Pylon registration remains LDK-compatible only;
- active public/operator docs mention Spark only as historical incident context;
- old receipt exports are retained if operators still need them;
- deprecated desktop wallet surfaces are either removed or explicitly isolated
  from Nexus/Pylon production work.

The normal active path must stay LDK-only. Do not add a Spark selector, Spark
fallback, Spark recovery flag, or Spark drain flag.

## Verification Commands

These commands establish the current active state:

```bash
rg -n 'Spark.*Drain|spark_.*drain|NEXUS_SPARK_.*DRAIN' \
  apps/nexus-control/src crates scripts/deploy/nexus

rg -n 'NEXUS_TREASURY_PROVIDER=spark|provider=spark' \
  apps/nexus-control/src crates scripts/deploy/nexus

tmp_context="$(mktemp -d /tmp/openagents-nexus-build-context.XXXXXX)"
scripts/deploy/nexus/stage-build-context.sh "$tmp_context" >/dev/null
rg -n 'openagents-spark|breez-sdk-spark|spark-wallet|name = "spark"|breez/spark-sdk' \
  "$tmp_context" -S
```

The first two searches should return no active runtime hits. The staged-context
search must return no rows.
