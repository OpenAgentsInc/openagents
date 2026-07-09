# `openagents.compute_quota_routing.v1`

Status: implementation scaffold for `CND-065`

This contract specifies compute metering and quota routing for managed Cloud
sessions. It is the compute-layer analogue of model-account quota routing and
sits alongside — not in place of — the `openagents.resource_usage_receipt.v1`
receipt. It extends that receipt with compute-class metering dimensions,
defines per-owner and per-org session and lease caps, states TTL and
idle-timeout defaults, and introduces a refs-only rejection receipt for the
case where compute cannot be acquired. Actual settlement — invoicing, wallet
deduction, Treasury reconciliation — is handled downstream and is out of scope
here.

## Purpose

Every managed Cloud session consumes infrastructure resources: virtual CPUs,
RAM, GPU time, network egress, and transient storage. These have metered GCP
costs. This contract defines:

- the compute classes and metered dimensions recorded in a session receipt;
- the quota caps that bound concurrent session and remote-lease usage per owner
  and per org;
- the idle-timeout and TTL defaults that cap session lifetime regardless of
  activity;
- the shape of the rejection receipt emitted when compute cannot be acquired;
- the cost-plus-10% principle that converts metered GCP cost into the billing
  input passed to Treasury.

Vortex, Probe, and Treasury use these records to distinguish real infrastructure
consumption from modeled capacity, and to route billing inputs correctly without
exposing raw cost or customer identity in public-facing refs.

## Compute Classes

Each managed session is assigned a compute class at admission time. The class
determines the resource profile and the per-unit GCP rate applied during
metering.

| Class | vCPU | RAM (GiB) | GPU | Typical use |
| --- | --- | --- | --- | --- |
| `micro` | 1–2 | 1–4 | None | Lightweight agent tasks, short CI-like runs |
| `standard` | 2–8 | 4–32 | None | General workroom, benchmark, Codex sessions |
| `compute` | 8–32 | 32–128 | None | Heavy compilation, large CPU-bound inference |
| `gpu-standard` | 4–16 | 16–64 | 1× T4 or L4 | GPU-accelerated inference, small fine-tune |
| `gpu-high` | 8–32 | 64–256 | 1–4× A100 or H100 | Large-scale training, multi-GPU inference |

The class label is set at session start, recorded in every compute metering
record, and propagated into the `host` block of the associated
`openagents.resource_usage_receipt.v1` entry. It may not change during a
session's lifetime.

## Metered Dimensions

Compute metering extends the `openagents.resource_usage_receipt.v1` `run`
block with a `compute_usage` sub-record. Three dimensions are metered.

### VM-Seconds by Class

```text
compute_class           -- class label assigned at admission
vm_seconds              -- wall-clock seconds the VM or microVM was live (u64)
vcpu_count              -- configured vCPU allocation (u16)
ram_gib                 -- configured RAM allocation in GiB (f32)
gpu_type                -- GPU model string, e.g. "nvidia-l4", or null
gpu_count               -- GPU count; 0 for non-GPU classes (u8)
```

`vm_seconds` begins at microVM start — the Firecracker `InstanceStart` event
or the equivalent GCE instance `RUNNING` transition — and ends at the first
of: clean shutdown confirmation, idle-timeout eviction completion, or TTL
expiry. Suspended or paused intervals are excluded when the hypervisor exposes
unambiguous pause and resume events. When pause events are unavailable,
paused time is included conservatively.

### Network Egress

```text
egress_bytes            -- bytes transferred out of the GCP network boundary (u64)
```

Egress is measured at the GCP project level and apportioned per session by the
node agent. Intra-region traffic and Cloud Storage egress covered by GCP
no-charge tiers are excluded. All other outbound bytes are counted. Sessions
on `local` and `shc` provider lanes record `egress_bytes = 0` unless routed
through a GCP NAT.

### Storage

```text
workspace_byte_seconds  -- workspace bytes × seconds the workspace was live (u64)
artifact_byte_seconds   -- artifact bytes × seconds held in managed storage (u64)
log_byte_seconds        -- log bytes × seconds held before rotation (u64)
```

Storage dimensions accumulate from session start to closeout or TTL expiry.
Snapshot bytes are included in `workspace_byte_seconds` for the duration the
snapshot is retained under this session's `run_ref`. Once a snapshot is
released or transferred to long-term artifact storage it transitions to
`artifact_byte_seconds`.

