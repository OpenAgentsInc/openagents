# Spark Migration Final Drain Report

Issue: `OpenAgentsInc/openagents#4493`

Date: 2026-05-16

## Summary

Spark is no longer a normal Nexus or Pylon payment rail for new operations.
Nexus v0.2 and Pylon v0.2 are LDK-first for new funding material, payout
dispatch, and provider payment-target registration.

No Spark final drain was performed during this report. The hosted Nexus status
surface was checked after recovery from a public reachability incident and the
live wallet reported:

- `wallet_runtime_status=connected`
- `wallet_storage_runtime_mode=original`
- `wallet_balance_sats=159075`
- `payout_loop_health=degraded`
- `degraded_reason=continuity_alert:dispatch_stalled`

Those remaining funds are not proof that a Spark final drain is complete or
needed. They are the current hosted treasury state. A final drain requires an
explicit operator action with `NEXUS_TREASURY_PROVIDER=spark_final_drain` and
`NEXUS_SPARK_FINAL_DRAIN_ENABLED=true`; those gates must stay off in normal
runtime.

## Public Nexus Reachability During Closeout

Before writing this report, public Nexus returned Cloudflare `530` / `1033` on:

- `https://nexus.openagents.com/api/stats`
- `https://nexus.openagents.com/healthz`
- `https://nexus.openagents.com/v1/treasury/status`

The repo runbook classifies that as an incident. The VM was running, but IAP
SSH failed and serial logs showed the guest could not reach metadata, DNS, or
Cloudflare edge:

- `connect: network is unreachable` to `169.254.169.254`
- `lookup _v2-origintunneld._tcp.argotunnel.com ... server misbehaving`
- `nexus-cloudflared.service` restart loops

I reset `nexus-mainnet-1` per the Nexus GCP runbook. After reset:

- public `/healthz` returned `200`
- VM-local `http://127.0.0.1:8080/healthz` returned healthy Nexus relay state
- VM-local `http://127.0.0.1:8081/healthz` returned healthy recovery-proxy
  state
- metadata lookup from the VM succeeded
- `nexus-relay`, `nexus-cloudflared`, and `nexus-http-recovery-proxy` were
  active

Public traffic was still routed through `nexus-http-recovery-proxy` after the
reset. That is acceptable as a recovered reachability state, but the next ops
pass should verify whether the tunnel should return to the direct `8080`
origin after the service remains stable.

## Active Workers Advertising Spark Targets

Live status after recovery showed:

- `registered_payout_identities=2407`
- `eligible_online_payout_targets=0`
- sampled `payout_target_identities` rows were Spark-style historical targets
- sampled LDK-compatible target count was `0`

This means the old live population still contains Spark-era payout-target
records. Those records are stale for new paid work. The LDK-15 implementation
makes Spark-only targets ineligible for new accepted-work payouts and requires
new Pylon registration to provide an LDK-compatible target:

- `bolt12_offer`
- `bolt11_invoice`
- `bip353_name`
- `lnurl_pay`

Operators should treat old Spark targets as historical identity/payment
metadata until the associated Pylons upgrade to Pylon v0.2 target registration.

## Old Spark Receipts And Read Path

Old Spark receipts remain readable through the Nexus treasury state and receipt
projection path. The retained read path exists so historical payout and wallet
records can be audited without re-enabling Spark as a normal payment rail.

Retained readers:

- `TreasuryState::migrate_legacy_spark_operations`
- historical payout records projected into provider-neutral operation rows
- historical payout ledger and public stats fields
- recovery reports under `docs/reports/nexus/`
- the `crates/spark` wrapper for explicit recovery/final-drain inspection
- deprecated Autopilot wallet panes retained only for old-state inspection

The retained read path must not create new Spark invoices, Spark addresses, or
Spark sends unless the final-drain gates are deliberately enabled.

## Removed Or Blocked Spark Primary Paths

LDK-15 made these normal-path changes:

