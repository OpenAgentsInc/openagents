# CS336 A2 Device-Capability Dataset: A Genuine Second Device Class (x86_64 Linux / Intel)

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-20

Promise: `training.device_capability_dataset.v1` (stays **yellow** — no green
flip). Registry edit: `2026-06-20.7`.

Issue lineage: `OpenAgentsInc/openagents#4681` (the public device-capability
dataset), `#4852` (qualification-probe host/thermal kinds).

## What this advances

The public device-capability dataset previously covered exactly **one** device
class — `device_class.apple_silicon_macos.arm64`, the settled #4681 closeouts
from two Pylons on a single physical Mac. The promise's open blocker
`blocker.product_promises.second_device_class_missing` required at least two
**distinct** device classes captured with real capability/benchmark data.

This pass adds a **genuinely distinct, real-measured second device class**:

- `device_class.x86_64_linux.intel`

It is **measured-only**, not paid and not cross-check verified, and is labeled
honestly as such (`measurementProvenance: measured_unsettled`,
`crossCheckState: measured_unverified`, `verified: false`, no earning
estimate). The `second_device_class_missing` blocker is therefore **dropped**;
`thermal_throttle_detection_missing` and `same_host_replication_caveat`
**remain**.

## The second device class (real hardware)

A live Tailnet contributor node was characterized from on-device signals
(class/arch only — never an identifier):

| Property | Value |
| --- | --- |
| Device class ref | `device_class.x86_64_linux.intel` |
| Architecture | x86_64 (distinct from the Apple-Silicon class's arm64) |
| OS | Linux 6.19 (distinct from macOS) |
| CPU | Intel Core i7-14700K, 28 logical cores |
| System RAM | ~125 GB |
| Runtime | Node v25.8.2 |

This is a real, distinct class on three axes (ISA, OS, vendor) from the
existing Apple-Silicon class.

## What actually computed

The contributor ran the **exact** bounded CS336 A2 suite
(`benchmark_suite.cs336_a2.pylon_runtime_device_capability.v1`) — the same four
kernels in `apps/openagents.com/workers/api/src/cs336-a2-benchmark-workload.ts`
(seeded `attention_throughput`, `memory_bandwidth`, `tokens_per_second`,
`step_time_ms`) — at 24 repetitions, with wall-clock timing and per-kernel
deterministic output digests.

### Cross-architecture exactness (the honesty anchor)

The deterministic output digests produced on the x86_64 Linux box are
**byte-for-byte identical** to the digests produced by the in-repo workload and
by the Apple-Silicon class. Identical commitments across heterogeneous hardware
prove the same numeric work ran; only the timing distribution differs.

| metric | output digest (sha256) |
| --- | --- |
| `attention_throughput` | `70b508a8a655e0b0e14c0535323f18f9a17a37dfba37bf8d3b47dfb81c7880b3` |
| `memory_bandwidth` | `02d2cf92913ee000487aa356bb39bce5e3e754ebd04913089b80cdc89ec876f3` |
| `tokens_per_second` | `6b8502b3d1f381d12a37807c8ddb2b14069fba3c3f0fb111c2a1542cd4d10a0a` |
| `step_time_ms` | `1bde26dbb6c833ce15fb76b7b1e3136b4938c523d6fe5b73ed87e6b40459cac3` |

Verified equal three ways: (1) the in-repo `runCs336A2BenchmarkSuite` on this
machine, (2) a faithful standalone mirror of the same kernels, (3) the same
mirror run on the x86_64 Linux device.

### Measured distribution (x86_64 Linux / Intel i7-14700K, 24 reps)

| metric | p50 | p90 | min | max | unit | samples |
| --- | --- | --- | --- | --- | --- | --- |
| `attention_throughput` | 3109.9212 | 3169.043 | 174.2336 | 3203.387 | megaflops | 24 |
| `memory_bandwidth` | 10.6705 | 12.3328 | 8.2227 | 13.5337 | gigabytes_per_second | 24 |
| `tokens_per_second` | 352479.8426 | 364722.2412 | 13096.9515 | 366764.9801 | tokens_per_second | 24 |
| `step_time_ms` | 0.1018 | 1.6674 | 0.0968 | 2.9134 | milliseconds | 24 |

These are genuinely different from the Apple-Silicon class (e.g. the Intel box
sustains markedly higher attention throughput and memory bandwidth in this
single-thread JS suite), which is the point of a device-capability dataset.

## Provenance model added (schema)

`apps/openagents.com/workers/api/src/training-device-capability.ts` now carries
a typed `measurementProvenance`:

- `settled_cross_checked` (default; the original receipted basis) — may carry a
  settlement receipt and a modeled-from-measured earning estimate, and becomes
  `verified` at the same-class sample/verdict bar.
- `measured_unsettled` (new) — genuinely measured but **not** paid and **not**
  cross-check verified. Admission **enforces**: at least one
  `digestCommitmentRef`, **no** settlement receipt, **no** earning estimate; a
  run-level verdict is never borrowed onto such a row; `crossCheckState` is
  pinned to `measured_unverified` and `verified` is always `false`.

The projection now also reports `observedSettledDeviceClassCount` next to
`observedDeviceClassCount`, so a reader can tell total observed class coverage
(now 2) apart from settled+verified class coverage (still 1).

## Honest remainder (what green still needs)

Green for `training.device_capability_dataset.v1` still requires, beyond the
second class now present:

- **Paid + cross-check verified second-class coverage.** The Intel class is
  measured-only. Green requires a paid CS336 A2 benchmark assignment dispatched
  to this class, a `statistical_cross_check` verdict against a **second
  same-class Intel device**, and a settled closeout receipt — exactly the path
  the Apple-Silicon class already has. `thermal_throttle_detection_missing` and
  `same_host_replication_caveat` are unchanged.
- **Real capture still needed for the Intel class to settle:** a second
  independent x86_64-Linux/Intel contributor (for same-class replication), an
  operator-dispatched paid assignment with the `cs336_a2_device_benchmark`
  job kind, a validator verdict, and a Lightning closeout. None of that was
  fabricated here; the rows are admitted explicitly as `measured_unsettled`.

## Scope / safety

- No device identifiers, wallet, payment, mnemonic, or path material appears in
  the dataset rows or this doc. Class/arch labels and digests only.
- No green flip; green count unchanged at 24. No promise_transition required
  (yellow → yellow).
- The contributor runner
  (`apps/openagents.com/workers/api/scripts/cs336-a2-device-benchmark.ts`) now
  emits the canonical Linux class ref and admission-ready `measured_unsettled`
  measurement rows so this path is reproducible by any x86_64-Linux Pylon.