### Compute Usage Record

The complete `compute_usage` sub-record within a resource_usage_receipt:

| Field | Purpose |
| --- | --- |
| `compute_class` | Class label assigned at admission. |
| `vm_seconds` | Metered VM wall-clock seconds. |
| `vcpu_count` | vCPU allocation for the session. |
| `ram_gib` | RAM allocation in GiB. |
| `gpu_type` | GPU model string, or `null` for non-GPU classes. |
| `gpu_count` | GPU count; `0` for non-GPU classes. |
| `egress_bytes` | Outbound bytes past the GCP network boundary. |
| `workspace_byte_seconds` | Workspace storage time-integral. |
| `artifact_byte_seconds` | Artifact storage time-integral. |
| `log_byte_seconds` | Log storage time-integral before rotation. |
| `metering_source` | `gcp_reported`, `node_measured`, or `estimated`. |
| `cost_input_microusd` | Nullable billing input: metered/catalog GCP cost × 1.10. |
| `cost_input_basis` | `cost_plus_10pct_gcp` (live metered Billing export), `cost_plus_10pct_gcp_catalog` (measured VM-seconds × published list-price catalog rate), or `unavailable`. |

`cost_input_microusd` is the billing input forwarded to Treasury. It is not the
invoiced or settled amount. Set it to `null` when GCP billing data is not
available at receipt closeout time; downstream reconciliation must not invent
a cost figure.

## Session Caps and Lease Limits

Quota routing enforces two independent caps evaluated per owner identity and
per org identity at admission time.

### Active-Session Cap

The active-session cap limits concurrently live managed sessions — those whose
microVM is in a `running` or `paused` state — for a single owner or org.

| Scope | Default cap |
| --- | --- |
| Per owner (individual user) | 4 active sessions |
| Per org (organisation account) | 20 active sessions |

Sessions in `terminating` state do not count against the cap. Sessions in
`paused` state do count. Caps apply across all compute classes; a `micro`
session and a `gpu-high` session each consume one slot.

### Max-Remote-Lease Cap

The max-remote-lease cap limits concurrently held GCP remote compute leases —
VM instances, TPU slices, or reserved accelerator capacity — regardless of
whether the associated session is currently active or paused.

| Scope | Default cap |
| --- | --- |
| Per owner | 2 remote leases |
| Per org | 10 remote leases |

A remote lease is held from the GCP `InstanceStart` event (or equivalent)
until the GCP resource is fully released and the deallocation is confirmed.
Sessions on the `local` or `shc` provider lane, including Firecracker-local
microVMs, do not consume a remote lease slot.

Caps are evaluated read-consistently at admission. A narrow race between two
concurrent session-start requests may transiently allow one slot past the cap;
the next admission corrects the in-memory count. Sessions already admitted past
the cap due to such races are not forcibly terminated; only new admission
requests are rejected until the count falls back within bounds.

## TTL and Idle-Timeout Defaults

Every managed session carries a hard TTL and an idle-timeout enforced by
`oa-workroomd`, applied regardless of provider lane.

| Parameter | Default | Description |
| --- | --- | --- |
| `session_ttl` | 8 h | Hard wall-clock lifetime from session start. |
| `idle_timeout` | 30 min | Inactivity window before the session is evicted. |
| `lease_ttl` | 12 h | Hard lifetime of the remote lease, even if the session is replaced. |
| `pause_ttl` | 2 h | Maximum time a paused session may remain paused before eviction. |

Activity signals that reset `idle_timeout`:

- Any runner event written to the session event stream.
- A workroom keep-alive ping from the client.
- An artifact or log flush initiated by the session.

Reaching `session_ttl` triggers a graceful shutdown followed by a final
resource-usage receipt. Reaching `idle_timeout` triggers the same sequence.
In both cases the compute dimensions are metered through the shutdown
completion timestamp, not the eviction decision timestamp. Reaching `pause_ttl`
triggers a clean teardown with the same receipt trail.

Operators may override defaults for individual owners or orgs via the
quota-routing config. Overrides above 24 h `session_ttl` or 4 h `idle_timeout`
require an explicit allowlist entry and are not available to self-serve owners.

