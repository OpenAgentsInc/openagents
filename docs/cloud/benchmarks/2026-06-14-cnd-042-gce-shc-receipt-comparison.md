# CND-042 GCE vs SHC Receipt & Cost Comparison

Date: 2026-06-14
Repo: `cloud`
Tracking: `OpenAgentsInc/cloud#89` (CND-042), epic `OpenAgentsInc/openagents#4996`
Owner direction recorded: 2026-06-14 (Google GCE is the preferred lane; SHC is a
secondary pilot kept only if it is materially cheaper and proven).

This report satisfies the CND-042 acceptance in `docs/ISSUES.md`:

- it cites setup, execution, artifact, benchmark, and closeout receipts from
  both the GCE node (`oa-gcp-shc-katy-01`) and the SHC node (`oa-shc-katy-01`);
- it separates measured GCP cost, the real SHC host invoice ($1000/yr capex),
  the SHC per-session amortization, and the unsettled assumptions behind each;
- it shows the per-session cost math for each lane;
- it recommends **HOLD** for the SHC pilot; and
- it lists follow-up issues for the missing integration surfaces.

No new live GCE run was performed for this report. Every receipt class required
by the acceptance already exists in captured form, so per the infra note we
preferred existing receipts over new spend. There is therefore **no GCE VM to
tear down for this issue** (the most recent live GCE smoke, CND-054, already
confirmed zero leftover instances at its own closeout).

---

## 1. Receipt citations (both nodes)

All cited digests are reproduced verbatim from the source docs. Note that the
bootstrap receipt docs use bare 64-hex SHA-256 strings for tarball/task
checksums; the contract-level receipt ids, run refs, and cleanup digests are
`sha256:` refs that, by the redaction rule, carry **no cost figures and no raw
GCP identifiers** (CND-054).

### 1.1 GCE node — `oa-gcp-shc-katy-01` (the `cloud-gcp` lane)

| Receipt class | Source | Key facts |
| --- | --- | --- |
| Setup | `docs/bootstrap/CND-045-gcp-benchmark-cloud-substrate.md` | GCP benchmark substrate, project `openagents-bench-dev`, region `us-central1`; Artifact Registry `oa-benchmark-runners`, GCS `${PROJECT_ID}-oa-benchmark-{specs,datasets,artifacts,proofs}-${ENV}`, Pub/Sub `benchmark-{task,run}-events-${ENV}`, scoped SAs `bench-controller-${ENV}`, `bench-runner-${ENV}`. |
| Setup (live lane) | `docs/bootstrap/CND-054-gce-live-per-session-provisioner-smoke.md` | Live per-session provisioner proof, `cloud#91`. Machine type `e2-small`, zone `us-central1-a`, image `ubuntu-2404-lts-amd64`. VM `oa-codex-sess-0d03539455829690`, firewall `oa-codex-sess-fw-0d03539455829690` (IAP `35.235.240.0/20`, tcp:22). `RUN_ID=run_gce_live_smoke_20260614185249`, passed 2026-06-14. |
| Execution | `docs/bootstrap/CND-054-gce-live-per-session-provisioner-smoke.md` | `LiveGceProvisioner` drove acquire → ready → in_use → release on a real `e2-small`. Test `gce_lane_provisions_runs_emits_receipt_and_cleans_up`. |
| Benchmark backend | `docs/bootstrap/CND-046-cloud-batch-benchmark-backend.md` | One normalized benchmark task runs through Cloud Batch on `us-central1`; runner image `py-bench-runner:dev`; emits `result.json`, `events.jsonl`, `metadata.json`, `artifact_manifest.json`, `proof_bundle.json`. |
| Artifact | `docs/bootstrap/CND-054-gce-live-per-session-provisioner-smoke.md` | `finish_gce_lease` emits an `openagents.resource_usage_receipt.v1` (refs-and-limits only). Receipt id / run ref / cleanup digest are `sha256:` refs with no cost figures and no raw GCP ids. |
| Closeout | `docs/bootstrap/CND-054-gce-live-per-session-provisioner-smoke.md`; `docs/BENCHMARK_CLOUD.md` | Idempotent lease release verified by label/name-filtered `instances list` → zero session VMs remaining. Closeout authority `cloud_execution_closeout.json`: `walletAuthority=false`, `payoutAuthority=false`, `publicClaimAuthority=false`, `authorityOwner=omega`. |

