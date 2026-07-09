# Compute Isolation Benchmark Plan

Status: planning — not yet executed
Date: 2026-06-13
Owner: Cloud infrastructure

## Context and Owner Decision

OpenAgents Cloud is launching on **full ephemeral per-session GCE VMs** now,
under the `openagents.gce_capacity_class.v1` contract. The decision is to move
to microVMs (Firecracker, and separately gVisor / TDX confidential compute via
`sek8s`) once benchmarks justify the cutover in density and cold-start latency.

Two binding outputs must come from this benchmark cycle:

**(a) Compute markup over GCP cost.** The `openagents.compute_quota_routing.v1`
contract fixes the formula at `cost_input_microusd = floor(gcp_metered_cost_microusd × 1.10)`,
establishing a 10% margin as the billing input to Treasury. That 10% figure is
the **current default**. This benchmark must confirm whether actual overhead
(provision latency burn, idle bleed, failed-acquire waste, GCP rate-cache
staleness, firewall/SSH setup cost) is covered at 10%, or whether the margin
requires adjustment. The result is a measured recommendation for the markup
percentage that is then locked into the `cost_input_microusd` formula for each
compute class.

**(b) MicroVM cutover criteria.** Quantified thresholds on density, cold-start
latency, isolation fidelity, and per-session cost that must be met before
full-VM sessions are replaced with Firecracker or gVisor microVMs in
production. These become the go/no-go gate for the `sandbox.firecracker.exec`
and `sandbox.gvisor.exec` profiles in `oa-workroomd`.

Both outputs feed directly into the contracts at
`docs/contracts/openagents.gce_capacity_class.v1.md` and
`docs/contracts/openagents.compute_quota_routing.v1.md`.

---

## 1. Benchmark Tracks

### 1.1 Track A — Full-VM Baseline (GCE Ephemeral)

Contract anchor: `openagents.gce_capacity_class.v1`

This is the current production configuration. Each workroom or benchmark session
acquires a dedicated Compute Engine VM via `capacity_class_id = gce.ephemeral.standard.v1`,
progresses through `acquire → ready → in_use → release`, and is destroyed at
session end or on `ttl_expired` / `idle_timeout` eviction.

#### 1.1.1 Provision and Boot Latency

Measure elapsed wall-clock time from the moment the quota router emits
`compute.quota.admitted` to the moment the lease enters the `ready` state
(VM exists, labels applied, managed firewall rule applied, SSH metadata
installed, and bootstrap health check passed). The `gce_capacity_class.v1`
contract requires all five of those conditions before `ready` is declared.

Sub-intervals to capture separately:

| Sub-interval | Start event | End event |
| --- | --- | --- |
| GCP API enqueue latency | `compute.quota.admitted` | GCP `instances.insert` request submitted |
| VM creation latency | GCP `instances.insert` submitted | GCP `RUNNING` transition confirmed |
| SSH metadata apply | GCP `RUNNING` | SSH metadata written and `ssh_metadata_ref` recorded |
| Firewall apply | SSH metadata written | Managed firewall rule applied, `firewall_rule_ref` recorded |
| Bootstrap health check | Firewall applied | Health check passed, `ready` state entered |
| Total provision-to-ready | `compute.quota.admitted` | `ready` state entered |

Measure at each defined compute class:

| Class | vCPU | RAM (GiB) | Target provision-to-ready |
| --- | --- | --- | --- |
| `micro` | 1–2 | 1–4 | TBD from runs |
| `standard` | 2–8 | 4–32 | TBD from runs |
| `compute` | 8–32 | 32–128 | TBD from runs |
| `gpu-standard` | 4–16 | 16–64 | TBD from runs |
| `gpu-high` | 8–32 | 64–256 | TBD from runs |

Methodology: 30 cold provisions per class across two GCP zones (one primary,
one backup). Record `provision_receipt_ref` and timing from the lease
projection. Compute p50, p90, p99, and max. Failures (failed-acquire, VM
creation timeout) are counted separately and feed the markup analysis.

