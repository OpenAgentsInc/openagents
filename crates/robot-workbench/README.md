# Robot Lab workbench

This crate provides versioned robot tools, portable viewing state, accessible
Rust Native controls, and a local HTTP player. `robot-web` mounts the player with
Rust/WASM and Verse. Opening or replaying a card never starts a controller.

Build the browser files with the pinned Rust toolchain and the `wasm-bindgen-cli`
version in `Cargo.lock`:

```sh
scripts/build-robot-web.sh /absolute/path/to/player
cargo run -p robot-workbench -- serve \
  --root /absolute/path/outside/git/robot-files \
  --project EXISTING_PROJECT --build /absolute/path/to/player
```

The owner's local arm demo (SO-101 on a table with a cube) runs from a copied
binary, player, and catalog, with its lab folder outside every Git checkout:

```sh
~/work/film-out/robot-lab-bin/robot-workbench serve --root ~/robot-lab \
  --project arm-demo --build ~/work/film-out/robot-lab-player \
  --catalog ~/work/film-out/robot-lab-catalog --port 8795 \
  --execution ~/work/film-out/mujoco/execution.json
open "http://127.0.0.1:8795/play?document=so101-table&version=81d51e5dfbd5c23a0d1d90f44b3aaef1493a7215f0849b60b87e27976126a249"
```

To create that scene in a new project, call `robot_import` once:
`curl -X POST -H 'content-type: application/json' -d
'{"document":"so101-table","source":"so101","revision":"REVISION"}'
http://127.0.0.1:PORT/api/tool/robot_import`, with `REVISION` from
`robot-description/assets/so101/source.json`. It returns the player link.

A CAD Lab version imports the same way once its URDF is exported
(`openagents cad export rover --formats urdf`): call `robot_import` with
`"source":"cad:rover/v38"` and the `revision` the export printed (the
SHA-256 of the version's document, in its `source.json`). The service reads
CAD Lab's folder (`$CAD_LAB_DIR`, else `~/work/film-out/lab/cad`, or
`serve --cad DIR`) and stands the robot on a floor with each link's CAD
mass and inertia and its joints; it can be inspected and posed, not
simulated (the URDF has no contact shapes).

In a scene, the arm panel has one slider per joint within the description's
limits, Open gripper, Close gripper, and Reset pose. Keys: Q/A shoulder pan,
W/S shoulder lift, E/D elbow flex, R/F wrist flex, T/G wrist roll, O/C gripper,
0 reset. Moves change the unsaved draft and draw immediately; Save pose keeps
them as a new version.

With `--execution`, the configuration's pinned MuJoCo library also enables the
panel's Simulate toggle. Its `{"library":{"path":…,"sha256":…}}` names the
official 3.3.7 build; on this Mac that is `libmujoco.3.3.7.dylib` from
`mujoco-3.3.7-macos-universal2.dmg`, with hashes in
`~/work/film-out/mujoco/pin.json`. Simulate starts unsaved physics from the
saved scene through `POST /api/sim`: `{"start":{"document":…,"version":…}}`,
then `record`'s commands (`pose` with joint targets and steps, `step`,
`finish`), at most 240 steps per call. The player sends the current targets
with 8 steps (1/30 s) about every 33 ms, and draws the arm and cube from the
returned step. Reset pose restarts the physics. Nothing is saved; use `record`
for an episode.

