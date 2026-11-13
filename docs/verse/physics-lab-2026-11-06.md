# Physics Lab results, November 6, 2026

The Physics Lab runs all six requested scenes through the OpenAgents Metaverse Engine, Rapier 3D
and Jolt with one generated input table and one Coax metric observer.
All six the Metaverse Engine repeats match every recorded physical-state byte. The
results expose unresolved physics and performance gaps; deterministic
replay does not establish solver quality.

## Run

On coderos-4080, November 6, **04:54:25–04:55:31 Chicago time (CST)**:

```sh
nice -n 15 env COAX_CHECK_JOBS=6 \
  coax/web/tool.sh aethyr/lab/physics-lab.coax \
  --skip-build --out aethyr/lab/.cache/all-results
```

The reference cache was built by the same runner's default preparation
path during the stack rehearsal. The complete command then ran all three
engines, repeated the Metaverse Engine and captured all six scenes in 65.5 seconds. Its
exit status was 0. No room/page code or Wasm budget changed.

The host is an Intel Core i7-14700K with 28 logical CPUs. Every engine uses
one simulation thread and inherits nice 15. This is a shared host without
CPU isolation; timings include its scheduling conditions. GPU models were
left alone; captures used muted headless Chromium with SwiftShader.

The [lab README](../../aethyr/lab/README.md) defines the scenes, profiles,
precision, fixed step, timing interval, rest threshold and sampled overlap
metric. The Metaverse Engine uses Coax 0.51.2; the Linux compiler SHA-256 is
`088782b2838c3119eeb5b352f9ef36144d08171da8b910e1a0a79d93e401ee4e`.
The source delivery is `38b687866f`, following the scene/record contract
at `96c3957ffd`. Source hashes retain the measured files. The later public
clock helper/selftest does not change the foreign clock or physics kernels
used by this run.

## Step timings

Times are milliseconds. Percentiles cover all 180 advances; snapshots,
file I/O, metrics and captures are excluded.

| Scene | Engine | p50 | p95 | p99 |
| --- | --- | ---: | ---: | ---: |
| Fall 1,000 | Metaverse Engine | 0.847 | 4.292 | 5.087 |
| Fall 1,000 | Rapier | 1.508 | 1.550 | 1.639 |
| Fall 1,000 | Jolt | 0.498 | 0.722 | 0.788 |
| Fall 10,000 | Metaverse Engine | 9.017 | 39.391 | 51.680 |
| Fall 10,000 | Rapier | 21.201 | 21.621 | 22.776 |
| Fall 10,000 | Jolt | 5.528 | 7.866 | 8.527 |
| Fall 30,000 | Metaverse Engine | 28.752 | 115.198 | 146.631 |
| Fall 30,000 | Rapier | 93.719 | 95.424 | 97.195 |
| Fall 30,000 | Jolt | 19.155 | 26.041 | 27.604 |
| Stack, 32 | Metaverse Engine | 1.495 | 2.814 | 3.057 |
| Stack, 32 | Rapier | 0.079 | 0.088 | 0.159 |
| Stack, 32 | Jolt | 0.109 | 0.119 | 0.123 |
| Pile, 128 | Metaverse Engine | 20.507 | 22.528 | 23.220 |
| Pile, 128 | Rapier | 0.271 | 0.707 | 1.694 |
| Pile, 128 | Jolt | 0.419 | 1.617 | 1.848 |
| Slope, 64 | Metaverse Engine | 0.067 | 0.136 | 0.137 |
| Slope, 64 | Rapier | 0.102 | 0.140 | 0.185 |
| Slope, 64 | Jolt | 0.063 | 0.082 | 0.130 |

## Physics after three seconds

Energy is joules; overlap is the final sample's maximum penetration in
metres. Rest counts use speed thresholds, with sleeping disabled in every
engine. Full energy/overlap/rest time series are in
[results.json](physics-lab-2026-11-06/results.json).

| Scene | Engine | Energy | Overlap | At rest |
| --- | --- | ---: | ---: | ---: |
| Fall 1,000 | Metaverse Engine | 5,457.058 | 0.002000 | 0/1,000 |
| Fall 1,000 | Rapier | 4,904.329 | 0.000092 | 1,000/1,000 |
| Fall 1,000 | Jolt | 4,904.957 | 0.000247 | 1,000/1,000 |
| Fall 10,000 | Metaverse Engine | 54,786.774 | 0.002000 | 0/10,000 |
| Fall 10,000 | Rapier | 49,043.290 | 0.000092 | 10,000/10,000 |
| Fall 10,000 | Jolt | 49,049.546 | 0.000327 | 10,000/10,000 |
| Fall 30,000 | Metaverse Engine | 164,140.742 | 0.002000 | 0/30,000 |
| Fall 30,000 | Rapier | 147,129.882 | 0.000092 | 30,000/30,000 |
| Fall 30,000 | Jolt | 147,148.617 | 0.000327 | 30,000/30,000 |
| Stack, 32 | Metaverse Engine | 415.701 | 0.004006 | 0/32 |
| Stack, 32 | Rapier | 4,965.737 | 0.017184 | 32/32 |
| Stack, 32 | Jolt | 4,991.797 | 0.010536 | 1/32 |
| Pile, 128 | Metaverse Engine | 1,254.984 | 0.003858 | 0/128 |
| Pile, 128 | Rapier | 1,049.831 | 0.001842 | 123/128 |
| Pile, 128 | Jolt | 1,233.689 | 0.002306 | 128/128 |
| Slope, 64 | Metaverse Engine | −6,229.608 | 0.002000 | 0/64 |
| Slope, 64 | Rapier | −5,908.481 | 0.000084 | 0/64 |
| Slope, 64 | Jolt | −5,921.260 | 0.000005 | 0/64 |

