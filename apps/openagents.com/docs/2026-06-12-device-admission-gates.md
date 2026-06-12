# Reasoned Device Admission Gates + Host-RAM Probe (Pluralis Roadmap P1.4)

Date: 2026-06-12
Issue of record: openagents#4852 (master tracking issue openagents#4855)
Rails: #4681 (CS336 A2 device-capability dataset), #4848 (join-lifecycle
ladder, P0.1)
Roadmap source: `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
(workspace `openagents` repo), item P1.4

## The pattern: no reasonless gates

Pluralis ships its hardware exclusions with their reasons attached. The
canonical example: T4 and V100 cards are excluded from bf16-class work
because they would emulate BF16 *slower than FP32* — the measured reason
travels with the gate instead of living in a maintainer's head. This
change adopts that pattern into the device-capability dataset:

- Every device class **admitted** to a work class carries its measured
  reason.
- Every **exclusion** carries one too, surfaced through funnel reason
  codes in the existing `*.public.*` namespace style
  (`device_admission.public.excluded_host_ram_headroom_below_floor`).
- A reasonless decision is unrepresentable: the decision record schema
  requires `statedReason` and `reasonCode` on both branches, and the
  validation layer rejects blank reasons, branch-mismatched reason
  codes, and decisions that contradict their own measured value,
  comparison, and threshold.

## What landed

`workers/api/src/training-device-admission-gates.ts`:

1. **`DeviceAdmissionGateDefinition`** — versioned gate definitions:
   `gateRef`, `workClassRef`, a requirement
   (`measurementKind`, `threshold`, `comparison: at_least | at_most`,
   `unit`), a stated `rationale`, and the paired
   `admittedReasonCode` / `excludedReasonCode` funnel constants.
2. **`DeviceAdmissionDecisionRecord`** — the typed decision:
   `{ decision: admitted | excluded, gateRef, measuredValue,
   reasonCode, statedReason, ... }` with `statedReason` REQUIRED on
   both branches. `evaluateDeviceAdmissionGate` composes the stated
   reason from the measured value, the requirement, and the gate
   rationale; `assertAdmissibleDeviceAdmissionDecision` rejects
   reasonless, contradictory, or private-material-bearing records.
3. **Funnel surfacing** — decision reason codes are funnel-compatible
   refs (`funnelReasonRefForDeviceAdmissionDecision`); the capacity
   funnel's privacy scanner exempts the platform-issued
   `device_admission.public.*` taxonomy the same way it exempts
   `dark_capacity.public.*` (the wallet_not_ready lesson of
   2026-06-11), so exclusions can join `darkCapacityReasonRefs`
   without tripping the substring scan.
4. **Qualification probe schema extension**
   (`workers/api/src/training-device-capability.ts`) — two new
   measurement kinds join the CS336 A2 qualification payload, evidence
   admission, and public projection:
   - `host_ram_headroom_gb` (unit: gigabytes)
   - `sustained_vs_burst_throughput_ratio` (unit: ratio, in (0, 1])
   These are additive: the kernel-executable benchmark tuple
   (`Cs336A2BenchmarkMeasurements`) is unchanged, existing fixtures
   and the bounded workload module keep working, and the new kinds
   ride the same admission/validation/privacy path
   (`min <= p50 <= p90 <= max`, receipt refs required, projection
   privacy guard).

## Seeded example gate set (DEFINITIONS, not live policy)

`EXAMPLE_DEVICE_ADMISSION_GATE_SET` demonstrates the pattern with three
gates. They are definitions only — no device has been measured against
them and no funnel row may cite them as a live admission claim:

| Gate | Requirement | Stated rationale |
| --- | --- | --- |
| `gate.device_admission.example.bf16_attention_throughput_floor.v1` | `attention_throughput` at_least 2000 megaflops | bf16-class work needs native bf16 throughput; Pluralis excluded T4/V100 because they would emulate BF16 slower than FP32 |
| `gate.device_admission.example.host_ram_headroom_floor.v1` | `host_ram_headroom_gb` at_least 80 gigabytes | optimizer-offload work keeps Adam moments in host RAM; the Pluralis contributor shape is 24 GB GPU + 80 GB system RAM |
| `gate.device_admission.example.sustained_throughput_ratio_floor.v1` | `sustained_vs_burst_throughput_ratio` at_least 0.8 | one thermally throttling GPU collapsed a 14-node Pluralis collective; burst benchmarks overstate sustained capability |

## Why host RAM, specifically

Pluralis's contributor hardware shape is 24 GB GPU plus 80 GB system
RAM because Adam optimizer moments are offloaded to host RAM. GPU
memory alone does not predict whether a device can hold an
optimizer-offload work class: host-RAM headroom is a binding constraint
our benchmark suite did not measure. `host_ram_headroom_gb` makes it a
first-class qualification measurement so the admission gate can state
the floor and the measured value in the same record.

## Why sustained-vs-burst, specifically

A collective runs at the pace of its slowest member. Pluralis observed
a single thermally throttling GPU collapse a 14-node collective: the
device benchmarked fine in bursts and degraded under sustained load.
`sustained_vs_burst_throughput_ratio` records sustained throughput as
a fraction of burst throughput, so thermal behavior joins the same
admission record instead of being discovered mid-window.

## Psionic preflight seam (cross-repo, not edited here)

`exportDeviceAdmissionGateContract()` returns the versioned, deeply
frozen, JSON-able gate-definition contract:

```jsonc
{
  "schemaVersion": "openagents.training.device_admission_gates.v1",
  "definitionsOnly": true,
  "liveAdmissionClaim": false,
  "gates": [ /* DeviceAdmissionGateDefinition[] */ ],
  "policyRefs": [ "policy.public.device_admission.psionic_preflight_consumes_this_contract", ... ]
}
```

Psionic's preflight qualification (the actual-pretraining lane already
does GPU/memory/thermal checks) consumes this exact structure rather
than duplicating gate thresholds. The schema-version string is the
compatibility key: psionic mirrors the contract by version, and a gate
change on the openagents side is a version bump, not a silent drift.
This document records the seam; the psionic-side consumption is its own
change in the psionic repo and is **not** claimed here.

## Verified vs remaining gate

Landed and test-covered (`smoke:cs336-a2:device-capability` +
`src/training-device-admission-gates.test.ts`):

- gate evaluation admits and excludes with stated, measured reasons on
  both branches (`at_least` floors and `at_most` ceilings),
- reasonless, branch-mismatched, self-contradicting, and
  private-material-bearing decisions are rejected,
- the exported contract is frozen, JSON-round-trip stable, versioned,
  and marked definitions-only,
- the new measurement kinds flow through evidence admission and public
  projection without breaking existing fixtures, and the privacy
  guards still hold,
- exclusion reason codes project through the capacity funnel as
  `darkCapacityReasonRefs` without tripping the privacy scanner.

Remaining hardware-gated acceptance bullets, **not** claimed here:

- a live device measuring `host_ram_headroom_gb` or
  `sustained_vs_burst_throughput_ratio` and submitting receipted
  evidence (the bounded workload module cannot synthesize either; live
  values wait on real contributor devices),
- any live admission or exclusion decision against the seeded gates —
  the example set stays definitions-only until receipted device
  evidence exists,
- the psionic preflight consuming the exported contract (cross-repo
  seam, psionic-side change).
