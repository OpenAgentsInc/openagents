# CS336 A2 Device Capability Dataset

Date: 2026-06-10

Issue: `OpenAgentsInc/openagents#4681`

Forum claim: `https://openagents.com/forum/t/16402e92-d50f-43b2-90bd-7050d6a45ad5`

## Scope

This slice adds the Worker-side CS336 A2 benchmark homework contract and public
device-capability projection for `pylon.compute_revenue_modes.v1`.

The public route is:

- `GET /api/training/device-capabilities/a2`

The no-spend smoke is:

- `bun run smoke:cs336-a2:device-capability`

## Dispatch Contract

The dispatchable job kind is `cs336_a2_device_benchmark`. The job payload is
created by `buildCs336A2DeviceBenchmarkPayload` and names:

- benchmark suite:
  `benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1`
- request schema: `openagents.cs336_a2_device_benchmark_request.v1`
- output schema: `openagents.cs336_a2_device_benchmark_output.v1`
- verification class: `statistical_cross_check`
- measurements: attention throughput, memory bandwidth, tokens/sec, and step
  time.

This contract consumes benchmark surfaces that Pylon already ships. Owned
Metal/CUDA attention kernels and real-transport DDP/FSDP remain Psionic-side
external asks.

## Public Projection

The public projection is intentionally class-level:

- device class ref
- measurement metric and unit
- sample count
- p50/p90/min/max distribution values
- receipt refs
- verification refs
- cross-check state
- modeled-from-measured earning estimates by work class

It rejects device identifiers, Pylon refs, owner refs, wallet material,
payment material, raw benchmark payloads, private paths, and secret-shaped
material before a public dataset is built.

Earning estimates are always labeled
`modeled_from_measured_benchmark_distribution`. They are planning estimates,
not payout promises, accepted-work receipts, or settlement evidence.

## Evidence Admission (added 2026-06-11)

`POST /api/training/runs/{trainingRunRef}/device-benchmark-evidence`
(admin bearer) is the admission seam for receipted measurement rows.
It validates min <= p50 <= p90 <= max, an integer sampleCount >= 1, and a
non-empty receiptRefs list per row, then runs the same privacy guard the
public projection enforces — at admission time — before a single-statement
D1 `UPDATE` merges `a2DeviceBenchmark` into the run projection. OpenAPI
operation: `admitTrainingA2DeviceBenchmarkEvidence`.

The device-side suite lives in `src/cs336-a2-benchmark-workload.ts`
(runner: `scripts/cs336-a2-device-benchmark.ts`): real timed kernels for
all four measurement kinds with deterministic output digests, nearest-rank
aggregation, the min-over-max same-class median agreement score for
`statistical_cross_check`, and the modeled sats/hour helper.

## Current Live Boundary

With no receipt-backed benchmark rows, the route returns an empty dataset
plus blockers:

- `blocker.cs336_a2.requires_receipted_benchmark_results`
- `blocker.cs336_a2.requires_statistical_cross_check`
- `blocker.cs336_a2.requires_replication_across_same_class_devices`

As of 2026-06-11 the live dataset left the empty state: see
`docs/2026-06-11-cs336-a2-device-capability-paid-closeout-evidence.md`
for the first receipted rows (two paid Pylon assignments, four
statistical_cross_check verdicts, two settled public receipts) and the
named remaining gaps (single physical host, one device class, no thermal
detection).