#### 1.1.2 Per-Session Cost ($/session)

The `compute_quota_routing.v1` contract defines:

```text
cost_input_microusd = floor(gcp_metered_cost_microusd × 1.10)
```

where `gcp_metered_cost_microusd` sums:

- `vm_seconds × gcp_on_demand_rate` for the compute class and zone
- `egress_bytes × gcp_network_egress_rate`
- `(workspace_byte_seconds + artifact_byte_seconds + log_byte_seconds) × gcp_storage_rate`

GCP rates are read from the Cloud Billing Catalog API with a 24 h cache TTL.
Any rate staleness beyond 24 h sets `metering_source = estimated`.

**Benchmark approach:** Run synthetic sessions of three durations — 5 min, 30
min, and 60 min — per compute class. For each run record the full
`compute_usage` sub-record (`vm_seconds`, `egress_bytes`,
`workspace_byte_seconds`, `artifact_byte_seconds`, `log_byte_seconds`) as
written by `oa-workroomd` to `compute-quota-receipts.jsonl`. Derive observed
GCP cost from live Billing Catalog rates and apply the 1.10 multiplier. Report:

- raw `gcp_metered_cost_microusd`
- `cost_input_microusd` at 1.10×
- cost per session-minute for each class

Then compare against actual GCP billing export data from the benchmark run
to validate that metered cost closely tracks billed cost. The delta (metered
vs. billed) is the calibration gap that must be covered by the markup.

#### 1.1.3 Idle Cost

A `ready` lease with no attached session is subject to `idle_timeout_ms`
(default 30 min). An `in_use` lease refreshes idle state only through
explicit heartbeat receipts. Idle compute still accrues `vm_seconds` and
associated GCP cost.

Measure:

- Rate of `vm_seconds` accumulation per idle class during `ready` state
- Idle burn rate in $/hour for each compute class at list GCP on-demand pricing
- Average idle duration before `idle_timeout` eviction across 20 sessions per
  class (simulated by withholding heartbeats after `in_use`)
- Failed-acquire waste: cost of VM creation attempts that fail before `ready`
  is reached, expressed as `vm_seconds × rate` for the failed interval

Idle burn and failed-acquire waste are overhead not attributable to billed
session time. They are part of the infrastructure cost that the markup must
cover.

#### 1.1.4 Teardown Time

Measure elapsed time from lease entering `release` to the cleanup receipt
recording `result = completed`. The `gce_capacity_class.v1` contract requires
that release deletes the managed VM, removes the managed firewall rule,
revokes session SSH metadata, and mints a cleanup receipt ref.

Sub-intervals:

| Sub-interval | Trigger | End |
| --- | --- | --- |
| GCP VM deletion API call | `release` initiated | `instances.delete` submitted |
| VM deletion confirmed | `instances.delete` submitted | GCP confirms deletion |
| Firewall rule removal | Deletion confirmed | `firewall_rule_ref` cleaned, `removed_firewall_rule = true` |
| SSH metadata revocation | Firewall removed | `revoked_ssh_metadata = true` |
| Cleanup receipt minted | All prior steps | `cleanup_receipt_ref` written |
| Total release-to-clean | `release` initiated | `result = completed` |

30 runs per class, covering three `release_reason` codes:
`manual`, `ttl_expired`, and `idle_timeout`. Idempotency must be verified: a
second release call on the same `lease_ref` must return `already_clean` without
re-triggering cloud operations.

#### 1.1.5 Concurrency Ceiling

The `compute_quota_routing.v1` contract enforces:

- Per-owner active-session cap: **4**
- Per-org active-session cap: **20**
- Per-owner remote-lease cap: **2**
- Per-org remote-lease cap: **10**

Caps apply across all compute classes regardless of resource weight.

