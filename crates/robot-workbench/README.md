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
