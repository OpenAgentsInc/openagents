# Haunted House commands and Jev verification

- **Scope:** the scene-command proof in #12502, following the owner's request to replace the slow coding loop with constrained scene actions. Raw WGSL draft/undo integration remains on that broader issue.
- **Implementation and creator flow:** [`apps/haunt-preview`](../../../../apps/haunt-preview/README.md).
- **Setup:** Apple M5 Max, 128 GiB RAM, macOS 26.4, Chrome 155.0.8059.85, Apple `metal-3` adapter, 834 × 665 render pixels at device scale 1.
- **Ready view:** [a real Jev edit through the UI button](ready.png).
- **Evidence:** [`receipt.json`](receipt.json), [12 real decision examples](decision-cases.json), and [the house still running after the deliberate timeout](preview.png). Browser keys, API credentials, and preview connection tokens are omitted.

## Measured results

| Path | Samples | Result |
| --- | --- | --- |
| Direct scene edit to submitted frame | 30 | 13.6 ms median; 14.0 ms p95 |
| Jev: add a big pumpkin to my left | 1 | 311.5 ms through frame application and Kiln save; 300.5 ms SDK call |
| Jev: move the selected pumpkin to (1, 2) | 1 | 279.4 ms total; 256.9 ms SDK call |
| Jev: make the lantern blue | 1 | 226.8 ms total; 215.6 ms SDK call |
| Additional UI-button smoke after rebasing | 1 | 753 ms total, rounded UI display; no separate SDK timing retained |
| Deliberately stalled browser request | 1 fixture | Stopped after 2,703 ms; the scene kept rendering |
| Shader regression: save to submitted frame | 30 | 13.0 ms median; 21.0 ms p95 |
| Shader regression: save to screenshot-observed pixels | 30 | 201 ms median; 234 ms p95 |

The live model reported `jev-1.14.0`. Each text request contains four Choice
questions in one SDK exchange. These end-to-end calls are individual
observations, not a model latency percentile. Frame timing ends after
`queue.submit`; it is not a hardware presentation timestamp.

The earlier [201.7-second Pylon source-patch exchange](../2026-10-22-haunted-house-webgpu/README.md)
remains historical evidence. It involved several coding-agent turns. Normal scene
edits now apply commands to data or uniforms; the browser did not compile a shader
or recreate the GPU device for any of the 30 scene edits or the real Jev actions.

## Acceptance

- Manual add, move, and resize work through the UI and the shared transaction path.
- Invalid positions fail without changing the accepted scene or history.
- A real Kiln WASI file write to `haunt/scene.json` enters the same command path and can be undone. Manual and model edits preserve stable IDs, pose, view, and device.
- Undo/redo preserve object identity and increase revision numbers. Unit tests check stale/replayed/foreign transactions, schema refusal, atomic grouped edits, history bounds, and preserving custom shaders during the v1 upgrade.
- Real Jev decisions add and move a pumpkin and change the light. Exact coordinates come from text spans; code validates the bounds.
- The 12-example service probe accepted eight explicit supported requests, rejected two unsupported/compound requests, and held two uncertain requests. One negation had the wrong top action but stayed below the confidence floor and could not execute. “Put a big pumpkin” was ambiguous between add and move and was held; “Add a big pumpkin” passed. This small probe is not broad calibration.
- A manual edit cancels a delayed decision. Its eventual response cannot overwrite the newer scene. Controls stay enabled during inference.
- A timeout leaves the scene unchanged and rendering continues. There is no automatic fallback to the coding agent or retry loop.
- Kiln OPFS save/reopen round-trips the project without GPU handles, model data, or player state in the saved representation.
- Foreign origins receive HTTP 403 from the credentialed decision route.
- Bounded GPU readback produces a nonempty PNG; its temporary buffer is released. Teardown leaves zero owned buffers or active pipelines. No uncaught browser errors remain.
- Shader syntax, binding, and empty-draft failures still retain the working shader; movement, repair, rapid saves, and unsupported-WebGPU handling pass.

## Checks

- `cargo test -p kiln`: 44 passed, including a real SDK transport fixture with fixed questions, unused uncertainty, and unknown-action refusal.
- `cargo build -p kiln --bin haunt-director`; Kiln Wasm/WASI build.
- `npm run check`; `npm test`: 9 passed; `npm run build`.
- `HAUNT_URL=... HAUNT_EVIDENCE=... openagents browser run -- npm run verify:browser`: both browser scripts passed, including real Jev requests.
- Both workspace dependency generators, dependency-target normalization, and `scripts/dev/check-workspace-hack.sh` passed after adding the SDK dependency. Stale generated dependency lines were regenerated.

Public hosting, collaboration, device reconstruction, arbitrary behavior edits,
and general free-form authoring remain separate phases. The local native service
holds the TypeSafe credential; the renderer never receives it.

## Multiple pumpkins regression

The exact UI request `add 5 huge pumpkins to my left` now adds five size-2.5
pumpkins in one transaction. The browser check verifies left placement, a real
Kiln save, one-step undo/redo, and unchanged player position, device, and shader.
Counts come from the request text; the action definition accepts a group as one
action. See [the receipt](pumpkin-group-regression.json) and
[the resulting editor](five-huge-pumpkins.png). The full editor check also passed.

## Expanded asset catalog

Six original procedural props join pumpkins: ghosts, gravestones, coffins, candles,
crates, and cauldrons. The shared catalog drives the picker, validation, rendering,
and the Rust service’s fifth Choice question (asset kind).

[The browser receipt](assets-receipt.json) records real UI edits and real Jev
requests for every new kind, a three-ghost group and one-step undo, unsupported
asset rejection, unchanged player/device/shader, and Kiln OPFS save/reopen.
Those six text edits took 151.6–202.2 ms in this run; the group took 152 ms.
The [gallery](assets-gallery.png) comes from a separate isolated warm-light check
that also passed manual editing and save/reopen.

Validation: 44 Kiln library tests, 12 project/protocol tests, TypeScript check,
production build, and all three browser scripts. Existing pumpkin scenes and
legacy command names remain accepted. Startup controls now wait for scene restore.