**Benchmark approach:** Drive concurrent session-start requests to the quota
router at 2×, 4×, 8×, and 16× the per-owner cap. Record:

- `compute.quota.admitted` events (successful admissions)
- `compute.quota.rejected` events with `rejection_code`
  (`active_session_cap_exceeded` or `remote_lease_cap_exceeded`)
- `rejection_ref` and `quota_dimension` from each rejection receipt

Measure the concurrency at which GCP zone capacity (`no_capacity` rejections)
becomes the binding constraint rather than the software cap. That zone-level
ceiling determines the effective fleet concurrency ceiling at current
provisioned quota.

Also measure soft-race behavior: send two simultaneous session-start requests
for the same owner at exactly the cap limit; confirm that at most one extra
slot is transiently admitted and that the next admission is rejected until the
count corrects.

---

### 1.2 Track B — MicroVM Comparison

Contracts are grounded in the `compute_quota_routing.v1` `vm_seconds` metering
definition: for microVMs, `vm_seconds` begins at the Firecracker `InstanceStart`
event and ends at the first of clean shutdown, idle-timeout eviction
completion, or TTL expiry.

The `VirtualizationFacts` struct in `crates/openagents-cloud-contract/src/lib.rs`
already carries `firecracker_candidate: bool`. The SHC host `oa-shc-katy-01`
has Firecracker v1.15.1 and Jailer v1.15.1 installed, with KVM acceleration
confirmed and a manual boot smoke that returned `OA_FIRECRACKER_GUEST_OK`
(see `docs/bootstrap/CND-041-shc-katy-01-bootstrap.md`).

Two microVM sub-tracks are in scope:

| Sub-track | Technology | Repo | Isolation model |
| --- | --- | --- | --- |
| B1 | Firecracker + jailer | `projects/repos/firecracker` | KVM-based lightweight VMM; strong VM-level isolation; no hypervisor overhead from full QEMU |
| B2 | gVisor / TDX confidential | `projects/repos/sek8s` | gVisor (kernel syscall interception) and/or Intel TDX hardware-attested confidential VMs |

Sub-track B1 runs on `oa-shc-katy-01` (SHC provider lane). Sub-track B2 runs
on GCP Confidential VM instances where TDX is available, using the `sek8s`
bootstrap. Sessions on the `local` or `shc` provider lane, including
Firecracker-local microVMs, do not consume a remote lease slot under the
`compute_quota_routing.v1` max-remote-lease cap.

#### 1.2.1 Density (microVMs per host)

Measure the number of concurrently running microVM sessions a single host
supports before CPU steal, memory pressure, or KVM scheduling latency degrades
the p99 in-guest response time beyond an agreed threshold (proposed: +50%
degradation relative to single-microVM baseline).

Methodology:

1. Start with 1 microVM, measure in-guest latency baseline (ping latency to TAP
   interface, disk I/O throughput, memory bandwidth via `sysbench`).
2. Increase concurrently running microVMs in steps of 4 until degradation
   threshold is crossed.
3. Record the maximum N at which all microVMs remain within threshold.
4. Compare against full-VM concurrency ceiling from Track A (Track A measures
   GCP quota/cap ceiling; Track B measures host-level density per physical node).

Report `microVMs per host vCPU` and `microVMs per GiB host RAM` as density
ratios. These ratios feed the $/session comparison in §1.2.3.

#### 1.2.2 Cold-Start Latency

The `compute_quota_routing.v1` `vm_seconds` field begins at the Firecracker
`InstanceStart` event. Cold-start latency is the interval from when
`oa-workroomd` issues the Firecracker launch command to the moment the guest
kernel has booted and the first session-ready signal is received (analogous to
the `ready` state in the GCE lease lifecycle).

Sub-intervals:

