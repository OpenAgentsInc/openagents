# Scene and rigid broadphase evidence

The primary comparison uses `pinned-baseline.json` and `integrated.json` on an
Intel Core i7-14700K, CPU 0, with a 0.2-second CPU warm-up. CPU affinity applies
to this process; the core is not reserved and concurrent machine work remains
uncontrolled. Each row performs 200 rays, overlaps, and sweeps against mixed mesh
and capsule geometry, then 60 rigid steps with eight slowly moving spheres and
a distant static background. Release optimization is enabled. This is an
isolated fixture, with no display, host, network, or credentials.

| Colliders | Exhaustive rigid step p95 (ms) | Integrated rigid step p95 (ms) |
| --- | ---: | ---: |
| 8 | 0.000851 | 0.001580 |
| 64 | 0.013982 | 0.007137 |
| 256 | 0.078089 | 0.023157 |
| 1,024 | 0.896426 | 0.091899 |
| 4,096 | 12.634588 | 0.408151 |

At 4,096 colliders, exhaustive detection enumerates 8,386,560 pairs. Indexed
detection visits 4,320 scene nodes and selects zero pairs; this sparse fixture
has no rigid contacts. The first integrated step, including initial tree
construction, takes 2.082 ms. Ray/overlap/sweep p95 changes from
280.421/130.914/158.607 µs to 0.961/0.349/8.384 µs. Query candidates remain one
as distant colliders increase; top-level node visits rise from 7 to 27. Query
hits remain `[4, 1, 6]` in every row. Tree maintenance adds cost in small scenes.

The moving query fixture updates 1,024 capsule poses and performs 1,024
self-excluding, half-meter sweeps per tick for 60 ticks. It records 59,520 hits
from 61,440 sweeps, with 59,520 capsule narrow-phase tests. Pose-update p95 is
0.303 ms; the sweep phase p95 is 1.014 ms. This is scripted movement and collision
query work, not crowd avoidance or a complete character-controller simulation.

Separate closed contact fixtures place 8, 64, and 256 touching spheres in a line
with alternating velocities and advance 120 steps. The 256-body fixture performs
30,600 narrow-phase/contact-point operations. Detection/solve/total p95 is
0.062/0.235/0.300 ms. Final linear momentum residual is about `7.03e-16` kg m/s;
angular residual is zero. Energy decreases by 1.275 J from 1.280 J, as expected
for this dissipative contact setup. Three-dimensional conservation, stacks,
supports, and rotated contacts also retain their existing unit-test checks.

Receipts bind exact source and executable hashes. The reconstructed pinned
baseline executable matches the original baseline executable's SHA-256. Its
seven implementation files are retained with `pinned-baseline-` names; earlier
baseline and unpinned runs remain available. Earlier indexed source files are
retained where later changes alter their hashes. `before-shared-mesh-queries.rs.txt`
binds the runs before main's shared mesh buffers merge. `integrated.json` follows
that merge and records chronological rigid-step and crowd-phase samples, including
startup. Baseline `detect` timing includes force integration; integrated `detect`
starts at detection. The table compares the unchanged total-step boundary.

`integrated-tests.log` records 123 physics, 586 Verse, and 315 world tests passing,
with one physics oracle and 14 GPU/platform tests ignored. Tests compare indexed
hits/manifolds and exact serialized simulation state against exhaustive paths
through transforms, filtering, truncation, wake-up, fast motion, removal/reuse,
and restoration. The tree test checks balance, bounds, parents, and storage reuse
through 3,000 updates. Native desktop/mobile and browser compilation checks and
formatting pass. No Clippy, release gate, phone suite, live owner host, or Genesis
installation is involved.

Reproduce with the pinned toolchain and a warm target directory:

```sh
export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1"
cargo test -p physics -p verse-world -p verse --no-default-features --lib
cargo build -p physics --example scene_broadphase --release
python3 - <<'PY'
import os, pathlib, subprocess, time
os.sched_setaffinity(0, {min(os.sched_getaffinity(0))})
deadline = time.perf_counter() + 0.2
while time.perf_counter() < deadline:
    pass
binary = pathlib.Path(os.environ['CARGO_TARGET_DIR']) / 'release/examples/scene_broadphase'
subprocess.run([str(binary), '/tmp/scene-broadphase.json'], check=True)
PY
```

Choose an allowed performance core for a comparable heterogeneous-CPU run. To
reconstruct the exhaustive baseline, replace the seven source files in a separate
checkout with their retained `pinned-baseline-` text, then build the same example.
Use that checkout's own warm target directory. Measurements apply to their
recorded revisions and workload; they are observations rather than a hardware
performance gate. Dense overlaps, loose sphere bounds, large motion margins,
and selective filters can still produce large candidate sets. Rigid steps
retain linear synchronization and a sequential solver. Arbitrary-margin detection,
rigid sensor rays, and plume scans retain their existing enumeration. General
rotating-body time of impact, parallel solve, and whole-game tick budgets remain
separate workload acceptance work.

The final measurement precedes main's later journal-value reuse change. Physics
implementation hashes remain identical after that rebase. `rebased-checks.log`
records the focused world checkpoint and indexed rigid replay checks passing on
the integrated publication tree.
