# Web build dependency reduction

Issue: [#12183](https://github.com/OpenAgentsInc/openagents/issues/12183).

`openagents-web` now uses `verse-cad-sync` for CAD placement and simulation.
The existing simulation, terrain, water, and force implementations moved
without changing their serialized fields or calculations. Verse re-exports
the shared types at their existing paths. The `/lander3` scene is generated
by `cargo run -p verse --no-default-features --bin lander3-scene` and checked
in, so serving it needs neither Verse nor its animation engine.

## Measurements

Measured on an Apple M5 Max (18 cores, 128 GiB), Rust 1.97.1, eight Cargo
jobs, with incremental compilation and the compiler cache disabled. Both
runs use fresh source-specific compiler state under
`/Users/christopherdavid/work/openagents-target-1-app`. The baseline is
`7194a6242e`. Other chats were building on the machine, so these are single
samples under shared load.

These measurements isolate the CAD extraction before the concurrent
cargo-hakari change (#12185, `2796c12a25`). They are not timings of the
combined build graph after rebasing onto that change.

The cold command is `scripts/dev/build-bench.sh --cold --only build
--variant noincr --no-wrapper --jobs 8 openagents-web`. Only its cold-build
row is compared. The after run uses a scratch copy with separate log names
and exits after that row; no shared artifacts are cleared.

| Measurement | Before | After |
| --- | ---: | ---: |
| Cold `cargo build -p openagents-web`, seconds | 592.2 | 381.6 |
| Cargo-reported build time, seconds | 585.0 | 381.3 |
| Compiled units (zero fresh units in both runs) | 1,073 | 999 |
| Package versions in `cargo tree -p openagents-web` | 851 | 789 |

The measured cold command takes 210.6 seconds less (35.6%).

The Docker native stage was not timed: Docker and Colima are installed,
but neither daemon is running on this machine. Its before/after measurement
is listed in the master delegation's after-close checks.

## Verification

`cargo tree -p openagents-web` contains none of `verse`, `verse-world`,
`verse-zone-everglade`, `verse-content`, `verse-engine`, `verse-pbr`, `wgpu`,
`wgpu-core`, `wgpu-hal`, or `naga`, including development dependencies.
Tests cover the six changed crates and the generated landing scene.
The crate run also found a stale Grid pack; its existing `grid_pack`
example regenerated it. Temporary test files use the task's scratch root:
an extra directory level makes two workshop tests wrap their expected paths.