| Sub-interval | Start | End |
| --- | --- | --- |
| Firecracker process spawn | `oa-workroomd` issues launch | Firecracker process running |
| Guest kernel boot | Firecracker `InstanceStart` emitted | Guest kernel `init` reached |
| Guest network ready | Guest `init` reached | TAP interface reachable |
| Session-ready signal | TAP reachable | In-guest `oa-workroomd` or health-check probe responds |
| Total cold-start | `oa-workroomd` issues launch | Session-ready signal received |

Measure 50 cold starts per image configuration (minimal rootfs, standard
workroom rootfs). Compare p50 and p99 against full-VM provision-to-ready from
§1.1.1. The microVM cold-start advantage — if present — is the primary
justification for cutover.

For Track B2 (TDX confidential), also record the attestation handshake
overhead added to cold-start: the time from TDX REPORT generation to remote
attestation verification complete.

#### 1.2.3 Isolation Guarantees

Document the isolation model for each sub-track and map it to the OpenAgents
threat model (per-session isolation, no cross-session data access, no
container escape to host).

| Property | Full VM (GCE) | Firecracker (B1) | gVisor/TDX (B2) |
| --- | --- | --- | --- |
| Kernel isolation | Separate kernel per VM | Separate kernel per microVM | Separate kernel (gVisor) or hardware-attested (TDX) |
| Memory isolation | Hardware page tables | Hardware page tables + minimal VMM | Hardware page tables + gVisor interceptor or TDX SEAM mode |
| Hypervisor attack surface | Full QEMU/KVM stack | Firecracker minimal VMM (~50 kLOC) | gVisor runsc or TDX firmware |
| Network isolation | Managed VPC + firewall rule | TAP + host netns | TAP + host netns or TDX vNIC |
| Storage isolation | Separate persistent disk | Separate rootfs image per microVM | Separate rootfs image per microVM |
| Attestation | None by default | None by default (can add vTPM) | Hardware attestation via TDX REPORT |
| Confidential memory | No | No | Yes (TDX encrypted memory) |
| Receipt support | `gce_capacity_class.v1` | `VirtualizationFacts.firecracker_candidate` | `VirtualizationFacts.firecracker_candidate` + attestation receipt |

For each sub-track, the benchmark must confirm that the `sandbox.firecracker.exec`
or equivalent profile satisfies the Psionic sandbox evidence requirements
referenced in `CND-041`: jailer/cgroup policy, guest kernel/rootfs digests,
TAP/firewall receipts, artifact closeout, and idempotent cleanup.

#### 1.2.4 Per-Session Cost ($/session) — MicroVM

The `compute_quota_routing.v1` pricing formula applies identically to microVMs.
`vm_seconds` begins at Firecracker `InstanceStart` and is metered at the host
GCP instance rate, apportioned across concurrent microVMs by the node agent.

For the SHC provider lane (Track B1 on `oa-shc-katy-01`), `egress_bytes`
is 0 unless routed through a GCP NAT; `workspace_byte_seconds` and
`artifact_byte_seconds` are measured locally. The effective $/microVM-session
is:

```text
host_vm_seconds × gcp_rate / microVMs_per_host + storage_cost
```

Because microVMs share a host, the per-session cost decreases as density
increases. Measure:

- $/session at density N = 1, 4, 8, 16, 32 microVMs per host
- Break-even density: the N at which microVM $/session equals full-VM $/session
  for the same compute class
- Cost savings at max stable density from §1.2.1

Also measure for Track B2 (TDX): whether confidential VM instance types carry a
GCP pricing premium and at what density that premium is offset by reduced
per-session infrastructure overhead.

---

## 2. Metrics and Methodology

### 2.1 Metric Taxonomy

All metrics are collected via the receipt trail defined by the contracts.
No ad hoc cost figures or manually entered rates are accepted.

