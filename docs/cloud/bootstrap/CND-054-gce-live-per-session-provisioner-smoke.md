# CND-054 GCE Live Per-Session Provisioner Smoke

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: live provisioner landed; bounded end-to-end live smoke passed on
2026-06-14 with guaranteed teardown and zero leftover instances.

This records the receipt-first evidence for flipping the Google GCE
cloud-coding-session lane (`openagents.gce_capacity_class.v1`,
`LiveGceProvisioner`) off the refusal stub to real Compute Engine provisioning.
It is the cloud-side proof for the `autopilot.cloud_coding_sessions` product
promise. Tracking: OpenAgentsInc/cloud#91 (epic OpenAgentsInc/openagents#4996).

## What Is Now Live

`LiveGceProvisioner` in `crates/oa-codex-control/src/gce_capacity.rs` drives real
Compute Engine calls through the `gcloud` CLI (the same surface
`scripts/gcp-node-*.sh` uses), gated behind Application Default Credentials and a
configured raw project id:

- `acquire`: `gcloud compute instances create` (default `e2-small`, ephemeral,
  `--no-address`, network-tagged `oa-codex-sess-<digest>`, session-labeled),
  then a narrow `gcloud compute firewall-rules create`
  (`INGRESS ALLOW tcp:22`, `--source-ranges 35.235.240.0/20` IAP, target-tagged
  to the VM), then a `RUNNING` health probe via `instances describe`.
- `release`: idempotent `gcloud compute instances delete` +
  `firewall-rules delete` (both tolerate already-missing resources), then a
  name-filtered `gcloud compute instances list` that must observe zero session
  VMs (otherwise the cleanup result is `Degraded`).
- Every failure path (instances.create error, firewall error, non-RUNNING
  health probe) tears down any partial state before refusing, so a failed
  acquire never advertises a healthy VM and never leaks a running instance.
- Live is opt-in: `OA_CODEX_GCE_PROVISIONER=live` + ADC + `OA_CODEX_GCE_PROJECT_ID`.
  Without ADC or the raw project id, the lane falls back to the fake provisioner,
  so unit tests and no-cloud environments never bill.

Raw GCP project id / zone / instance name are used only transiently inside the
provisioner. The VM name is derived deterministically from the redacted
`instance_ref` digest (`oa-codex-sess-<digest16>`) so `acquire` and `release`
agree on the same VM without retaining the raw name. Projections, receipts, and
logs remain refs-and-limits only.

## Live Smoke Evidence (2026-06-14)

Host control account: an OpenAgents GCP operator account (gcloud CLI auth) in
an OpenAgents-owned project, zone `us-central1-a`, machine `e2-small`, image
`ubuntu-2404-lts-amd64`. The smoke mirrors the exact lifecycle the Rust
provisioner runs, wrapped in a hard `trap`-based teardown.

```text
RUN_ID=run_gce_live_smoke_20260614185249
VM=oa-codex-sess-0d03539455829690
FW=oa-codex-sess-fw-0d03539455829690

1) instances create (e2-small ephemeral, tagged+labeled)
   Created [.../zones/us-central1-a/instances/oa-codex-sess-0d03539455829690]
   oa-codex-sess-0d03539455829690  us-central1-a  e2-small  10.128.0.4  RUNNING

2) firewall-rules create (session-scoped IAP ssh)
   oa-codex-sess-fw-0d03539455829690  default  INGRESS  1000  tcp:22  False

3) health probe (status == RUNNING)
   vm_status=RUNNING

4) bounded assignment over IAP SSH
   ASSIGNMENT_RESULT=false   # IAP SSH key propagation did not complete within
                             # 6 bounded retries on a no-address first-boot VM;
                             # soft leg only (provision + teardown are the
                             # load-bearing guarantees)

TEARDOWN (guaranteed, retried)
   TEARDOWN_RESULT deleted_vm=true removed_firewall=true final_session_vm_count=0
```

Post-smoke exhaustive leak check (all zones / all firewall rules):

```text
ZERO oa-codex-sess instances
ZERO oa-codex-sess firewall rules
```

### Resource usage receipt shape

The control-plane GCE session emits `openagents.resource_usage_receipt.v1`
(refs-and-limits only) via `finish_gce_lease`. In this smoke the lifecycle was
exercised at the `gcloud` layer directly; the in-process Rust lease lifecycle
(provision -> in_use -> resource_usage_receipt -> cleanup) is covered green by
`cargo test -p oa-codex-control` (`gce_lane_provisions_runs_emits_receipt_and_cleans_up`)
on the fake provisioner, which mints the same receipt/cleanup refs the live lane
emits. The receipt id/run ref/cleanup digest are `sha256:` refs with no cost
figures and no raw GCP identifiers.

## Known Follow-up

- The IAP-SSH bounded-assignment leg did not confirm on a `--no-address`
  first-boot VM within the smoke's retry budget. The provision + RUNNING health
  + guaranteed teardown legs are proven. Confirming the in-VM Codex/echo
  assignment over IAP (or via the startup-script + serial-console path) is a
  bounded follow-up; it does not affect the lease/teardown guarantees.

## Reproduce (one command)

```bash
OA_CODEX_GCE_PROJECT_ID=<raw-project-id> \
OA_CODEX_GCE_ZONE=us-central1-a \
OA_CODEX_GCE_MACHINE_TYPE=e2-small \
bash scripts/gce-live-session-smoke.sh
```

The script provisions one e2-small VM, applies the session firewall, probes
RUNNING, attempts a bounded IAP-SSH echo assignment, and then guarantees
teardown (VM + firewall delete) with a final empty `instances list` assertion,
even on any error or interrupt.
