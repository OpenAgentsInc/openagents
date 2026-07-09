# `openagents.gce_capacity_class.v1`

Status: live (lease lifecycle + real GCE provisioner; cloud#88 leg landed)

The lease state machine (`acquire -> ready -> in_use -> release`), reconciliation
labels, provision/cleanup receipts, TTL/idle caps, and idempotent release are
implemented in `crates/oa-codex-control/src/gce_capacity.rs` and wired into the
`cloud-gcp` placement lane in `crates/oa-codex-control/src/main.rs`
(`acquire_gce_lease_for_run` / `finish_gce_lease`). A `cloud-gcp` placement now
provisions a per-session VM lease, runs the Codex runner for the assignment,
emits an `openagents.resource_usage_receipt.v1`, then releases and mints a
cleanup receipt.

Real Compute Engine calls are gated behind `OA_CODEX_GCE_PROVISIONER`:

- `fake` (default) — dry-run provisioner that produces deterministic refs and
  keeps execution on the local control host. Used by unit tests and any
  no-cloud environment.
- `live` — gated behind Application Default Credentials **and** a configured raw
  project id. The live provisioner shells the `gcloud` CLI (the same surface the
  `scripts/gcp-node-*.sh` bootstrap lane uses) to run real Compute Engine calls:
  `instances create` (smallest reasonable machine, default `e2-small`,
  ephemeral, `--no-address`, network-tagged, session-labeled for
  reconciliation), a `RUNNING` health probe, a narrow session-scoped
  `firewall-rules create` (IAP-sourced `tcp:22`, target-tagged to the VM), then
  on release `instances delete` + `firewall-rules delete` (idempotent, tolerant
  of already-missing resources) followed by a label/name-filtered
  `instances list` that must observe zero leftover session VMs. If ADC or the
  raw project id is absent, or any acquire/health step fails, the provisioner
  tears down any partial state and refuses so the lane falls back to the fake
  provisioner (and lane resolution falls back to SHC when GCE is marked
  unavailable). Raw GCP project id / zone / instance name are used only
  transiently inside the provisioner and never retained in projections,
  receipts, or logs; the deterministic VM name is derived from the redacted
  `instance_ref` digest so acquire and release agree without retaining it.

Enable the live lane on a control host that has `gcloud` + ADC:

```bash
gcloud auth application-default login   # or set GOOGLE_APPLICATION_CREDENTIALS
export OA_CODEX_GCE_PROVISIONER=live
export OA_CODEX_GCE_PROJECT_ID=<raw-project-id>          # required for live
export OA_CODEX_GCE_PROJECT_REF=<redacted-project-ref>   # retained ref only
# optional overrides:
export OA_CODEX_GCE_ZONE=us-central1-a
export OA_CODEX_GCE_MACHINE_TYPE=e2-small
export OA_CODEX_GCE_IMAGE_FAMILY=ubuntu-2404-lts-amd64
export OA_CODEX_GCE_IMAGE_PROJECT=ubuntu-os-cloud
```

A bounded end-to-end live smoke (provision a real VM, run a trivial bounded
assignment, then guaranteed teardown + empty `instances list`) is recorded under
`docs/bootstrap/`.

This contract describes a GCE-backed ephemeral capacity class for managed
OpenAgents Cloud sessions. It lets Cloud provision one bounded Compute Engine
VM for a workroom or benchmark session, attach only the bootstrap material
needed to reach it, and destroy the VM when the lease is released or expires.

This is private managed-cloud infrastructure. Public contributor Pylon should
only see refs, digests, and public-safe capability shape; it must not learn raw
GCP project ids, instance names, external IPs, credentials, firewall topology,
or placement details.

## Purpose

`openagents.gce_capacity_class.v1` gives Cloud a refs-only contract for:

- acquiring an ephemeral Compute Engine VM in an OpenAgents-owned GCP project;
- declaring SSH metadata, managed firewall policy, labels, and reconciliation
  refs without retaining raw cloud identifiers;
- tracking the lease state from `acquire` to `ready`, `in_use`, and `release`;
- enforcing TTL and idle-timeout cleanup;
- minting receipt refs for provisioning, readiness, use, release, and cleanup.

The class is compute capacity, not open-ended labor. Workroom assignment,
runtime profile, policies, resource limits, expected outputs, and receipt
semantics must already be declared by the caller contract.

## Capacity Class Fields

| Field | Purpose |
| --- | --- |
| `capacity_class_id` | Stable policy id such as `gce.ephemeral.standard.v1`. |
| `contract_version` | Must be `openagents.gce_capacity_class.v1`. |
| `owner_ref` | Cloud-owned org or fleet authority ref. |
| `gcp_project_ref` | Redacted OpenAgents GCP project ref, not a raw project id. |
| `provisioner_identity_ref` | Runtime identity ref for Application Default Credentials. |
| `zone_or_region_ref` | Redacted placement ref suitable for reconciliation. |
| `machine_profile_ref` | Ref to CPU, memory, disk, image, and accelerator policy. |
| `boot_image_ref` | Image family, template, or digest ref with no raw project id. |
| `network_policy_ref` | Ref to the managed VPC/subnet/firewall policy. |
| `ssh_metadata_ref` | Ref to the SSH metadata bundle installed on the VM. |
| `labels` | Bounded reconciliation labels described below. |
| `ttl_ms` | Maximum lease lifetime from accepted acquire. |
| `idle_timeout_ms` | Maximum ready or in-use idle period before release. |
| `receipt_sink_ref` | Ref where lease and cleanup receipts are written. |

## Provisioning

Cloud provisions the VM with Google Application Default Credentials from the
provisioner process. ADC is an execution-time credential source only. Retained
state records `provisioner_identity_ref` and receipt digests, not ADC tokens,
service-account keys, raw OAuth material, or metadata-server credentials.

The provisioner must create a Compute Engine VM in an OpenAgents-owned GCP
project selected by `gcp_project_ref`. The retained projection records only:

```json
{
  "contract_version": "openagents.gce_capacity_class.v1",
  "lease_ref": "gce-lease://cloud/session/example",
  "capacity_class_id": "gce.ephemeral.standard.v1",
  "gcp_project_ref": "gcp-project-ref://openagents/cloud-primary",
  "instance_ref": "gce-instance-ref://sha256/example",
  "network_policy_ref": "gce-network-policy-ref://sha256/example",
  "provision_receipt_ref": "sha256:provision-example"
}
```

Raw GCP project ids, numeric project numbers, instance self-links, instance
names, IP addresses, SSH private keys, access tokens, and firewall rule names
must not be retained in projections or normal logs.

## SSH Metadata

The provisioner may attach SSH metadata required for the managed bootstrap and
health check path. The metadata bundle is referenced by `ssh_metadata_ref` and
must be session scoped.

Retained state may record:

- `ssh_metadata_ref`;
- public-key fingerprint digest;
- bootstrap user ref;
- expiry timestamp;
- provisioning receipt digest.

Retained state must not record SSH private keys, raw authorized-key material,
metadata-server access tokens, or generated host credentials. SSH metadata must
expire no later than the VM lease TTL.

## Managed Firewall Rule

The VM must be protected by a Crabbox-style managed firewall rule: narrow,
session scoped, labeled, and owned by the capacity lease. The rule admits only
the bootstrap and workroom access path required by policy, and it must be
removed during release.

The retained projection records:

| Field | Purpose |
| --- | --- |
| `firewall_policy_ref` | Ref to the managed firewall intent. |
| `firewall_rule_ref` | Redacted managed rule ref for reconciliation. |
| `access_source_ref` | Ref to the approved source or tunnel policy. |
| `ports_profile_ref` | Ref to the bounded bootstrap/workroom port profile. |
| `firewall_receipt_ref` | Digest of the firewall apply or cleanup receipt. |

Raw source IPs, destination IPs, CIDR ranges, rule names, network self-links,
and private topology markers are forbidden in retained projections.

## Reconciliation Labels

Every managed VM and managed firewall rule must carry labels sufficient for
reconciliation and garbage collection. Labels are bounded non-secret strings.

Required label keys:

```text
openagents-managed
openagents-contract
openagents-capacity-class
openagents-lease-ref
openagents-workroom-ref
openagents-owner-ref
openagents-ttl-expires
```

Label values may include refs, short enum values, and normalized timestamps.
They must not include raw user data, raw GCP project ids, IP addresses,
credentials, private topology details, wallet material, or bearer tokens.

## Lease Lifecycle

The lease state machine is explicit and receipt bearing:

```text
acquire -> ready -> in_use -> release
```

`acquire` accepts a capacity request, checks policy, creates the VM, applies SSH
metadata, applies the managed firewall rule, writes labels, and emits a
provisioning receipt ref.

`ready` means the VM exists, labels and firewall policy are applied, bootstrap
health passed, and no workroom has started using it. Ready leases are subject
to `idle_timeout_ms`.

`in_use` means the VM is attached to one declared workroom, benchmark task, or
Cloud session. In-use leases remain subject to both `ttl_ms` and
`idle_timeout_ms`; activity refreshes idle state only through explicit,
redacted heartbeat receipts.

`release` is terminal for the capacity lease. Release deletes the managed VM,
removes the managed firewall rule, revokes session SSH metadata, and mints a
cleanup receipt ref.

Failed acquire or readiness checks must degrade or refuse capacity instead of
advertising a healthy VM. They must emit a failure receipt or health event with
redacted refs only.

## TTL And Idle Timeout

`ttl_ms` starts when acquire is accepted. A lease that exceeds TTL must be
released even if no caller explicitly asks for release.

`idle_timeout_ms` applies to `ready` and `in_use` leases. A ready lease with no
attached session, or an in-use lease with no accepted heartbeat before the idle
deadline, must be released.

Timeout-driven release follows the same idempotent release behavior as a manual
release and must mint a cleanup receipt ref.

## Release And Cleanup Receipt

Release is idempotent. Repeating release for the same `lease_ref` must not
recreate cloud resources, must tolerate already-missing VM or firewall
resources, and must return the existing cleanup receipt ref when cleanup already
completed.

The cleanup receipt records:

| Field | Purpose |
| --- | --- |
| `lease_ref` | Redacted capacity lease ref. |
| `instance_ref` | Redacted instance ref. |
| `workroom_ref` | Redacted workroom or session ref, when attached. |
| `release_reason` | `manual`, `ttl_expired`, `idle_timeout`, `failed_acquire`, `policy`, or `reconciler_gc`. |
| `deleted_vm` | Boolean deletion outcome. |
| `removed_firewall_rule` | Boolean firewall cleanup outcome. |
| `revoked_ssh_metadata` | Boolean SSH metadata cleanup outcome. |
| `cleanup_started_at` / `cleanup_completed_at` | Timestamps. |
| `result` | `completed`, `already_clean`, or `degraded`. |
| `receipt_digest` | Local `sha256:` digest over cleanup receipt material. |

Cleanup receipts must not contain raw GCP project ids, VM names, IP addresses,
network links, service-account emails, credentials, SSH keys, or private
topology markers.

## Retained Projection

Cloud may retain a compact lease projection for status and reconciliation:

| Field | Purpose |
| --- | --- |
| `lease_ref` | Stable capacity lease ref. |
| `state` | `acquire`, `ready`, `in_use`, or `release`. |
| `capacity_class_id` | Capacity policy id. |
| `workroom_ref` | Redacted attached workroom/session ref, nullable before use. |
| `instance_ref` | Redacted GCE instance ref. |
| `gcp_project_ref` | Redacted project ref. |
| `firewall_rule_ref` | Redacted managed firewall ref. |
| `ssh_metadata_ref` | Redacted SSH metadata ref. |
| `expires_at` | TTL deadline. |
| `idle_deadline_at` | Idle timeout deadline. |
| `latest_receipt_ref` | Latest lease or cleanup receipt digest. |
| `cleanup_receipt_ref` | Cleanup receipt digest after release. |

This projection is refs-only. It is suitable for Vortex, Forge, Nexus, or
Probe-facing status only after the same forbidden-secret and private-topology
checks used by other Cloud contracts.

## Validation Rules

- `contract_version` must be `openagents.gce_capacity_class.v1`.
- `capacity_class_id`, refs, labels, and enum fields must be bounded
  non-secret strings.
- `gcp_project_ref` must be a redacted ref, never a raw GCP project id or
  project number.
- ADC may be used only at provisioning time; retained state must not include
  raw ADC tokens, service-account keys, OAuth material, or metadata-server
  credentials.
- `ttl_ms` and `idle_timeout_ms` must be positive and bounded by Cloud fleet
  policy.
- A lease must not enter `ready` until VM creation, SSH metadata, managed
  firewall policy, labels, and bootstrap health have all succeeded.
- A lease must not enter `in_use` without a declared workroom, benchmark task,
  or Cloud session ref.
- Release must be idempotent and must mint or return a cleanup receipt ref.
- Managed VM, firewall, and SSH metadata refs must be sufficient for private
  reconciliation while remaining safe for retained projections.
- Retained projections must reject raw GCP project ids, IP addresses, instance
  names, self-links, CIDR ranges, credentials, SSH keys, wallet material,
  bearer tokens, customer data, and private topology markers.