| Metric | Source | Contract field |
| --- | --- | --- |
| Provision-to-ready latency | Lease state timestamps | `gce_capacity_class.v1` `acquire` → `ready` |
| Release-to-clean latency | Cleanup receipt timestamps | `cleanup_started_at`, `cleanup_completed_at` |
| `vm_seconds` | `oa-workroomd` `compute_usage` record | `compute_quota_routing.v1` `vm_seconds` |
| `egress_bytes` | Node agent GCP apportionment | `compute_quota_routing.v1` `egress_bytes` |
| `workspace_byte_seconds` | `oa-workroomd` closeout | `compute_quota_routing.v1` `workspace_byte_seconds` |
| `cost_input_microusd` | GCP Billing Catalog × 1.10 | `compute_quota_routing.v1` `cost_input_microusd` |
| `metering_source` | `oa-workroomd` | `gcp_reported`, `node_measured`, or `estimated` |
| `rejection_code` | Quota router rejection receipt | `compute_quota_routing.v1` rejection receipt |
| Cold-start latency | Firecracker `InstanceStart` event | `vm_seconds` start anchor |
| In-guest latency | Active measurement from host | Not in receipt; benchmark-only metric |
| Density | Concurrent microVMs on host | Derived from `vm_seconds` and host inventory |
| `firecracker_candidate` | `VirtualizationFacts` | `openagents.resource_usage_receipt.v1` `host` block |
| Attestation latency | TDX REPORT timestamp | Track B2 only; outside current contract scope |

`metering_source` is a quality flag: any session recording `estimated` must
include a declared reason in the associated `compute.usage.captured` runner
event. The share of sessions with `metering_source = estimated` is a secondary
quality metric that must stay below 5% in steady state.

### 2.2 GCP Rate Sourcing

GCP on-demand rates are read from the Cloud Billing Catalog API and cached with
a 24 h TTL per region, as specified by `compute_quota_routing.v1`. Benchmark
sessions must record the rate-fetch timestamp alongside `cost_input_microusd`.
Any session where the cached rate is older than 24 h must set
`metering_source = estimated`; the benchmark analysis excludes such sessions
from the primary cost analysis and flags them separately.

Rate versions used during the benchmark run are frozen in a rate-snapshot
artifact. Post-run cost reconciliation uses the same frozen rates to ensure
reproducibility.

### 2.3 Session Lifecycle Coverage

The benchmark covers all lease lifecycle transitions defined by
`gce_capacity_class.v1`:

| Transition | Covered by | Runs |
| --- | --- | --- |
| `acquire → ready` | §1.1.1, §1.2.2 | 30 per class |
| `ready → in_use` | §1.1.2, §1.1.3 | 30 per class |
| `in_use → release (manual)` | §1.1.4 | 30 per class |
| `in_use → release (ttl_expired)` | §1.1.4 | 10 per class |
| `in_use → release (idle_timeout)` | §1.1.3, §1.1.4 | 20 per class |
| `acquire → failed_acquire` | §1.1.5 | 10 per class |
| Idempotent repeat release | §1.1.4 | 5 per class |

For microVM sub-tracks, the equivalent lifecycle uses Firecracker `InstanceStart`
as the `acquire → ready` anchor (§1.2.2) and `oa-workroomd` session closeout
as the `release` anchor.

TTL defaults under test:

| Parameter | Default | Benchmark override for short-duration runs |
| --- | --- | --- |
| `session_ttl` | 8 h | 5 min, 30 min, 60 min |
| `idle_timeout` | 30 min | 1 min (idle-timeout tests only) |
| `lease_ttl` | 12 h | Unmodified |
| `pause_ttl` | 2 h | Unmodified |

### 2.4 Data Collection and Retention

Each benchmark run produces:

- `resource_usage_receipt.json` (per `openagents.resource_usage_receipt.v1`)
- `compute-quota-receipts.jsonl` (per `compute_quota_routing.v1`)
- `compute-quota-rejections.jsonl` (rejection receipts with `rejection_code`)
- `cloud_execution_closeout.json` (Cloud-side closeout gate artifact)
- Raw lease projection snapshot at each state transition
- GCP Billing Catalog rate snapshot (frozen at run start)
- In-guest latency time series (Track B only; not a contract artifact)

