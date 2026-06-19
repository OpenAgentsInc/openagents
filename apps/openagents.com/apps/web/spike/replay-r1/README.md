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
  The path is resolved per frame and passed through `window.driveReplayFrame`
  as `{position,target,fov}`, so the `three-effect` WebGL camera actually
  reframes the rendered shot.
- `camera-path-alt.example.json` — a second path used by the acceptance proof
  to verify that equal-time frames visibly differ when the caller changes the
  camera.

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

## Render job + R2 upload

`render-job.mjs` is the production render-box wrapper for the typed
`openagents.replay_clip_job.v1` contract. It validates a job, compiles the
camera-path DSL into the existing renderer's camera input, writes the mp4 plus
`openagents.replay_clip_manifest.v1`, and, when `--upload` is present, uploads
both objects to R2 through Cloudflare's S3-compatible API.

```sh
cd apps/openagents.com/apps/web
node spike/replay-r1/render-job.mjs \
  --job spike/replay-r1/job.example.json \
  --out spike/replay-r1/out/clip.mp4
```

Upload mode requires owner-provisioned R2 credentials on the render box:

```sh
export R2_REPLAY_CLIPS_BUCKET=oa-replay-clips
export R2_REPLAY_CLIPS_PUBLIC_HOST=https://clips.openagents.com
export R2_REPLAY_CLIPS_ACCOUNT_ID=<cloudflare-account-id>
export R2_REPLAY_CLIPS_ACCESS_KEY_ID=<r2-access-key-id>
export R2_REPLAY_CLIPS_SECRET_ACCESS_KEY=<r2-secret-access-key>
export R2_REPLAY_CLIPS_PREFIX=replay-clips # optional

node spike/replay-r1/render-job.mjs \
  --job spike/replay-r1/job.example.json \
  --out spike/replay-r1/out/clip.mp4 \
  --upload
```

The job uploads:

- `replay-clips/<jobRef>/clip.mp4`
- `replay-clips/<jobRef>/clip.mp4.clip-manifest.json`

The manifest artifact URL is built from `R2_REPLAY_CLIPS_PUBLIC_HOST` and is
validated as a public HTTPS URL. Missing bucket credentials fail closed with a
typed `needs_owner.replay_clip.r2_bucket_not_provisioned` blocker; the script
does not print secret values.

## Regression renders

`render-regression-smoke.mjs` is the #5434 owned-infra/local regression check.
It renders:

- the curated `first-real-settlement` replay with the primary camera path,
- the same curated replay with `camera-path-alt.example.json`, and
- a generated public activity timeline replay bundle from
  `GET /api/public/proof-replays?mode=activity-timeline`.

The command fails if the first frames are blank, the render manifest does not
record an available `data-proof-replay-webgl` surface, the two curated camera
paths produce the same first-frame sha256, or any generated clip manifest lacks
source refs/caveats/public HTTPS artifact refs.

```sh
cd apps/openagents.com/apps/web
node spike/replay-r1/render-regression-smoke.mjs \
  --out /tmp/openagents-replay-clip-regression \
  --duration 1 \
  --fps 1 \
  --width 640 \
  --height 360
```

Outputs are intentionally inspectable:

- `curated-main.mp4`, `curated-alt-camera.mp4`, and
  `generated-timeline.mp4`
- retained `*.frames/frame_00000.png` screenshots
- `*.render.json` render manifests with WebGL surface probes
- `*.clip-manifest.json` public-safe clip manifests
- `regression-summary.json` with frame hashes and manifest refs

This is a render-box / owned-infra smoke. It does not depend on GitHub-hosted
CI, and it must not run inside the Cloudflare Worker.

New dev dependency: **playwright** (added to the repo root `package.json`).
This is a **render-box / CI workload** (headless Chrome), NOT a Cloudflare
Worker workload — the Worker can trigger a render and serve the mp4 from R2, it
must not render.

## RESULT — does a single frame come out as real WebGL pixels headlessly?

**YES.** A 1280x720 RGB PNG with real content comes out of headless Chrome,
driven entirely programmatically (no human clicking). The driver seeks to
second 24 (the `payment_zap_confirmed` beat), sets camera `zap_focus`, and
screenshots. `cameraPoseFor` returns a concrete pose for that second.

The #5353/#5348 follow-up replaced the old 2.5D DOM/CSS projection with a
`three-effect` WebGL mount. The driver now inspects the rendered shadow DOM and
finds:

- `canvasCount: 1`.
- `context: webgl2` in headless Chromium.
- `projectionNodeCount: 0`.
- `computedPose` matches the caller-supplied camera-path pose when one is
  present.

An example camera-path render was produced at
`spike/replay-r1/example-cinematic-3d.mp4` with matching ignored frame and
manifest outputs beside it.

The camera-path acceptance proof rendered that clip and
`spike/replay-r1/example-alt-camera-3d.mp4` over the same replay window. Matching
frames differed by hash; frame 0 PSNR was `22.945324` dB and frame 24 SSIM was
`0.338971`. Both manifests record requested camera-path poses matching the
computed Three camera pose.

## Verdict for the EPIC

The headless-Chrome-screenshot pattern (capability 3 of the gap analysis) is
**proven viable** — the make-or-break unknown is resolved: pixels do come out
headlessly, and the `proof-replay` clock + `cameraPoseFor` math drives a chosen
moment + pose programmatically. R-2..R-5 (camera-path input -> frame sequence ->
ffmpeg -> CLI) are mostly assembly on top of this harness.

The headless-Chrome-screenshot pattern and the renderer decision are now both
resolved for this track. `render-clip.mjs` produces a real 3D clip, and
camera-path keyframes visibly reframe matching replay moments.

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