- `NEXUS_TREASURY_PROVIDER=ldk` is the default.
- `NEXUS_TREASURY_PROVIDER=spark` is rejected.
- Spark can only be configured through `spark_final_drain`.
- `spark_final_drain` requires `NEXUS_SPARK_FINAL_DRAIN_ENABLED=true`.
- Standard funding-target creation routes through the LDK provider boundary.
- Standard funding-target responses leave `spark_invoice` empty.
- Standard payout dispatch routes through the LDK provider boundary.
- Spark dispatch fails closed unless explicit final-drain mode is enabled.
- Nexus API payout-target registration rejects `spark_address` by default.
- Pylon no longer creates a Spark destination on startup.

LDK-15 proof is recorded in the closeout comment:

- `https://github.com/OpenAgentsInc/openagents/issues/4492#issuecomment-4467240698`

The code proof included:

- `cargo fmt -p nexus-control -p pylon --check`
- `cargo test -p nexus-control provider_payout_target_registration_rejects_spark_by_default --lib`
- `cargo test -p nexus-control spark_final_drain_provider_is_disabled_without_explicit_flag --lib`
- `cargo test -p pylon legacy_spark_write_gate_is_default_off_for_normal_runtime --lib`
- grep review of Spark creation/send/registration paths
- `git diff --check`

## Retained Historical Readers And Deletion Conditions

Keep these only until final Spark signoff is complete:

- `crates/spark`
- Spark wallet recovery scripts
- Spark receipt/state migration code
- Spark final-drain config gates
- archived Spark runbooks
- old recovery reports and receipt JSON
- deprecated desktop wallet inspection panes

Deletion is safe only after all of these are true:

- an operator confirms no Spark funds need draining
- old Spark receipts have been exported or retained in immutable report form
- no active Pylon depends on Spark address registration
- no normal admin/API/chat workflow references Spark as a new-operation path
- LDK funding and payout proofs remain green on production
- rollback has been explicitly defined as LDK rollback, not Spark reactivation

## Runbook Status

Normal operator docs now point at:

- `docs/nexus-treasury.md`
- `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md`
- `docs/2026-05-15-ldk-nexus-treasury-transition-audit.md`

Legacy Spark docs are retained only as historical/final-drain references:

- `docs/deploy/NEXUS_TREASURY_FUNDING_INVOICE_RUNBOOK.md`
- `docs/deploy/NEXUS_GCP_RUNBOOK.md` sections that predate LDK v0.2
- `docs/deploy/PYLON_NEXUS_EARNING_RELEASE_RUNBOOK.md` Spark payout sections

Those legacy runbooks must not be used for ordinary funding, payout, or Pylon
registration work.

## Rollback Limits

Rollback after LDK cutover does not mean reintroducing Spark as a normal rail.
Allowed rollback actions are:

- pause payout dispatch
- route funding/payout through the LDK local harness in tests
- roll back a bad Nexus binary or LDK Server deployment
- restore LDK Server state from backup
- use explicit final-drain gates for one reviewed Spark recovery action

Not allowed:

- restoring `NEXUS_TREASURY_PROVIDER=spark` as a product mode
- allowing new Spark payout-target registration by default
- allowing Pylon startup to auto-create Spark destinations
- using Spark as a fallback for normal accepted-work payouts

## Operator Signoff Criteria

Spark migration can be signed off only when an operator has verified:

- public Nexus `/healthz`, `/api/stats`, and `/v1/treasury/status` are
  reachable
- LDK funding target creation works in production or the approved production
  proof environment
- LDK payout dispatch works for upgraded Pylon v0.2 workers
- all active paid workers have LDK-compatible payout targets or are knowingly
  ineligible
- no final Spark drain is needed, or a final drain was performed and recorded
  with explicit receipts
- old Spark receipt exports are retained
- the Spark final-drain gates remain disabled in normal runtime
- runbooks and issue comments do not instruct agents to use Spark for new
  operations

## Closeout Result

Spark is decommissioned from new operations. Historical Spark records remain
readable. No final drain was performed in this issue. Spark final-drain
capability remains disabled by default and should be deleted after operator
signoff confirms old Spark state no longer needs live wallet inspection.