All retained projections must comply with the forbidden-field rules of both
contracts: no raw GCP project ids, IP addresses, instance names, self-links,
credentials, or private topology markers in any retained artifact.

Receipt digests are recorded as `sha256:` references. The `receipt_digest` in
the `compute_usage` record must be recomputed after the `compute_usage` block
is appended to the `resource-usage-receipts.jsonl` entry, per
`compute_quota_routing.v1` runner behavior rules.

Artifact storage: benchmark artifacts land in the designated Cloud Storage
benchmark bucket under the `openagents-benchmark` project label. Raw
per-session timing data is retained for 90 days. Aggregated summary artifacts
are retained indefinitely.

---

## 3. Decision Outputs

### 3.1 Decision A — Compute Markup Over GCP Cost

The `compute_quota_routing.v1` contract sets the billing formula:

```text
cost_input_microusd = floor(gcp_metered_cost_microusd × 1.10)
```

The 10% markup is the **billing input to Treasury**. It is not the final
invoiced or settled amount. This benchmark determines whether 10% covers
the full cost of the compute infrastructure relative to raw GCP cost.

**Overhead items the markup must cover:**

| Overhead source | Mechanism | Measured in |
| --- | --- | --- |
| Provision latency burn | `vm_seconds` accrues during `acquire → ready` before session use | §1.1.1 |
| Idle bleed (ready state) | `vm_seconds` accrues while lease is `ready` with no workroom attached | §1.1.3 |
| Idle bleed (in-use, no heartbeat) | `vm_seconds` accrues during idle periods within a session | §1.1.3 |
| Failed-acquire waste | VM creation cost for leases that never reach `ready` | §1.1.5 |
| Teardown cost | `vm_seconds` continues during `release` until GCP confirms deletion | §1.1.4 |
| GCP rate-cache staleness corrections | Estimated vs. reported deltas requiring downstream reconciliation | §2.2 |
| Billing Catalog API call cost | Negligible; excluded from markup analysis unless measurable |  |

**Analysis method:**

1. For each compute class, compute total observed GCP cost across all benchmark
   sessions (billed by GCP, from Billing export).
2. Compute total billed session time (time from `in_use` to `release` where the
   session produced actual work).
3. Compute total overhead time (provision burn + idle bleed + teardown + failed
   acquires).
4. Overhead fraction = `overhead_vm_seconds / total_vm_seconds`.
5. The minimum markup that makes the 10% formula sustainable:
   `minimum_markup = 1 / (1 - overhead_fraction) - 1`.
   If `overhead_fraction = 0.07` then `minimum_markup ≈ 7.5%`.
6. Add a target buffer (proposed: 2–3 percentage points) to arrive at the
   recommended markup.

**Output:** A recommended markup percentage per compute class, grounded in
measured overhead fractions. The default 10% must be confirmed or revised.
The revised figure becomes the canonical `cost_input_basis = cost_plus_N_pct_gcp`
value locked into Treasury routing for each class.

If measured overhead fractions differ significantly across classes (e.g., `micro`
has proportionally higher provision latency burn than `standard`), the contract
may need per-class markup rates rather than a single 10% figure. This plan
outputs a recommendation; any contract change requires a separate contract
revision.

### 3.2 Decision B — MicroVM Cutover Criteria

Cutover from full ephemeral GCE VMs to Firecracker (and eventually gVisor/TDX)
requires satisfying all of the following criteria simultaneously. These are
proposed go/no-go thresholds; the owner confirms final threshold values after
reviewing benchmark data.

#### B.1 Cold-Start Latency

| Criterion | Proposed threshold | Source |
| --- | --- | --- |
| Firecracker cold-start p50 (total, §1.2.2) | ≤ 3 s | vs. GCE p50 from §1.1.1 |
| Firecracker cold-start p99 (total, §1.2.2) | ≤ 8 s | vs. GCE p99 from §1.1.1 |
| Cold-start p50 must be less than full-VM provision-to-ready p50 | Yes (required) | Comparison of §1.1.1 and §1.2.2 |