## Rejection Receipt ('could not acquire compute')

When a session-start request is rejected because a cap is exhausted or no
compute is available, the quota router emits a **rejection receipt** in place
of starting a session. The rejection receipt is refs-only: it carries no raw
GCP project identifier, no raw owner or customer identity, no cost figures,
and no customer billing data in any public-facing field.

### Rejection Receipt Fields

| Field | Purpose |
| --- | --- |
| `rejection_ref` | Stable opaque ref for this rejection event, e.g. `cqr://reject/sha256:…`. |
| `request_ref` | Ref to the originating session-start request. |
| `owner_ref` | Redacted owner or org ref, e.g. `owner://sha256:…`. |
| `compute_class_requested` | The class label that was requested. |
| `rejection_code` | One of the codes listed below. |
| `quota_dimension` | Cap dimension exhausted, if applicable; `null` for capacity or policy rejections. |
| `receipt_digest` | `sha256:` digest over the rejection receipt material. |

### Rejection Codes

| Code | Meaning |
| --- | --- |
| `active_session_cap_exceeded` | Owner or org active-session cap is full. |
| `remote_lease_cap_exceeded` | Owner or org remote-lease cap is full. |
| `no_capacity` | No GCP capacity is available for the requested class at this time. |
| `class_not_permitted` | The requested compute class is not enabled for this owner or org. |
| `quota_suspended` | Owner or org quota access is administratively suspended. |

The rejection receipt is emitted as an `openagents.runner_event.v1`
`compute.quota.rejected` event and written to `compute-quota-rejections.jsonl`
on the node. It is not a billable event; no `cost_input_microusd` field
appears on a rejection receipt. Free-text diagnostic details belong in the
associated runner event, not in the receipt itself.

## Pricing Principle

Compute billing inputs follow a **cost-plus-10%** principle. The
`cost_input_microusd` in a `compute_usage` record is:

```text
cost_input_microusd = floor(gcp_metered_cost_microusd × 1.10)
```

where `gcp_metered_cost_microusd` is the sum of:

- VM-seconds × the GCP on-demand rate for the compute class at the metered
  region and time of the session;
- egress bytes × the applicable GCP network egress rate;
- storage byte-seconds × the applicable GCP storage rate converted to a
  per-byte-second unit.

GCP rates are read from the Cloud Billing Catalog API and cached with a 24 h
TTL per region. Rate staleness beyond 24 h must be reflected by setting
`metering_source` to `estimated`; a reconciliation pass corrects
`cost_input_microusd` once fresh rates are available.

The 10% markup is the billing input to Treasury. It is not the final invoiced
or settled amount. Treasury, Nexus, and the downstream billing pipeline apply
their own discount, subscription, credit, and rounding logic before settlement.
This contract specifies only what enters `cost_input_microusd`.

### Cost-Driven Placement (CND-042)

The same cost-plus-10% principle drives lane placement. For a non-caller-pinned
`Auto` placement where both cloud lanes are eligible, placement compares the
per-lane cost-plus-10% per-VM-second estimate and binds the cheaper lane,
subject to the owner-direction tiebreak below. The comparison basis is the
CND-042 receipt comparison report:
`docs/benchmarks/2026-06-14-cnd-042-gce-shc-receipt-comparison.md`.

Per-lane cost-plus-10% estimates (default compute class), from that report:

| Lane | Raw basis | cost-plus-10% (micro-USD / VM-sec) |
| --- | --- | --- |
| GCE `e2-small` (us-central1) | $0.016751 / VM-hr list price | ~5 |
| SHC `oa-shc-katy-01` (whole-host amortized) | $120/mo modeled invoice ÷ 730h | ~50 |

Owner-direction tiebreak: Google GCE is the preferred lane and wins ties and
near-ties. SHC is selected by cost only when it is BOTH materially cheaper than
GCE (by at least the configured margin, default 10%) AND the SHC pilot
recommendation is "expand". The report currently recommends HOLD, so cost-driven
placement resolves to GCE.

The chosen lane records `cost_driven = true` and a refs-only `cost_basis`
(`PlacementCostBasis`) on the `RunnerBinding` / `placement.bound` event. The
cost basis surfaces the lane estimates and the per-session figure only; it never
carries raw customer cost, raw GCP/SHC invoice identifiers, or settlement refs.

