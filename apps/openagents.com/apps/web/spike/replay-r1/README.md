# Replay R-1 — one-frame headless render spike

Date: 2026-06-18 · Issue #5347 (EPIC #5346)

## The question

Can the **existing** proof-replay scene be rendered to pixels in **headless
Chrome** (no interactive UI, no human), driven programmatically? Today there is
no pixel readback outside a live browser; this spike tests the Remotion-pattern
bet (port the _pattern_ — headless Chrome screenshots — not the Remotion
package; see `docs/launch/2026-06-18-remotion-port-audit-for-replay-clips.md`).

## What this spike is

A minimal, self-contained spike (no live Worker / D1):

- `fixture-bundle.ts` — a shipment-gate-passing `proof_replay_bundle.v1` fixture
  (lifted from `packages/proof-replay`'s own test fixture).
- `index.html` + `page-entry.ts` — a minimal page that mounts the **existing**
  scene custom element `oa-tassadar-proof-replay`
  (`apps/web/src/scene/tassadarProofReplayElement.ts`), driven by the
  **existing** `@openagentsinc/proof-replay` primitives
  (`buildReplayRenderPlan`, `cameraPoseFor`). It exposes two programmatic hooks:
  - `window.setReplaySecond(s)` — seeks the scene's real clock to second `s`.
  - `window.setCamera(mode)` — sets the scene's camera mode.
- `render-one-frame.mjs` — a Playwright driver: boots a vite dev server rooted
  at the web app (real package resolution), opens the page in headless
  Chromium, drives the scene to ONE moment + ONE camera pose, and
  `page.screenshot()` -> a single PNG (`frame.png`). It also captures the
  widget-free "social" presentation (`frame-social.png`).
- `render-clip.mjs` — the R-3/R-4/R-5 assembly CLI: drives the same page one
  frame at a time, writes a deterministic `frame_%05d.png` sequence, shells
  ffmpeg to produce an H.264 mp4, and writes a render manifest next to the mp4.
- `camera-path.example.json` — a caller-supplied camera path/keyframe example.
  The current DOM bridge records and mode-selects the camera path for each
  frame; a future true-3D `three-effect` proof-replay scene can consume the same
  manifest poses to re-frame the actual viewpoint.

Reference design only (NOT a dependency): Remotion's `renderStill`
(= `seekToFrame` + `takeFrame`). License forbids forking/depending; we port the
pattern.

## Run it

```sh
cd apps/openagents.com/apps/web
bunx playwright install chromium      # one-time, downloads headless Chromium
node spike/replay-r1/render-one-frame.mjs
```

Outputs `frame.png` and `frame-social.png` in this directory (git-ignored).

Render a short clip from the local fixture:

```sh
cd apps/openagents.com/apps/web
node spike/replay-r1/render-clip.mjs \
  --start 20 \
  --duration 4 \
  --fps 12 \
  --camera spike/replay-r1/camera-path.example.json \
  --out spike/replay-r1/clip.mp4
```

Outputs:

- `clip.frames/frame_00000.png` ... — the R-3 frame sequence.
- `clip.mp4` — the R-4 H.264/yuv420p/faststart mp4.
- `clip.mp4.render.json` — the R-5 manifest with bundle source, frame seconds,
  camera modes, computed `cameraPoseFor` poses, requested camera-path poses,
  codec settings, and run-location boundary.

Render a live public replay bundle by slug:

```sh
node spike/replay-r1/render-clip.mjs \
  --slug first-real-settlement \
  --start 32 \
  --duration 5 \
  --fps 24 \
  --camera zap_focus \
  --out spike/replay-r1/first-real-settlement.mp4
```

Dependencies for clip rendering:

- Playwright Chromium (`bunx playwright install chromium`).
- `ffmpeg` on the render box PATH, or pass `--ffmpeg <path>`.

New dev dependency: **playwright** (added to the repo root `package.json`).
This is a **render-box / CI workload** (headless Chrome), NOT a Cloudflare
Worker workload — the Worker can trigger a render and serve the mp4 from R2, it
must not render.

## RESULT — does a single frame come out as real pixels headlessly?

**YES.** A 1280x720 RGB PNG with real content comes out of headless Chrome,
driven entirely programmatically (no human clicking). The driver seeks to
second 24 (the `payment_zap_confirmed` beat), sets camera `zap_focus`, and
screenshots. `cameraPoseFor` returns a concrete pose for that second.

**BUT — and this is the load-bearing caveat — the frame is a 2.5D DOM/CSS
projection, not a true-3D WebGL render.** The driver inspected the rendered
shadow DOM and found:

- `canvasCount: 0` in the interactive scene — **no WebGL canvas at all**.
- 110 DOM nodes / 7 absolutely-positioned `.plane` projection nodes (stages,
  actors, zap, caption).
- The computed `cameraPoseFor` pose is written to `data-camera-*` attributes
  only; it does **not** move a rendered viewpoint. Selecting `zap_focus` changes
  data, not the picture.

So headless Chrome screenshots the **existing** scene fine — because the scene
is DOM, and Chrome paints DOM. But "camera direction" (the owner's "move the
camera here/there") is not honored by what renders, because there is no real 3D
camera to move. The `?camera=social` capture (`frame-social.png`) is widget-free
and uses a real **2D** `<canvas>` background, but is still a fixed DOM
projection, not a 3D viewpoint.

## Verdict for the EPIC

The headless-Chrome-screenshot pattern (capability 3 of the gap analysis) is
**proven viable** — the make-or-break unknown is resolved: pixels do come out
headlessly, and the `proof-replay` clock + `cameraPoseFor` math drives a chosen
moment + pose programmatically. R-2..R-5 (camera-path input -> frame sequence ->
ffmpeg -> CLI) are mostly assembly on top of this harness.

**However, before R-2..R-5 deliver an owner-directable _3D_ clip, a renderer
decision is required:** the current proof-replay scene
(`tassadarProofReplayElement.ts`) is an explicitly-temporary 2.5D DOM bridge
with no WebGL and no camera-honoring viewpoint. To make camera direction
actually change the rendered picture, the proof-replay scene needs a true-3D
WebGL render path in `@openagentsinc/three-effect` (the renderer of record per
`apps/openagents.com/AGENTS.md`) that consumes `buildReplayRenderPlan` +
`cameraPoseFor` and renders from the camera pose. Other three-effect scenes in
this app already do real WebGL (e.g. `scene/tassadarRunSnapshot.ts`,
`scene/pylonElement.ts`), so the substrate exists; the proof-replay scene simply
has not been ported to it yet.

### Two valid next paths (both work with this exact harness)

1. **Ship clips of the DOM scene as-is.** The headless screenshot loop already
   works on the current scene. `render-clip.mjs` now assembles R-3..R-5 and gets
   real clips — they just won't have a moving 3D camera (the "camera here/there"
   direction is captured in the manifest and limited visually to the existing
   mode-selected fixed projections).
2. **Port the proof-replay scene to a true-3D `three-effect` WebGL mount
   first** (in `three-effect`, per AGENTS.md), then run this same harness. This
   is what makes owner-supplied camera paths actually re-frame the shot. This is
   the renderer decision to make before R-2 if 3D camera direction is required.

The screenshot harness itself does not change between the two — that is the part
this spike de-risked.

## R-3/R-4/R-5 status

- **R-3 frame sequence:** `render-clip.mjs` seeks the proof-replay clock at a
  fixed `fps`, applies the caller camera path/mode for each frame, waits for the
  page to paint, and writes `frame_%05d.png`.
- **R-4 mp4 encode:** the same CLI shells `ffmpeg -framerate <fps> -i
frame_%05d.png -c:v libx264 -pix_fmt yuv420p -movflags +faststart`, with
  configurable CRF and preset.
- **R-5 one entrypoint + run location:** the CLI accepts fixture, local file,
  explicit URL, or public replay slug inputs plus `--camera`, `--start`,
  `--duration`/`--end`, `--fps`, resolution, and `--out`. It runs on
  local/CI/Container render boxes with Bun/Node, headless Chromium, and ffmpeg.
  The Cloudflare Worker should only enqueue/trigger the job and serve/upload the
  resulting mp4 from R2; it must not render frames or run ffmpeg on the edge.
