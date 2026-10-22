# Haunted House WebGPU shader verification

- **Scope:** #12500, a local two-room preview in [`apps/haunt-preview`](../../../../apps/haunt-preview/README.md), using Effect and direct WebGPU in an OffscreenCanvas worker.
- **Reference setup:** Apple M5 Max, 128 GiB RAM, macOS 26.4. The browser reports an Apple `metal-3` adapter; it does not expose a device description.
- **Results:** [`receipt.json`](receipt.json) records the browser version, render resolution, 30 raw timing samples, player state, resource counters, and the actual Kiln agent's tools. Signing keys and connection tokens are omitted.
- **Images:** [warm lantern](warm.png), [blue shader edit](cool.png), and [agent-produced shader visible](preview.png).

## Verified flow

- Move and turn away from spawn, then save WGSL through Kiln's real WASI `write` tool and OPFS workspace.
- Observe changed pixels in the same WebGPU scene, with the same position, view, and device.
- Reject syntax and binding-layout failures, show source diagnostics, retain the working pipeline, keep walking, and repair each edit.
- Reject an empty tool-produced draft through GPU pipeline validation and repair it.
- Converge after rapid valid/invalid/valid Kiln writes. Unit tests force older valid completions after a newer invalid draft and check frame-boundary adoption and late cleanup.
- Run 30 distinct warm shader edits, retaining one active pipeline, no staged pipeline after completion, two buffers, and one depth texture. Compilation is limited to one job plus the latest queued edit.
- Ask the existing Kiln agent for a tint change. A real free Pylon reads the file and returns a patch; Kiln applies it with its WASI tools and the preview adopts it without moving the player.
- Use no local HTTP mutation or publishing endpoint. The agent uses Kiln's existing encrypted relay/Pylon connection with a fresh browser key.
- Release the demo's buffers, depth texture, pipeline references, and animation loop on teardown.
- Show an unsupported-WebGPU message and disabled save controls when WebGPU is unavailable.
- Finish with no uncaught browser errors.

## Measured results

- **Browser:** Chrome 155.0.8059.85; 834 × 665 render pixels at device scale 1.
- **Warm edits:** 30. Save to first submitted frame: 14.9 ms median, 20.1 ms p95. Save to screenshot-observed pixels: 200 ms median, 214 ms p95.
- **Frame interval:** largest recorded interval across the run: 33.4 ms, including shader compilation and the agent wait.
- **Agent:** 201.7 s total; 201.6 s across model steps and 8.2 ms across its read/patch tools. One malformed reply was rejected before a successful read, patch, and completion. The shader patch then reached a submitted frame in 17.3 ms.
- **Cleanup:** zero owned buffers, depth textures, or active/staged pipelines after disposal. No uncaught errors or local HTTP mutations.

## Timing interpretation

The submitted-frame measurement starts before Kiln saves and ends after the
worker submits the first frame using the accepted pipeline. The screenshot
measurement also includes browser automation and capture overhead; it provides
a conservative bound on observing the changed pixels. Neither measurement is a
GPU presentation timestamp. The receipt records both distributions and the
largest observed frame interval; no universal latency or frame-rate claim follows
from this machine.

Agent response time is separate from shader iteration. The receipt includes
Pylon/model timing and Kiln tool timing when available. Waiting for a model does
not stop the render worker or force the shader through a remote build.

## Reproduce

Follow the [build and browser-check commands](../../../../apps/haunt-preview/README.md#check).
The retained run uses `openagents browser run -- node scripts/verify-browser.mjs`
with `HAUNT_URL` pointing at the local Vite server and `HAUNT_EVIDENCE` under
`openagents scratch`. Native checks: `cargo test -p kiln` (41 passed), formatting
of changed Rust files, and the Kiln Wasm build. Browser package checks:
`npm run check`, `npm test` (3 passed), and `npm run build`.

Device-loss reconstruction, full Haunted House parity, behavior hot reload,
shared projects, and public publishing remain follow-up work. This verification
does not deploy or change the existing public game.