The SO-101 jaws collide through capsule pads on their inner faces (the jaw
meshes' hulls would fill the gap between them), with a gentle, slow grip; see
`robot-sim`'s README. To pick up the cube, turn on Simulate, press Reset pose,
and press each key the given number of times, about ten a second. Wait where
shown.

| Step | Keys |
| --- | --- |
| Raise and open | S ×3, O ×20 |
| Turn the jaws to the cube's faces | T ×23 |
| Point the jaws down | R ×27 |
| Reach out | E ×2 |
| Swing over the cube, then wait ½ s | A ×8 |
| Lower around the cube, then wait ½ s | W ×8 |
| Close, then wait 2½ s while the jaws close slowly | C ×24 |
| Lift (about 6 cm) | S ×8 |
| Set it down, then open and wait 2½ s | W ×8, O ×20 |
| Raise clear | S ×8 |

`robot-sim`'s native test `so101_jaw_pads_pick_up_the_table_cube_with_key_presses`
replays this sequence and checks that the cube rises at least 5 cm and stays
held for 2 s.

Keep `CARGO_TARGET_DIR` outside the checkout. The service listens on loopback;
`--port 0` chooses an available port. Its SO-101 catalog defaults to the licensed,
pinned files in `robot-description/assets/so101`. Imports copy checked files into
the private project store. The service does not execute repository setup scripts.

Set `ROBOT_LAB_URL` to this service's URL for Coder's typed `robot_*` tools. For
web chat, set `OPENAGENTS_ROBOT_LAB` to the URL and
`OPENAGENTS_ROBOT_LAB_OWNER` to the owner's account identifier. Web chat offers
the tools only to that owner in the matching project. Its authenticated routes
proxy the player and files; cards cannot supply their own service URL.

Tools create immutable scene versions, change bounded joint poses and object
placements, compare scenes or saved episodes, inspect recorded steps, and attach
notes to an exact version and frame. Every edit names its base version. Chat
selection saves the version, frame, camera, and comparison with a conditional
write so a late callback cannot replace a newer selection.

The browser shows one selected 3D card at a time. Hidden cards release their
renderer and pending requests and retain a poster. Recorded cameras use saved
image references; a missing demonstration frame stays missing. Simulated camera
samples remain visible until the next recorded sample. Comparisons share an
orbit camera and have independent timelines. Terminal tools return a poster and
local player link. Native semantic controls are available to platform adapters.

The browser uses Verse's colored-triangle profile with vertex lighting, including
WebGL2 when WebGPU is unavailable. Saved native posters use the PBR renderer.
Both use the same checked description, link transforms, placements, and saved
states. Simulation and policy execution remain in `robot-sim` and its adapters.

Run `cargo test -p robot-workbench` for the local tool and lifecycle checks.
Build `robot-web` for `wasm32-unknown-unknown` to check the browser adapter.

Record manual simulation commands with an explicitly pinned native MuJoCo
library. The scene must already be saved in this project:

```sh
robot-workbench record --root /private/robot-files --project PROJECT \
  --scene DOCUMENT@VERSION --job recording-one \
  --library /absolute/path/libmujoco.so.3.3.7 --library-sha SHA256 \
  --export /private/new-dataset
```

Send one JSON command per line on standard input:

```json
{"command":"pose","joints":{"shoulder_pan":0.2},"steps":8}
{"command":"step","steps":24}
{"command":"finish"}
```

Each command advances at most 240 physics steps; joint targets remain within
the description's limits. Simulation otherwise stays paused. The recorder saves
each completed step and samples the scene's cameras at 30 Hz, using their focal
lengths, principal points, and optional link mounts. Its camera renderer uses
the shared colored-triangle profile. It never reads the inspection orbit.
Closing standard input before `finish` saves an interrupted recording. A process
crash leaves the last saved step available through `robot-sim`'s job recovery.

Exports use LeRobot v2.1 Parquet tables with embedded PNG images, feature
definitions, episode statistics, explicit joint units, source references, and
the source robot's license and notices. Every row pairs an observed state and
fresh image with the command applied on the following physical step. The final
state without a next command is excluded. Missing required images prevent
export; stopped and interrupted recordings keep their outcome in `export.json`.
Camera PNGs are saved as three-channel RGB for the reference loader. You can
also export an existing episode with `robot-workbench export --root FOLDER
--project PROJECT --episode DOCUMENT@VERSION --output NEW_FOLDER`.

Run `robot-data/tools/check-export.py` with LeRobot 0.3.3 to check an export's
images, states, and statistics against the Python reference. The tool requires
explicit scratch and report paths and runs offline.

Publication is a separate command. Set `HF_TOKEN`, review the export's manifest
hash and the destination's current commit, and run:

```sh
robot-workbench publish --dataset /private/new-dataset --repo OWNER/NAME \
  --parent CURRENT_HUB_COMMIT --manifest-sha REVIEWED_EXPORT_SHA256
```

The Hub repository must already exist. Public destinations also require
`--allow-public`. The uploader freezes checked files, preserves license notices,
and uses a commit bound to the reviewed parent. It supports regular, basic LFS,
and multipart LFS uploads. Credentials go to the Hub API and its own verification
endpoint; storage uploads use the supplied signed addresses.

Execute a saved scripted or ACT experiment with `robot-workbench execute`.
Choose `--experiment DOCUMENT@VERSION`, `--job NAME`, and `--seed N`, plus
the same private store and MuJoCo library options as the recorder. ACT adds
`--policy /private/checkpoint --policy-sha SHA256`. Saved scripted sequences
reopen by their build hash. Jobs checkpoint physical steps and save stopped
or interrupted attempts for replay.

The `gym_robot_study` example prepares separate seed sets and runs ten paired
validation cases. It pins each cube placement before executing either
controller. Its arguments are `STORE PROJECT SCENE@VERSION STUDY LIBRARY
LIBRARY_SHA POLICY_FOLDER POLICY_SHA NEW_OUTPUT_FOLDER`. Results remain
private until an explicit Gym publication command.

Pass `--execution /private/runner.json` to `serve` to enable the experiment
and run tools. The file is a `experiments::Config` JSON value with a pinned
MuJoCo `library` and optional pinned `policy`. `robot_experiment` saves the
settings; `robot_run` starts them explicitly; `robot_run_status` reads progress;
`robot_run_control` pauses, resumes, steps, resets, or stops. At most two runs
execute here. Interactive attempts start paused and save partial work after
60 seconds without a control command. Batch runs continue when a view closes.
Run `robot-workbench recover --root FOLDER --project PROJECT --job NAME` after
a process interruption to save its last completed step without restarting it.

Policy configuration `id` selects `act`, `smolvla`, or `service`; the execute
command exposes the same choice as `--policy-kind`. The study example accepts
this optional final argument. Service experiments save their pinned manifest,
including checkpoint hashes, rather than copying remote weights to the client.

## Phone and paired-host access

The OpenAgents phone app opens **Robot Lab** from an online computer's menu.
The Rust-owned card shows checked saved-frame previews, joint positions and
limits, frame playback, pose changes, saved experiments, and run controls.
Scenes and runs are paged; images and views use checked 16 KiB chunks. Playback
shows successive preview frames as they arrive, rather than promising real-time
rendering. The browser and desktop retain their interactive 3D views.

Start this workbench with an explicit private project and execution configuration
as above. Create a small host configuration outside Git:

```json
{"url":"http://127.0.0.1:8795","project":"arm-demo"}
```

Start the paired host with `coder host serve --robot-lab-config /absolute/path/to/lab.json`
(or set `OPENAGENTS_ROBOT_LAB_CONFIG` to that path). The host checks the workbench's
project before every forwarded call, refuses redirects and remote URLs, and
rechecks the device's current grant on every request. Observe permits reads;
operate permits pose changes and explicit execution of saved pinned experiments.
No host configuration means no remote lab access.

**Run batch** starts one declared seed and continues when the card closes.
**Open run** follows an existing job after reconnect or app restart. **Stop**
saves partial work; it does not discard earlier states or images. Hidden cards
stop playback and progress polling. The phone downloads no model checkpoint
and opens no simulator.
