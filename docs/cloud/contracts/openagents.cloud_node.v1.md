# openagents.cloud_node.v1

Status: ratified scaffold for Cloud MVP issue `CND-001`

This contract is the shared node vocabulary between open-source contributor
Pylon and private managed OpenAgents Cloud nodes.

## Identity

```json
{
  "contract_version": "openagents.cloud_node.v1",
  "identity": {
    "node_id": "oa-node.gcp.test.example",
    "operator_identity": "org.openagents.test",
    "account_or_org_binding": "org.openagents.test",
    "signing_key_ref": "gcp-secret-manager-key-ref"
  }
}
```

## Host

```text
os
arch
cpu
memory
disk
accelerator_inventory
site_or_power_metadata
```

## Lifecycle

```text
desired_mode
observed_status
degradation_reason
service_manager
update_channel
last_started_at
last_heartbeat_at
```

## Capabilities

```text
inference_products
training_products
sandbox_profiles
workroom_capacity
ingress_support
artifact_support
```

## Policy

```text
accepted_work_policy
sandbox_policy
network_policy
filesystem_policy
secret_policy
settlement_policy
```

## Evidence

```text
current_snapshot_digest
health_events
job_receipts
artifact_receipts
payout_or_accounting_receipts
```

## Compatibility Rule

Public Pylon can implement this contract without knowing private cloud fleet
topology. Private `oa-node` can implement this contract without carrying
contributor wallet UX.

## Fixture Set

The executable fixture set lives in `fixtures/cloud_node_v1/`:

- `contributor-pylon.json`
- `managed-oa-node.json`
- `degraded-node.json`

The `openagents-cloud-contract` crate parses and validates all three fixtures.
The contributor fixture is intentionally public-safe so public Pylon can carry
its own compatibility test without depending on the private `cloud` repo.

## Local Managed State

`oa-node init --org <id>` creates local managed-node state under
`$OPENAGENTS_CLOUD_NODE_HOME` or `$HOME/.openagents/cloud/oa-node`. Status JSON
loads that state and projects the identity into this contract. The state keeps a
signing-key reference only; key material and other raw secrets are not tracked.

`admin-store.json` persists desired mode, observed status, inventory/update/
quarantine placeholders, and receipt cursors. `health-events.jsonl` is
append-only. Missing or corrupt admin state degrades the status snapshot instead
of advertising healthy capacity.

## Service Manager

`oa-node service install` records launchd or systemd service-manager intent in
local node state. `start`, `stop`, `restart`, `status`, and `uninstall` are
explicit commands. State-changing commands append `service-events.jsonl` and
health events without raw secret-bearing fields. `oa-node status --json`
projects service manager and observed node health through the lifecycle block.

## Signed Updates

`oa-node update` records signed release-channel policy and update receipts in
local state. Receipts include previous version, target version, signer,
signature digest, result, and receipt digest. Successful applies promote the
current version. Failed applies roll back to the previous version when possible
or quarantine the node when no previous version exists. Fleet policy can pin an
exact version or defer updates; deferred updates become pending updates instead
of changing the current version.

## Quarantine

`oa-node quarantine enter` sets desired and observed node state to
`quarantined`, records a workroom drain policy, appends a health event, and
emits a quarantine receipt. Forge assignment intake refuses new work while the
node is quarantined. The workroom policy is one of `pause`, `migrate`, or
`close`. `oa-node quarantine exit` releases the node back to offline state and
emits a release receipt.

## Settlement Modes

Managed `oa-node` defaults to `no-wallet` settlement. `internal-accounting`
mode records Treasury and Nexus reconciliation refs in
`settlement-receipts.jsonl` and projects the latest accounting receipt digest
into `evidence.payout_or_accounting_receipts`. `contributor-wallet` mode is
rejected in this private Cloud repo and remains public Pylon behavior.

## Capability Broker Redaction

`oa-node broker redact` accepts broker payload kinds for headers, URLs, env,
config, logs, and receipts. Secret-looking fixtures fail unless explicitly
marked fake. Marked fake inputs are redacted before writing artifacts, and
broker redaction receipts contain only digests and redacted artifact paths.

## Sandbox Profile Enforcement

`oa-node sandbox profile register` records declared sandbox profile policy:
profile digest, execution class, network policy, filesystem policy, timeout
limit, artifact byte limit, and secret policy. Registered profiles project into
`capabilities.sandbox_profiles`, and the node reports
`policy.sandbox_policy = profile_enforced` once profiles exist.

Forge sandbox worker assignments must include a matching sandbox policy block.
Missing or mismatched network/filesystem policy is refused with a normal
assignment receipt. Sandbox Psionic receipts must include `profile_digest`.

## Capability Detection

`oa-node detect --json` separates present host hardware from sellable managed
Cloud capability. Status projects detected host facts into `host` and projects
unconfigured backends as `backend_ready=false` and `eligible=false` capability
rows instead of claiming healthy capacity.

## Nexus Registry Adapter

`oa-node nexus register` and `oa-node nexus heartbeat` post a private registry
envelope to Nexus with the current snapshot digest and a ref-bound MVP
signature. The signature records the signing-key reference and digest, not raw
key material.

Accepted Nexus responses may update desired mode. Rejected, stale, expired,
invalid, or unreachable registry responses persist a degraded observed status
and append a Nexus health event so the node remains locally inspectable while
refusing to advertise safe capacity.

## Forge Assignment Evidence

`oa-node forge assignment receive` reads `openagents.forge_assignment.v1` and
emits `openagents.forge_assignment_receipt.v1`. Accepted and refused assignment
receipts are appended locally and projected into `evidence.job_receipts` by
their receipt digest.

The MVP adapter refuses open-ended labor assignments so they route through
Forge/Probe instead of being treated as sandbox compute.

## Psionic Worker Attachment

`oa-node psionic attach` reads `openagents.psionic_worker_attachment.v1` and
projects Psionic readiness into the matching product lanes. A crashed Psionic
worker degrades only its own inference, training, or sandbox product/profile
row.

`oa-node psionic receipt append` emits
`openagents.psionic_execution_receipt.v1`; status projects the local receipt
digest into `evidence.job_receipts`, while the receipt body cites the upstream
`psionic_evidence_digest`.

## Probe Worker Attachment

`oa-node probe attach` reads `openagents.probe_worker_attachment.v1`, which
scopes Probe to one workroom workspace root and explicit capability names while
requiring `raw_secret_access=false`.

`oa-node probe closeout append` emits
`openagents.probe_closeout_receipt.v1`; status projects closeout receipt
digests into `evidence.artifact_receipts`.
