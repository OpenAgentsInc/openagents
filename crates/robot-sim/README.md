# Robot Lab simulation

`robot-sim` implements the bounded runner and native simulation slice in
[`docs/lab/`](../../docs/lab/README.md). Default builds contain scheduling,
scripted control, private jobs, and recovery. Enable `mujoco` on a native host
to load an operator-selected MuJoCo **3.3.7** library and its SHA-256.
The product runtime uses the C API and does not start Python.

Linux x86-64 is qualified with SO-101 and Franka fixtures. Other hosts receive
an explicit unsupported-platform error until qualified. Phone and browser
builds keep the native feature off.

`Runtime` owns the library; each `Model` owns matching native model/data and a
private copy of its checked inputs. Drops release data before model and remove
temporary files. Inputs have bounded paths, files, XML nesting, meshes, and
simulation memory. Plugins, model generators, flexible bodies, and external
textures are refused. Qualified textures are generated and bounded.

`arm_scene` converts the checked SO-101 URDF, preserves source link frames,
mounts the arm, and adds the scene's primitive colliders. That single backend
owns robot, table, cube, and self contacts. Neighboring links are excluded from
self collisions. The backend uses implicit position servos with declared gain,
damping, effort, and speed. Soft-limit violations remain in saved observations.

`Runner` pins backend/controller builds, timestep, rates, scene, and seed. A
scripted sequence is hashed and immutable. Control and sensor sampling use
step intervals; the controller sees the latest sampled image. State-only
native experiments currently return no camera images. Cameras for learned
control are supplied by a sensor adapter. Action chunks have checked origin,
expiry, finite named positions, and bounds. Waiting for inference leaves the
simulation clock still. Controller implementations must enforce their timeout;
the runner also rejects late returns. Failed inference retains its timing.

`max_steps` includes the initial observation. Interactive runs start paused;
closing their view pauses them. Explicit batch jobs continue. Stop requests
take effect before the next physical step. Reset returns the previous attempt.
`Book` checkpoints each completed step atomically under the project's private
store. Another worker cannot recover a live run. After a crash, recovery saves
the last checkpoint as an interrupted attempt; retry creates a new job.

For a local scripted SO-101 attempt:

```sh
cargo run -p robot-sim --features mujoco --example so101_run -- \
  run STORE PROJECT JOB LIBRARY SHA256
```

The example prints the saved episode reference. Use `stop STORE PROJECT JOB`
from another invocation to request a stop, or `recover STORE PROJECT JOB`
after an interrupted worker. `STORE` is an absolute directory outside Git.
Replay any saved frame with the existing Verse capture example:

```sh
cargo run -p verse --no-default-features --features capture \
  --example robot_lab_capture -- OUTPUT.png --lab-store STORE --project PROJECT \
  --saved-episode DOCUMENT@VERSION --frame 120
```

Tests use private temporary stores. `cargo test -p robot-sim` covers scheduling,
chunks, controls, cancellation, and recovery without native dependencies. For
native qualification, set `ROBOT_LAB_MUJOCO_LIBRARY` and fetch the exact Franka
assets with `python3 tests/fixtures/franka/fetch.py SCRATCH/franka`. Set
`ROBOT_LAB_FRANKA_ROOT` to that folder, then run:

```sh
cargo test -p robot-sim --features mujoco --test native -- --include-ignored
```

Regenerate the bindings with `cargo run --manifest-path tools/bindings/Cargo.toml
-- HEADERS OUTPUT`, optionally passing additional Clang include/sysroot options.
The checked-in bindings were generated with bindgen 0.65.1 from MuJoCo 3.3.7's
C headers, with layout tests and generated comments disabled and only the
model/data types and used C functions selected. The header version is 337;
runtime version checks precede foreign struct access. MuJoCo's Apache license
and notice are included. Franka's unchanged per-model license, XML, and all
asset hashes are in `tests/fixtures/franka/`. The small contact fixture is an
original CC0 fixture. The scripted capture demonstrates motion and contacts;
it does not claim a successful learned grasp.