> Material gap on the GCE side: CND-054 deliberately records **no VM-seconds and
> no cost figure**. The GCE lane therefore has a *measured provisioning lifecycle*
> but no *measured infra cost receipt* yet. See §3 unsettled assumptions and the
> CND-054-bis follow-up.

### 1.2 SHC node — `oa-shc-katy-01` (the `cloud-shc` lane)

| Receipt class | Source | Key facts |
| --- | --- | --- |
| Setup | `docs/bootstrap/CND-041-shc-katy-01-bootstrap.md` | Host `oa-shc-katy-01` @ `23.182.128.195`; 16 logical CPUs (Intel Xeon Skylake), 62 GiB usable RAM, 247 GiB root disk (~245 GiB free), Ubuntu 24.04.4 LTS, kernel 6.8.0-124, `/dev/kvm` present. Toolchain `rustc 1.96.0`, `codex-cli 0.135.0`, Firecracker/Jailer v1.15.1, guest kernel `vmlinux-6.1.155`. Bootstrap smoke passed 2026-06-01. |
| Execution (1-task) | `docs/bootstrap/CND-050-shc-codex-terminal-bench-smoke.md` | Shape `16 vCPU, 64 GB RAM, 256 GB NVMe class VPS`. Task `terminal-bench/openssl-selfsigned-cert` (checksum `2b70d5535b5873f644fad37b76dbef86a1e42162e018c7bc06316e5e2521929a`). Total runtime `1m12s`; reward 1.000 (6/6). Reported **model** cost `$0.120667`. |
| Benchmark (8-task) | `docs/bootstrap/CND-051-shc-codex-terminal-bench-8task.md` | `terminal-bench@2.0` (89 tasks total), Codex 0.135.0 / `gpt-5.5`, 8 tasks, mean reward 0.75 (6/8). Reported **model** cost `$3.697649`. |
| Benchmark + Artifact (16-task) | `docs/bootstrap/CND-052-shc-codex-terminal-bench-16task-preserved.md` | 16 tasks, mean reward 0.6875 (11/16). Total task wall `4172.571406 s`. Reported **model** cost `$13.300340`. Preserved artifacts: 16-task tarball SHA256 `e2ed556c34f1d26640b95ee90cebfb67c3042b3c198dcc676064a6e1b2b76148`; 8-task tarball SHA256 `da824df860e96a11dde875875a66d02d703eb63743c51b5faaf0a6b045cd5771`. |
| Closeout | `docs/BENCHMARK_CLOUD.md` | Every run carries `openagents.resource_usage_receipt.v1`; subscription-backed Codex records `count_source=unavailable` (`subscription_backed_codex_no_token_counts`), which must not be treated as silently complete. Closeout authority identical to GCE (`authorityOwner=omega`, all wallet/payout/public-claim authority false). |

> Note on the SHC side: the `$` figures above are **Codex model/API** costs, not
> infra cost. The SHC host infra cost is now a **real invoice** ($1000/year,
> capex paid upfront — see §2.2); it still does not appear inside any per-session
> `resource_usage_receipt` (those carry refs/limits only, no cost), so §2's
> per-session SHC cost is an amortization of the real invoice, not a metered
> per-session receipt.

---

## 2. Cost split and per-session math

The contract input we are comparing is `cost_input_microusd` from
`openagents.compute_quota_routing.v1`:

```text
cost_input_microusd = floor(gcp_metered_cost_microusd × 1.10)
```

For an apples-to-apples lane comparison we reduce each lane to a **cost-plus-10%
micro-USD per VM-second** for the default `standard` compute class, then multiply
by a modeled default session length. All rates live in one place in code
(`oa_codex_control::placement_cost::LaneCostModel`, see Deliverable 2) so they
can be updated from a real GCP Billing Catalog pull / real SHC invoice without
touching placement logic.

### 2.1 Measured GCP cost (GCE lane)

- Receipt-measured: provisioning lifecycle only (CND-054). No VM-seconds, no
  dollar figure captured yet — see unsettled assumptions.