Cost-driven placement can be disabled with `OA_CODEX_PLACEMENT_COST_DRIVEN=false`
to restore the policy-driven Google-first default (`cost_driven = false`, no
`cost_basis`). The cost-model rates have a single update point — the
`*_RAW_PER_VM_SEC_NANOUSD` constants in `openagents-cloud-contract` — so refresh
them from a real GCP Billing Catalog pull and a real SHC invoice rather than
editing placement logic.

`cost_input_microusd` must not be derived from subscription plans, flat-rate
agreements, manually entered figures, or any source other than a GCP rate × 1.10.
The rate source is recorded in `cost_input_basis`:

- `cost_plus_10pct_gcp` — the rate came from a live GCP Cloud Billing export /
  Billing-Catalog-API pull (the eventual target). This is the only basis that
  may claim a live metered cost.
- `cost_plus_10pct_gcp_catalog` — the VM-seconds are genuinely **measured** (from
  the lease wall-time) but the rate is the GCP **published list-price catalog**
  rate (`GCE_RAW_PER_VM_SEC_NANOUSD`), pending a live Billing export (cloud#92,
  CND-042 report §2.4). The measured dimension and the catalog rate are kept
  honestly distinct: do not relabel a catalog-rate cost as a live metered cost.
- `unavailable` — no GCP rate was available; `cost_input_microusd` is `null`.

Whichever basis is used, the markup is the same cost-plus-10% computed in one
place (`LaneCostModel::cost_plus_10pct_micro_usd_per_vm_sec`); do not re-derive
the 1.10 inline. When no rate is available at all, set `cost_input_microusd` to
`null` and `cost_input_basis` to `unavailable`; do not estimate or substitute.

## Runner Behavior

`oa-workroomd` writes compute quota and metering records to:

```text
compute-quota-receipts.jsonl
compute-quota-rejections.jsonl
```

At session closeout `oa-workroomd` appends the `compute_usage` sub-record to
the `resource-usage-receipts.jsonl` entry for the same `run_ref`. The
`receipt_digest` in that entry must be recomputed after the `compute_usage`
block is added.

Quota routing emits the following `openagents.runner_event.v1` events:

- `compute.quota.admitted` at session start, citing the `owner_ref` and
  `compute_class`;
- `compute.usage.captured` at closeout, citing the `receipt_digest` and
  `run_ref`;
- `compute.quota.rejected` on admission failure, citing the `rejection_ref`
  and `rejection_code`.

Artifact and closeout receipts remain separate. This contract covers only
compute metering and quota routing facts.

## Validation Rules

- `compute_class` must be one of the defined class labels; unlisted labels are
  rejected.
- `vm_seconds` must be positive and must not exceed `session_ttl` converted to
  seconds plus a 60-second shutdown grace window.
- `gpu_type` must be `null` for non-GPU classes and must be a non-null
  recognised model string for GPU classes.
- `gpu_count` must be `0` for non-GPU classes and must match the provisioned
  GPU count for GPU classes.
- `egress_bytes`, `workspace_byte_seconds`, `artifact_byte_seconds`, and
  `log_byte_seconds` must be non-negative.
- `metering_source = estimated` requires a corresponding note in the associated
  `compute.usage.captured` runner event explaining why GCP-reported data was
  unavailable.
- `cost_input_microusd`, when not `null`, must equal
  `floor(vm_seconds × cost-plus-10% rate)` for the basis's rate source; any other
  derivation is rejected.
- `cost_input_basis` must be `cost_plus_10pct_gcp` or
  `cost_plus_10pct_gcp_catalog` when `cost_input_microusd` is populated, and
  `unavailable` (with a `null` cost) otherwise.
- Rejection receipts must not contain raw GCP project identifiers, raw owner
  or customer identities, cost figures, or billing data in any field.
- `rejection_code` must be one of the defined codes; free-text rejection
  reasons belong in the associated runner event, not in the receipt.
- `receipt_digest` and `rejection_ref` must be `sha256:` references.
- Settlement refs, wallet seeds, private keys, bearer tokens, and private
  topology markers must not appear in any compute quota or metering record.
- Silent missing `compute_usage` blocks are not acceptable when a microVM was
  started; an explicit `metering_source = estimated` record with a declared
  reason is required.
