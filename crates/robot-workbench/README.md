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