- Rate basis (list price, **not** yet a metered receipt): GCP `e2-small`
  on-demand, `us-central1` ≈ **$0.016751 / VM-hour** (2 shared vCPU, 2 GiB).
  This is the same machine type CND-054 actually provisioned and the
  `gce_capacity::LiveGceConfig` default.

```text
GCE raw         = 0.016751 / 3600 × 1e6   = 4.6531 micro-USD / VM-sec
GCE cost+10%    = 4.6531 × 1.10           = 5.1184 micro-USD / VM-sec
```

### 2.2 Real SHC invoice cost (SHC lane)

- Real invoice (capex, paid upfront): the `oa-shc-katy-01` host (16 vCPU /
  64 GB / 256 GB-NVMe dedicated-class) is **$1000.00 / year, paid upfront**
  (capex, not a $/month subscription, no per-second metering). Amortized over
  8760 hr/year:

```text
SHC $/hr          = 1000.00 / 8760              = 0.114155 / hr  (whole host)
SHC raw           = 0.114155 / 3600 × 1e6       = 31.7098 micro-USD / VM-sec
SHC cost+10%      = 31.7098 × 1.10              = 34.8808 micro-USD / VM-sec  (whole-host)
```

A flat-cost host is paid 24×7 whether or not a session is running, so the *fair*
SHC unit cost depends on how much of the host one session occupies and on host
utilization. Two reference amortizations for the `standard` class:

```text
SHC cost+10% @ 2-vCPU share (2/16)  = 34.8808 × (2/16)  = 4.3601 micro-USD / VM-sec
SHC cost+10% @ 8-vCPU share (8/16)  = 34.8808 × (8/16)  = 17.4404 micro-USD / VM-sec
```

### 2.3 Modeled SHC economics

Default modeled session length = **300 VM-seconds** (rounded from the CND-052
mean per-task wall of `4172.571406 / 16 ≈ 260.8 s`, used as a bounded
coding-session proxy).

| Lane / amortization | micro-USD / VM-sec (cost+10%) | per-300s session (floor micro-USD) |
| --- | --- | --- |
| **GCE `e2-small`** | **5.1184** | **1535** |
| SHC whole-host | 34.8808 | 10464 |
| SHC 2-vCPU share | 4.3601 | 1308 |
| SHC 8-vCPU share | 17.4404 | 5232 |

> Note: under the real $1000/yr invoice, the SHC **2-vCPU fair-share**
> (4.36 micro-USD/VM-sec, 1308 per session) is now **cheaper than GCE**
> (5.12 / 1535). This only holds if a single SHC host is packed with ~8
> concurrent 2-vCPU sessions; at single-tenant / low utilization the whole-host
> rate (10464 per session) dominates. The pilot has not demonstrated that
> packing, so the recommendation stays HOLD (see §3).

### 2.4 Unsettled assumptions

1. **GCE rate is list, not metered.** CND-054 captured no VM-seconds and no
   cost. The $0.016751/hr e2-small figure is GCP published list price, not a
   Billing-Catalog-API pull cached per region. Until a metered GCE receipt
   exists, GCE `cost_input_basis` for real sessions must be `unavailable`
   per the contract (no estimated figure in the receipt).
2. **SHC invoice — RESOLVED (real invoice).** The SHC host cost is a real
   invoice: **$1000.00/year, paid upfront (capex)**, not a modeled $120/mo
   subscription. The whole-host cost-plus-10% rate is now 34.8808 micro-USD/VM-sec
   (was 50.2283 under the model). `SHC_RAW_PER_VM_SEC_NANOUSD` = `31_710`.
3. **SHC amortization depends on utilization.** A flat-cost host competes with
   per-second GCE only if it is heavily and continuously utilized AND each
   session is given only a small vCPU share. Under the real invoice the 2-vCPU
   fair-share (4.36 micro-USD/VM-sec) is cheaper than GCE, but this requires ~8
   packed concurrent sessions; at single-tenant / low utilization the whole-host
   rate is still ~7× more expensive per session than GCE.
4. **Model cost is identical across lanes** (same Codex subscription/API), so it
   cancels out of the lane decision and is excluded from `cost_input_microusd`,
   which is infra-only by contract.

---

## 3. Recommendation: HOLD the SHC pilot

**HOLD** (do not expand, do not stop).