The Metaverse Engine is **ahead of Rapier in falling-scene median step time** in these
profiles, and slightly ahead in slope p95. That does not mean comparable
quality. It is **behind at falling-scene p95/p99**, behind Jolt on these
falling timings, and much slower in the contacting stack and pile. At
30,000 bodies, the Metaverse Engine's p95 is 115.198 ms versus Rapier's 95.424 ms and
Jolt's 26.041 ms. The 128-box pile's Metaverse Engine p95 is 22.528 ms versus 0.707
ms and 1.617 ms.

The Metaverse Engine is **wrong as a settling/stack solver in this composition**: none of
the falling boxes reach the common rest thresholds by three seconds,
while both references settle all of them. Its 32-box stack collapses,
retaining 415.701 J from 5,071.378 J initially; both references keep a
standing stack with roughly 4,966–4,992 J. Jolt's stack still moves under
the strict rest threshold, so it is not reported as fully settled.
The Metaverse Engine's smaller final stack penetration follows collapse and is not a
stack-quality win.

The common pile starts with 0.585156 m overlap in all engines. Its maximum
sampled overlap therefore includes that initial condition. Rapier and
Jolt also reach about 0.142–0.143 m sampled ground penetration during the
falling impact at one second. The JSON retains those transient values;
final penetration alone does not describe collision quality. No thresholds
or solver tuning were changed to hide these results. This issue delivers
the measuring lab, not a claim that the Metaverse Engine's solvers now match references.

## Determinism and captures

All seven samples of every body match between the first and second Metaverse Engine
run, excluding elapsed-time bytes. The Metaverse Engine's exact sample checksums are:

| Scene | FNV-1a checksum |
| --- | --- |
| Fall 1,000 | `8e988f5b87e8d154` |
| Fall 10,000 | `3e2a7c42b2503f85` |
| Fall 30,000 | `f55c94acb7eb9a32` |
| Stack | `24bbe4ff8b61a69e` |
| Pile | `8a3dcf1389bf5a0f` |
| Slope | `3e34c7fb440a71fe` |

The capture uses each engine's actual final positions and rotations, with
one camera/scale and common selected IDs per scene. It shows all bodies
for small scenes, 1,000 for Fall 1,000, and 2,000 explicitly labelled
samples for the two larger scenes. Measurements always include all bodies.
All six saved captures were visually inspected:
[Fall 1,000](physics-lab-2026-11-06/fall-1k/compare.png),
[Fall 10,000](physics-lab-2026-11-06/fall-10k/compare.png),
[Fall 30,000](physics-lab-2026-11-06/fall-30k/compare.png),
[stack](physics-lab-2026-11-06/stack/compare.png),
[pile](physics-lab-2026-11-06/pile/compare.png),
[slope](physics-lab-2026-11-06/slope/compare.png).

## Checks and retained failures

The scene, Metaverse Engine kernel and metric checks were written red first. The
final full lab suite passes all 63 checks, including a conservative-sweep
check against all negative SAT overlaps in its rotated fixture.
The physical runner also rebuilds with that added test before the full
six-scene run. The scene authority kills all 116 root mutants, with zero
survivors or equivalent declarations. Native system selftests pass on
macOS and Linux, including nanosecond scale and agreement with the
monotonic millisecond clock. The tools gate reports zero new files without
a firm reason.

The first Jolt rehearsal aborted in `TempAllocatorImpl::Allocate` while
preparing its oversized one-million-constraint buffer with 128 MiB scratch.
Its [backtrace](physics-lab-2026-11-06/jolt-allocator-failure.log) is retained.
The delivered profile bounds constraints at 131,072 and provides 256 MiB
scratch; update-capacity errors still fail. The corrected standalone stack
rehearsal and complete run both finish. That failed adapter attempt is not
counted as a successful Jolt measurement.

The first extra broad-phase check incorrectly treated a positive SAT
separation distance as overlap; the corrected check requires a negative
distance. No physics code changed for that correction. Earlier scene
mutation survivors were resolved by fixed coordinate fixtures and simpler
canonical layout tables, without equivalence waivers.

Receipts include the [run log](physics-lab-2026-11-06/run.log),
[final checks](physics-lab-2026-11-06/checks-final.log),
[extra broad-phase check](physics-lab-2026-11-06/broad-check.log),
[mutation result](physics-lab-2026-11-06/scene-mutation.log),
[machine and binary identities](physics-lab-2026-11-06/machine.txt),
[source identities](physics-lab-2026-11-06/source-sha256.txt) and
[raw-record SHA-256 manifest](physics-lab-2026-11-06/raw-sha256.txt).
Large binary records are excluded from Git and retained for this session
under `/tmp/oa12959-artifacts/`. The runner regenerates them. References
remain lab-only; no production deployment, book, Psionic code or GPU model
was changed.