Rationale: Firecracker's primary advantage over full VMs is cold-start speed.
If Firecracker p50 cold-start exceeds the GCE provision-to-ready p50, the
density benefit alone must justify cutover, and the bar for density rises
accordingly.

#### B.2 Density

| Criterion | Proposed threshold |
| --- | --- |
| Stable microVMs per host (§1.2.1) | ≥ 8 concurrent at `standard` class resource allocation |
| Per-session cost at max stable density (§1.2.3) | ≤ 70% of equivalent full-VM $/session |
| In-guest p99 latency at max stable density | ≤ 150% of single-microVM baseline |

Rationale: 8× density at `standard` class translates to a meaningful per-session
cost reduction. The 30% cost reduction threshold (≤ 70% of full-VM cost) must
be demonstrated at sustained density, not just at N = 1.

#### B.3 Isolation Fidelity

All of the following must pass before any microVM profile is promoted to
production:

- `oa-workroomd` integration with Firecracker is complete (not just the manual
  smoke from `CND-041`): jailer/cgroup policy enforced, guest kernel and rootfs
  digests recorded, TAP/firewall receipts written, artifact closeout verified,
  idempotent cleanup confirmed.
- `sandbox.firecracker.exec` profile satisfies Psionic sandbox evidence
  requirements.
- Per-session cross-contamination test passes: no file system, memory, or
  network artifact from session N is accessible in session N+1 on the same host.
- `VirtualizationFacts.firecracker_candidate = true` is correctly recorded in
  every `openagents.resource_usage_receipt.v1` `host` block for microVM sessions.
- Receipt trail for microVM sessions satisfies `compute_quota_routing.v1`
  validation rules: `vm_seconds` starts at `InstanceStart`, silent missing
  `compute_usage` blocks are absent, `metering_source = estimated` with
  declared reason is used when needed.

#### B.4 Metering Accuracy

| Criterion | Proposed threshold |
| --- | --- |
| `metering_source = gcp_reported` or `node_measured` (not `estimated`) | ≥ 95% of sessions |
| Measured vs. billed GCP cost delta | ≤ 5% across a 100-session validation run |
| Failed-acquire rate | ≤ 2% across any 50-session window |

#### B.5 Track B2 — Additional gVisor / TDX Criteria

Track B2 cutover (if adopted) adds:

| Criterion | Proposed threshold |
| --- | --- |
| TDX attestation overhead (cold-start addition, §1.2.2) | ≤ 2 s added to Firecracker cold-start p50 |
| TDX confidential VM GCP pricing premium | Offset by density benefit at max stable density |
| Remote attestation verification passes | Required for every session; failures block `in_use` |
| `sek8s` TDX receipt schema aligned with `resource_usage_receipt.v1` | Required before production use |

If Track B2 gVisor/TDX does not meet these criteria within the benchmark
timeline, Track B1 (Firecracker-only) may be promoted independently provided
all B.1–B.4 criteria are met.

#### B.6 Cutover Gate Summary

```text
CUTOVER GO criteria (all must be true):
  [ ] Firecracker cold-start p50 ≤ threshold (B.1)
  [ ] Firecracker cold-start p99 ≤ threshold (B.1)
  [ ] Cold-start p50 < full-VM provision-to-ready p50 (B.1)
  [ ] Stable density ≥ 8 microVMs per host at standard class (B.2)
  [ ] Per-session cost at max density ≤ 70% of full-VM cost (B.2)
  [ ] In-guest p99 latency at max density ≤ 150% of baseline (B.2)
  [ ] oa-workroomd Firecracker integration complete (B.3)
  [ ] Isolation cross-contamination test passes (B.3)
  [ ] VirtualizationFacts.firecracker_candidate recorded correctly (B.3)
  [ ] Receipt trail satisfies compute_quota_routing.v1 validation (B.3)
  [ ] metering_source = estimated rate ≤ 5% (B.4)
  [ ] Measured vs. billed cost delta ≤ 5% (B.4)
  [ ] Failed-acquire rate ≤ 2% (B.4)
```

