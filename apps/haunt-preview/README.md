# Haunted House shader preview

A local, walkable WebGPU scene connected to the real browser Kiln workspace
(#12500). The renderer calls WebGPU directly. Effect manages GPU lifetime,
shader compilation, failures, and workspace-save jobs. The frame loop uses
ordinary TypeScript.

## Run

From the repository root, build the existing Kiln browser module and WASI tools:

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-1
export KILN_OUT="$(openagents scratch)/haunt-kiln-site"
KILN_TRANSCRIPT=0 KILN_DEV=0 scripts/build-kiln.sh
cd apps/haunt-preview
npm ci
npm run dev
```

Open the local URL Vite prints. The port is chosen automatically. Keep
`KILN_OUT` in the environment when running `npm run dev` or `npm run build`.
The prepare script copies generated Kiln files into ignored `public/kiln/`;
generated Wasm and third-party packages are not committed.

Use a current Chrome or Edge with WebGPU and OffscreenCanvas. The page reports
unsupported capabilities and does not substitute WebGL or Three.js.

## Try it

1. Click **Enter the house**, or focus the canvas. Use W/A/S/D to walk,
   drag to look, and Escape to release pointer lock. Arrow keys also work.
2. Change `LANTERN_TINT` near the top of `haunt/material.wgsl` and select
   **Save to Kiln** (Command/Ctrl+Enter). This runs Kiln's WASI `write` tool
   and saves the real workspace to OPFS.
3. Break the shader's syntax or change `@binding(0)` to `@binding(1)`.
   The last working shader continues rendering and the walk continues.
4. Repair the code or select **Original**. The replacement is installed at
   a frame boundary after compilation and pipeline validation.
5. Ask Kiln for a lighting change. Its existing agent loop reads and patches
   the same file through a free Pylon. Model responses can take much longer
   than shader compilation; **Agent activity** shows progress. **Stop** asks
   Kiln to stop after the current step.

The preview worker receives only a bounded shader message with preview and
project identities, a connection token, a request/revision, the fixed asset
path, and a verified SHA-256. It receives no Kiln signing key or account
authority. It keeps accepted runtime state separate from invalid drafts.

## Check

```sh
npm run check
npm test
npm run build
HAUNT_URL=http://127.0.0.1:PORT/ \
  HAUNT_EVIDENCE="$(openagents scratch)/haunt-webgpu-check" \
  openagents browser run -- npm run verify:browser
```

The browser check covers visible pixel changes, real Kiln writes, movement,
syntax and binding failures, repair, overlapping edits, 30 warm edits, a real
Pylon-backed agent patch, teardown, and unsupported WebGPU. `HAUNT_AGENT=0`
skips only the live inference check for offline development; that does not
establish agent acceptance.

The verification records shader-save-to-first-submitted-frame timing and a
conservative save-to-observed-pixels bound using a screenshot. Neither is a
hardware presentation timestamp. Counters describe resources owned by the
demo, not the driver's total allocation.

## Boundaries

- The two-room scene uses original generated geometry and the existing
  Haunted House palette and visual intent. It is not the full game's port.
- This is a local private preview. There is no public publishing action.
- Losing a GPU device stops the preview with a recovery message; device
  reconstruction is the follow-up runtime issue #12503.
- Source errors preserve the active shader. Full game-behavior replacement,
  shared projects, and arbitrary module migration remain follow-up work.
- The browser TypeScript exception is scoped to this application in
  [the agent contract](../../AGENTS.md).
