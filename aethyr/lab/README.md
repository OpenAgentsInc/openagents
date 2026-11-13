# Physics Lab

Run the six seeded scenes through the OpenAgents Metaverse Engine, Rapier and Jolt on coderos-4080:

```sh
nice -n 15 env COAX_CHECK_JOBS=6 \
  coax/web/tool.sh aethyr/lab/physics-lab.coax
```

Run this command from an isolated OpenAgents worktree on coderos-4080.
It builds the pinned reference SDKs in `aethyr/lab/.cache/`, runs each
engine sequentially, repeats the Metaverse Engine, prints the timing and physics table,
and writes `aethyr/lab/.cache/results/results.json`. Each scene directory
contains its input, three engine records, the Metaverse Engine's repeat, `compare.html`
and a muted, headless `compare.png`.

The first build needs Git, Cargo, Nix and network access. Cargo's committed
lockfile pins transitive dependencies. Nix supplies CMake and Ninja; their
versions are build tools rather than physics dependencies. No Nix file is
added outside `os/nix/`. Reference SDK builds are refused on macOS. They
are isolated references under the tools-in-Coax SDK exception; scene
construction, orchestration, record checking, metrics and drawing are Coax.
No product build loads these adapters or reference libraries.

## Options

| Option | Meaning |
| --- | --- |
| `--out DIR` | Output directory; created with its parents |
| `--seed N` | Unsigned 32-bit seed; default `20261106` |
| `--scene NAME` | Run `fall-1k`, `fall-10k`, `fall-30k`, `stack`, `pile` or `slope` |
| `--skip-build` | Use an already built, qualified reference cache |

The command has a 14-minute deadline, including child builds and captures.
A timeout kills only the owned child process group. If a cold build cannot
fit, prepare its cache separately, then use `--skip-build`; a scene slice
can isolate a slower solver. The recorded six-scene run took 65.5 seconds.

## Shared inputs

The [v1 record contract](../../proto/aethyr/physics-lab-v1.json) precedes the
runners. `physics-scene.coax` generates one dense body table, using the Metaverse Engine's
seed/stream/counter random draws. All adapters read the same file. The
observer requires every byte of each engine's initial state to match that
table before publishing a result.

Every box is 1 m on each side, with mass 1 kg, friction 0.5 and restitution
`f32(0.1)`. A static infinite plane supplies the ground. The slope's normal
is `(0, f32(0.8), f32(0.6))`. There are 180 advances at `f32(1/60)` seconds,
with gravity `(0, -f32(9.81), 0)`. Initial physical values are rounded to
binary32 and promoted to binary64 so the input bits are shared. Reported
three-second times are rounded; the actual duration is 3.000000156 seconds.

The scenes contain 1,000, 10,000 and 30,000 separated falling boxes, a
32-box vertical stack, a 128-box two-layer overlapping pile, and 64 boxes
above a friction slope. The pile intentionally starts with overlap; its
initial maximum penetration is reported, rather than attributed to a step.

All engines disable sleeping, damping and CCD. Rest counts instead use the
same physical thresholds: linear speed below 0.05 m/s **and** angular speed
below 0.05 rad/s. Zero initial velocity counts as initially at rest, even
for a body about to fall. The slope is steep enough to slide at friction
0.5, so zero final rest counts there are expected.

## Solver profiles and limits

The Metaverse Engine composes the existing `rigid_step`, `contact_box_plane`,
`contact_resolve` and box SAT kernels. Its lab-owned merge-sorted sweep
uses conservative circumscribed-box bounds and caller-owned arrays for
30,000 bodies and one million candidate pairs. Pair-capacity exhaustion
fails the run. Each of eight solve passes resolves the deepest ground
corner, then current SAT manifold points for the step's candidate pairs.
This is a measured lab composition of the kernels; it does not expand the
production `sim/rigid` or `sim/broad` fixed containers. It has no persistent
contact cache, warm starting or sleeping islands.

Rapier 3D 0.36.1 is pinned to
`3406750a38286a0d3c7d0f4be2f9d53153e81d90`. It uses `rapier3d-f64`,
`enhanced-determinism`, eight solver iterations and its remaining upstream
integration defaults. Parallel and block-solver features are disabled.
See the upstream [simulation structures](https://rapier.rs/docs/user_guides/rust/simulation_structures/)
and [integration parameters](https://rapier.rs/docs/user_guides/rust/integration_parameters/).

Jolt 5.6.0 is pinned to `e77f175595e64cb44218cc9d9d56fc365ad0e36a`.
It uses double positions, float velocity/rotation, deterministic mode,
`JobSystemSingleThreaded`, zero box convex radius, eight velocity steps,
eight position steps, 2 mm penetration slop and 0.4 m/s restitution
threshold. Capacities are 32,768 bodies, 262,144 body pairs and 131,072
contact constraints, with a 256 MiB scratch allocator. An update capacity
error fails the run. CMake propagates Jolt's public ABI defines to the
adapter. See the upstream [Jolt documentation](https://jrouwe.github.io/JoltPhysics/).

Eight passes do not mean equivalent solvers. Their contact persistence,
stabilization, collision timing, material rules and internal integration
remain different. Identical materials on both colliders make their
combination yield the common coefficient. The lab reports these profiles
rather than claiming bitwise cross-engine parity.

## Measurements

Step timings surround engine advance only, using a monotonic nanosecond
clock. File I/O, snapshots, metrics and drawing are outside the interval.
The nearest-rank p50/p95/p99 cover all 180 advances, including the first;
there is no hidden warm-up. The Metaverse Engine's tool uses the normal native tool
compiler flags (`-O2`, contraction off, no fast math); Cargo release uses
its release profile and Jolt uses `-O3` with contraction off and its
upstream AVX2 flags. These are measured native release profiles, not a
controlled comparison of identical compiler optimizations.

One Coax observer measures all engines at steps 0, 30, 60, 90, 120, 150 and
180. It computes kinetic plus gravitational potential energy from every
body, with zero potential at world y = 0. Negative slope energy is valid.
Penetration is the maximum plane-corner or SAT box-pair overlap at those
samples, not the maximum over every step or substep. Nonfinite states and
metric capacity exhaustion fail the run. JSON decimals retain six places.

The 64-bit FNV-1a state checksum covers every exact sample step and all
positions, rotations, linear velocities and world angular velocities, in
stable body ID order. Timing bytes are excluded. The second Metaverse Engine run
must match **every sample byte**, not just the digest. Cross-engine digest
differences are expected.

Captures draw the final recorded box poses with one common camera and
scale per scene. Large scenes show 2,000 evenly selected IDs; the legend
states that limit. Metrics and checksums still include every body.

## Results and checks

The [2026-11-06 results](../../docs/verse/physics-lab-2026-11-06.md) include
all eighteen rows, energy samples, checksums, captures, failure receipts
and source/build identities. They show a repeatable Metaverse Engine result alongside
substantial settling, stack and performance gaps.

```sh
coax/bin/coax test --library aethyr/lab/physics-lab.coax
coax/bin/coax mutate --library aethyr/lab/physics-scene.coax --root-only
coax/web/tool.sh coax/lib/sys/selftest.coax one 'two words'
```

The scene authority kills all 116 root mutants without equivalents. The
native system selftest checks nanosecond scale and agreement with the
existing monotonic millisecond clock on macOS and Linux.