Owner confirms final threshold values after reviewing benchmark data. Any
criterion marked NO-GO delays cutover; partial promotion (e.g., `micro` class
only) is possible if density and cost criteria are met for that class while
isolation work continues.

---

## 4. Referenced Repos and Contracts

| Reference | Role in this benchmark |
| --- | --- |
| `docs/contracts/openagents.gce_capacity_class.v1.md` | Full-VM lease lifecycle, receipt schema, and forbidden-field rules |
| `docs/contracts/openagents.compute_quota_routing.v1.md` | Compute classes, metering dimensions, quota caps, pricing formula, TTL defaults |
| `docs/contracts/openagents.resource_usage_receipt.v1.md` | `host` block facts including `VirtualizationFacts.firecracker_candidate` |
| `docs/bootstrap/CND-041-shc-katy-01-bootstrap.md` | Firecracker v1.15.1 manual smoke on `oa-shc-katy-01`; baseline for Track B1 |
| `projects/repos/firecracker` | Firecracker integration and `sandbox.firecracker.exec` profile (Track B1) |
| `projects/repos/sek8s` | gVisor and TDX confidential compute bootstrap; remote attestation receipt schema (Track B2) |
| `crates/openagents-cloud-contract/src/lib.rs` | `VirtualizationFacts` struct; `firecracker_candidate` field |
| `crates/oa-workroomd/src/main.rs` | `firecracker_candidate` detection logic; `compute-quota-receipts.jsonl` writer |

---

## 5. Timeline and Ownership

| Phase | Work | Owner | Prerequisite |
| --- | --- | --- | --- |
| P0 — Full-VM baseline | Track A (§1.1); all sub-metrics | Cloud infrastructure | GCP project access; `gce_capacity_class.v1` provisioner in test state |
| P1 — Firecracker integration | `oa-workroomd` Firecracker integration (`projects/repos/firecracker`); jailer/cgroup policy; TAP/firewall receipts | Cloud + Psionic | Track A complete; `CND-041` smoke passing |
| P2 — Firecracker benchmark | Track B1 (§1.2) on `oa-shc-katy-01` | Cloud infrastructure | P1 complete |
| P3 — gVisor / TDX | Track B2 (§1.2) using `projects/repos/sek8s` | Cloud + Psionic | P2 complete; TDX-capable GCP nodes provisioned |
| P4 — Decision review | Markup recommendation (Decision A); cutover criteria evaluation (Decision B) | Owner | All tracks complete |
| P5 — Contract update | Update `compute_quota_routing.v1` markup figure; set cutover gate state | Cloud | P4 sign-off |

No timeline dates are specified in this plan. Phases are sequentially
dependent. P0 begins on the next available benchmark window after this document
is reviewed.

---

## 6. Out of Scope

- Treasury, Nexus, and downstream billing pipeline behavior (settlement, invoicing,
  discounts, credits, and rounding logic are handled downstream of
  `cost_input_microusd`).
- GPU class benchmarks (Track A §1.1.1 measures `gpu-standard` and `gpu-high`
  provision latency but does not benchmark GPU workload throughput).
- Public leaderboard claims or public benchmark result publication (all outputs
  are internal; Omega/Vortex promotion gates apply for any public claim).
- Pylon contributor wallet UX and public Pylon contributor paths.
- Model token usage benchmarks (covered by `openagents.resource_usage_receipt.v1`
  model_usage fields but not in scope here).
- Subscription-backed Codex token-count availability (tracked separately as
  `count_source = unavailable` with `subscription_backed_codex_no_token_counts`).