Reasoning grounded in the cost split (now using the **real $1000/yr SHC
invoice**, capex paid upfront):

- GCE `e2-small` cost-plus-10% (**5.12 micro-USD/VM-sec**, ~1535 per session) is
  cheaper than SHC under realistic single-/low-tenant utilization:
  - ~**7× cheaper** than SHC whole-host (34.88 / 10464),
  - ~**3.4× cheaper** than the 8-vCPU share (17.44 / 5232).
- The real invoice does flip one case: the **2-vCPU fair-share** is now
  **4.36 micro-USD/VM-sec (1308 per session), cheaper than GCE**. But that win
  is conditional — it requires a single SHC host packed with ~8 concurrent
  2-vCPU sessions continuously. The pilot has **not** demonstrated that packing,
  so the conditional 2-vCPU win is not yet a basis to promote SHC.
- GCE is per-second and scales to zero; the SHC host is paid 24×7 regardless of
  load, so SHC only beats GCE under sustained high concurrency the pilot has not
  shown.
- This aligns with the recorded owner direction that **Google GCE is the
  preferred lane**. SHC remains valuable as a *capacity fallback* (KVM-capable,
  Firecracker-ready, already bootstrapped per CND-041) but the data does not yet
  support promoting it to a cost-driven primary.
- We **HOLD rather than STOP** because: (a) the conditional 2-vCPU win is real
  but unproven at the required ~8-session concurrency; (b) the GCE cost is still
  list-price, not a metered receipt; and (c) SHC is the only proven warm
  fallback if GCE capacity is unavailable. Stopping would remove the fallback
  before the concurrency assumption is settled with measured receipts.

Revisit to "expand" only if metered GCE receipts land AND a real SHC run
demonstrates the ~8-concurrent-2-vCPU-session packing that makes the 2-vCPU
fair-share win actually achievable. Until then, cost-driven placement keeps GCE
primary (see Deliverable 2).

---

## 4. Follow-up issues

Filed for the missing integration / measurement surfaces surfaced by this
comparison (refs filled in at filing time, see issue thread on `cloud#89`):

- **cloud (CND-054-bis): capture metered GCE VM-seconds + `cost_input_microusd`
  on the live lane.** The live provisioner must record VM-seconds and a
  Billing-Catalog-derived cost (or `cost_input_basis=unavailable`) so the GCE
  side has a *measured* infra receipt, not a list-price estimate.
- **cloud: record the real SHC host invoice** — DONE (cloud#93). The real
  invoice ($1000/yr capex, paid upfront) is now the cost basis;
  `SHC_RAW_PER_VM_SEC_NANOUSD` = `31_710`.
- **Treasury/openagents.com Worker: consume `cost_input_microusd`** from
  placement/metering receipts for settlement; placement now surfaces the chosen
  lane's cost basis but does not settle.
- **Nexus/Forge/Probe/Psionic/Autopilot/public-Pylon:** no new integration is
  required by this comparison beyond consuming the existing refs-only
  `placement.bound` event; the cost-driven decision is internal to `cloud`
  placement. (No new issues filed for these surfaces; the `placement.bound`
  event contract is unchanged in shape, only enriched with a cost basis ref.)

---

## 5. How cost-driven placement uses this report

Deliverable 2 wires these numbers into `resolve_placement_binding`:

- A per-lane cost model (`placement_cost::LaneCostModel`) encodes the
  cost-plus-10% micro-USD/VM-sec for GCE and SHC from §2, with a single
  documented place to update the underlying rates.
- For an `Auto` (non-caller-pinned) assignment where **both** lanes are
  eligible, placement compares the lanes' cost-plus-10% estimates.
- **Google GCE wins ties and wins whenever it is cost-competitive** (owner
  direction). SHC is selected only when it is BOTH *materially cheaper* (beyond
  a margin) AND the report recommendation is `expand`. Because this report
  recommends `HOLD` and GCE is cheaper anyway, cost-driven placement resolves to
  GCE today.
- `cost_driven` is set `true` with a `cost_basis` ref recorded on the binding
  (refs/limits only, no raw cost in public-facing refs).
- A config flag `OA_CODEX_PLACEMENT_COST_DRIVEN` (default `true`, per this
  report's recommendation) keeps the policy-driven Google-first path available.
