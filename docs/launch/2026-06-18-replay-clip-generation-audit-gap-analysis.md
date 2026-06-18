# Replay Clip Generation: Audit + Gap Analysis

Date: 2026-06-18
Scope: proof-replay primitives, the web replay scene, the desktop replay
surface, and the rendering/capture infrastructure, audited against the owner's
stated replay vision.
Type: audit, recommendation, and implementation status ledger.

## Implementation updates

- 2026-06-18: R-1 (#5347) landed the one-frame Playwright/headless-Chromium
  spike at `apps/openagents.com/apps/web/spike/replay-r1/`. The spike proved
  that the existing proof-replay DOM bridge can be rendered to pixels
  headlessly and that `cameraPoseFor` can be computed for a chosen moment. It
  also confirmed the important caveat: the current proof-replay scene is a 2.5D
  DOM/CSS bridge, not a true 3D `three-effect` camera-honoring renderer.
- 2026-06-18: R-3/R-4/R-5 (#5349/#5350/#5351) now exist as assembly on that
  harness. `render-clip.mjs` drives the replay page frame-by-frame, writes
  `frame_%05d.png`, shells ffmpeg to produce an H.264/yuv420p/faststart mp4,
  and writes a render manifest containing the bundle source, frame seconds,
  camera modes, computed `cameraPoseFor` poses, optional requested camera-path
  poses, codec settings, and the run-location boundary. Inputs may be the local
  fixture, a local bundle JSON file, an explicit bundle URL, or a public replay
  slug such as `first-real-settlement`.
- 2026-06-18: R-1a/R-2 (#5353/#5348) closed the true-3D/directable-camera
  blocker. `@openagentsinc/three-effect` now owns a proof-replay WebGL mount
  (`19013ae`) with real geometry, lighting, labels, source-ref beams, event
  motion, and a perspective camera. The web replay element now adapts
  `ProofReplayBundle` + `ReplayRenderPlan` into that shared renderer instead of
  drawing app-local DOM/CSS stage visuals. The `driveReplayFrame` hook now
  accepts a call-time `cameraPose` override and applies it to the Three camera
  before each headless screenshot. R-1 verification reported `canvasCount: 1`,
  `context: webgl2`, and `projectionNodeCount: 0`.
- Current capability status after this update: moment selection, true WebGL
  headless frames, camera-path input, frames-to-mp4, and one CLI entrypoint are
  implemented for local/CI/Container render boxes. The Cloudflare Worker remains
  only the intended trigger/serve boundary and must not render or run ffmpeg.
  The acceptance proof rendered two equal-time clips with different camera
  paths; matching frames had different hashes, frame 0 PSNR was ~22.95 dB, and
  frame 24 SSIM was ~0.339, proving real camera reframing.

## The vision (the target)

> "I want to be able to see replays of relevant moments... pick a moment, have
> you generate essentially a little video clip of the moment, with me able to
> provide direction — like move the camera here or there. But programmatically.
> I saw a bunch of UI widgets in the autopilot app; I don't want all that. I'm
> not sure any of [the replay stuff] is actually ready."

Decoded, the target is a single programmatic pipeline:

**pick a moment -> render that moment with owner-supplied camera direction ->
emit a short video clip.**

Properties the vision demands:

- Programmatic. Driven by parameters / CLI / API, not by a person clicking.
- Owner-supplied camera direction ("here / there", a camera path).
- Output is a **video clip** (mp4 / gif), not a live interactive page.
- Explicitly NOT a pile of interactive UI widgets.

This document maps what was built the night of 2026-06-17 against that target.

## What was actually built (last night)

The replay work landed 2026-06-17, ~21:17-23:41 CT, across these commits:

- `3a727bf31` Add shared proof replay primitives
- `cf2e1cec9` Add first Tassadar proof replay route
- `f2a7a660d` Add Tassadar replay social share cut
- `7ef66aa34` Add launch recognition payment replay
- `5451a7104` Add proof replay shipment gates
- `452306d1f` Expose proof replays in desktop
- `2846ab522`, `1d663e890`, `f3b33bae8`, `eb20f3b20` fix-ups (desktop scene,
  loading, pause controls / renderer guardrails, live route action)

### 1. `packages/proof-replay` — the replay primitives

File: `packages/proof-replay/src/index.ts` (1013 lines), README at
`packages/proof-replay/README.md`.

This is the strong part of the work, and it is more than "presentation-only 3D
visualization." It is a **deterministic, headless-friendly data + planning
layer**. It is presentation-only in the authority sense (the README is explicit:
"It does not validate proofs, authorize settlement, dispatch payments, read
wallet state, or promote product claims") — the 2026-06-18 Tassadar audit's
"validates/authorizes nothing" verdict is correct on authority. But on
_capability_ it already provides most of the scene-planning math a clip
generator needs:

- A typed bundle schema `proof_replay_bundle.v1` (`ProofReplayBundle`) with
  `actors`, `stages`, `events`, `flows`, `cameraCues`, `captions`, `gaps`,
  `sourceRefs`.
- A deterministic clock: `ReplayPlaybackState`, `ReplayClockCommand`,
  `reduceReplayClock` (play / pause / reset / seek / set_speed / tick), and
  `replayDurationSecond`. Pure reducer; no DOM, no wall clock.
- A render-plan builder `buildReplayRenderPlan(bundle) -> ReplayRenderPlan`
  with `stagePlacements` (deterministic 3D positions), `actorTracks`
  (per-actor keyframes), `paymentVisuals`, `hitTargets`, sorted `cameraCues`,
  sorted `captions`.
- Actor motion interpolation: `interpolateActorPosition(track, second)`
  (lerp between keyframes) — i.e. given any `second`, every actor has a
  deterministic 3D position.
- A camera model: `ReplayCameraMode`
  (`overview | follow_actor | orbit_proof | zap_focus | free_camera |
director_track`), `ReplayCameraCue`, and crucially
  `cameraPoseFor(plan, second, requestedMode?) -> ReplayCameraPose` which
  returns a concrete `{ position, target }` in world space for a given
  second. `cameraCueAt(plan, second)` resolves the active cue.
- A public-safety / shipment gate `assertProofReplayBundleShipmentGate` +
  `assertReplayPlanSourceCoverage` (schema, privacy level, claim scope, source
  refs on everything, unsafe-material regex scan, confirmed-zap evidence,
  blocked-settlement-carries-no-sats, simulation-not-real checks).

Key takeaway: the package can already answer, deterministically and without a
browser, "at second N, where is every actor, where is the camera, what caption
is showing, what events are active." That is the math layer a clip generator
needs. What it cannot do is turn that into pixels — by design and per
`apps/openagents.com/AGENTS.md`, it "owns replay bundle shape, clocks, source
gates, and render plans only; it is not a visual renderer."

### 2. `apps/openagents.com/apps/web/src/scene/tassadarProofReplayElement.ts` — the web scene

File: `apps/openagents.com/apps/web/src/scene/tassadarProofReplayElement.ts`
(1064 lines). Custom element `oa-tassadar-proof-replay`.

What it actually renders:

- It is **not** a real 3D renderer. It is a **2.5D DOM/CSS projection**:
  `projectPoint(position)` maps world `x`/`z` (and a little `y`) to CSS
  `left`/`top` percentages on an absolutely-positioned `.plane` div. Stages,
  actors, zaps, and markers are DOM nodes.
- It consumes `buildReplayRenderPlan`, `interpolateActorPosition`, and
  `cameraPoseFor` from the package — but the camera pose is only written to
  `data-camera-pose` / `data-camera-target` **attributes**. The DOM projection
  is fixed; selecting a camera mode does not actually move a camera, it changes
  a data attribute and which focus the inspector shows. So "camera direction"
  exists as data but does not yet drive any rendered viewpoint.
- It is interactive: a `setInterval(100ms)` playback timer, a play/pause
  button, a `<input type=range>` scrubber, a speed `<select>`, a camera-mode
  `<select>`, a clickable event list, clickable stage/actor buttons, and a
  source inspector. These are exactly the "bunch of UI widgets" the owner
  called out.
- A "social" presentation mode (`?camera=social&hud=social&duration=..&start=..`,
  commit `f2a7a660d`) autoplays a 45-60s cut with a title/HUD/end-card and
  draws a decorative `<canvas>` 2D background (`#drawSocialCanvas`). This is the
  closest thing to a "directed clip" today, but it is still a live
  self-playing DOM page, screen-size dependent, query-param driven, and it does
  not emit a file.

Per `apps/openagents.com/AGENTS.md`, this element is explicitly a "temporary
legacy bridge for the first replay route; do not extend its visual language
except to replace it with `three-effect` mounts." So it is known-temporary.

### 3. The data / "moment" model

Worker routes: `apps/openagents.com/workers/api/src/public-proof-replay-routes.ts`
serve `/api/public/tassadar-replays/first-real-settlement` and
`/api/public/proof-replays?ref=...`.

- `first-real-settlement` is **partly derived from real data**:
  `buildFirstRealSettlementReplayBundle()` pulls the real settlement summary
  (receipt ref, `realBitcoinMoved`, amount sats, state) from the public
  Tassadar summary envelope, but the **event sequence, camera cues, flows, and
  captions are hand-authored** with deterministic refs.
- `launch-recognition-payments`
  (`buildLaunchRecognitionReplayBundle()`) is a **100% hand-authored static
  fixture** keyed to launch docs and recognition refs.

A "moment" is addressable today: each `ReplayEvent` carries `eventRef`
(stable string id), `sequenceIndex` (narrative order), and `timelineSecond`
(float playhead position). `activeReplayEventsAt(bundle, second)` and
`cameraCueAt(plan, second)` resolve the world state at any second. So
"pick a moment" maps cleanly to (bundle + second) or (bundle + eventRef).

Web routes: `apps/openagents.com/apps/web/src/route.ts` (`TassadarReplayRoute`,
`replaySlug`) -> `apps/openagents.com/apps/web/src/page/run.ts` mounts
`tassadarProofReplayView()`.

Desktop: `apps/autopilot-desktop/src/shared/proof-replays.ts` fetches and
validates bundles and computes a summary; `apps/autopilot-desktop/src/ui/view.ts`
embeds the **same** `tassadarProofReplayView()` widget suite into a desktop
pane (commits `452306d1f`, `2846ab522`). This is the "widgets in the autopilot
app" the owner saw.

### 4. Rendering / headless-capture infrastructure

This is the load-bearing gap, so it was checked carefully:

- **WGPUI is gone.** There is no `docs/wgpui/HEADLESS_CAPTURE.md`, no
  `docs/headless-compute.md`, and no `wgpui`/Rust render code in this repo. The
  `openagents/AGENTS.md` references to WGPUI headless screenshot capture
  describe the **old** Rust `openagents` repo; this repo is the reset
  Bun/Effect/TypeScript monorepo and does not carry WGPUI. There is no
  inherited headless screenshot path here.
- **three-effect** (`/Users/christopherdavid/work/three-effect`, the canonical
  3D renderer per AGENTS.md) is **browser-canvas-first**. It can render a scene
  to a `WebGLRenderTarget` FBO (`renderSceneToTarget` in
  `packages/core/src/renderPrimitives.ts`), but there is **no pixel readback**
  (`readPixels` / `toDataURL` / `toBlob`), **no OffscreenCanvas**, **no Node
  headless context**, and **no frame-sequence export**. OffscreenCanvas is only
  listed as a _future suggestion_ in three-effect's own audit
  (`docs/2026-06-14-implementation-audit.md`), not implemented. Every renderer
  is constructed against a DOM canvas.
- **No video encoder anywhere.** Zero references to ffmpeg, `MediaRecorder`,
  `WebCodecs`, `VideoEncoder`, `canvas.captureStream`, mp4/gif/webm across the
  monorepo and three-effect.
- Note: `packages/tassadar-executor/src/replay.ts` / `replay-cli.ts` is
  **trace** replay (numeric re-execution / bitwise verification), unrelated to
  visual/video replay. Do not confuse the two.

## Gap analysis: the five capabilities the vision needs

| #   | Capability                                                                       | Status      | Where it stands                                                                                                                                                                                                                                                                                                                                                                                                         |
| --- | -------------------------------------------------------------------------------- | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Moment selection** (event/timestamp/ref -> scene state)                        | **EXISTS**  | `ProofReplayBundle` + `buildReplayRenderPlan` + `activeReplayEventsAt(bundle, second)` + `interpolateActorPosition`. Moments are addressable by `eventRef` / `sequenceIndex` / `timelineSecond`. Bundles exist for two moments.                                                                                                                                                                                         |
| 2   | **Programmatic camera direction** (API/params, camera path)                      | **EXISTS**  | `render-clip.mjs --camera <path>` resolves ordered keyframes into per-frame `{position,target,fov}` overrides and passes them through `window.driveReplayFrame`. The hook applies those overrides on top of `cameraPoseFor`, and the WebGL camera renders from the resulting pose.                                                                                                                                          |
| 3   | **Headless render -> frames** (no UI, deterministic, image sequence)             | **EXISTS**  | The R-1/R-3 harness runs headless Chromium, mounts the `three-effect` proof-replay canvas, seeks the deterministic replay clock, and writes `frame_%05d.png`. R-1 now reports `canvasCount: 1`, `context: webgl2`, and `projectionNodeCount: 0`.                                                                                                                                                                      |
| 4   | **Frames -> video clip** (ffmpeg/encoder -> mp4/gif)                             | **EXISTS**  | `render-clip.mjs` shells ffmpeg over the PNG sequence and emits H.264/yuv420p/faststart mp4 clips.                                                                                                                                                                                                                                                                                                                       |
| 5   | **One programmatic entrypoint** ("render moment X w/ camera path Y -> clip.mp4") | **EXISTS**  | `render-clip.mjs` accepts fixture, bundle file, explicit URL, or public slug inputs plus camera path/mode, start/duration/end, fps, resolution, ffmpeg, codec settings, and output path. It writes both the mp4 and a render manifest.                                                                                                                                                                                |

Current "is any of it ready" verdict, capability by capability:

- Capability 1 works today and is genuinely reusable.
- Capability 2 is complete for explicit keyframed camera paths: caller-supplied
  camera positions, targets, fov, and camera modes are resolved per frame and
  applied to the rendered Three camera.
- Capabilities 3, 4, 5 are complete for the local/CI/Container render-box path:
  headless Chromium produces WebGL frames, ffmpeg stitches them into mp4, and
  `render-clip.mjs` is the one entrypoint.

## Updated verdict after R-1a/R-2/R-3/R-4/R-5

The clip-generation path is ready as a render-box pipeline: a caller can pick a
replay bundle/time window, pass a camera path, and get an mp4 plus a manifest.
The Worker boundary is unchanged: it should trigger or serve uploaded results,
not render frames or run ffmpeg at the edge.

The original audit below remains as the historical pre-implementation finding.

## Verdict

**No, the clip-generation vision is not ready.** What shipped last night is a
solid deterministic replay **data + planning layer** (`proof-replay`) plus an
**interactive web/desktop viewer** — i.e. exactly the "bunch of UI widgets" the
owner explicitly does not want, sitting on top of a good data model. The
viewer is a temporary 2.5D DOM bridge, not a 3D renderer, and it does not honor
the camera model it computes. Zero of the three capabilities that make this
"generate a video clip programmatically" (headless frame render, frame->video
encode, single programmatic entrypoint) exist.

The biggest gap, and the program risk, is **headless frame rendering
(capability 3)**: with WGPUI removed and three-effect being browser-canvas-only
with no readback, there is currently **no way to turn a scene into pixels
outside a live interactive browser**. Everything downstream (frame sequence ->
video) is blocked on solving that first.

## Recommendation: minimal path to the vision

Reuse the good half; drop the widgets; build the headless render + encode path.

### Reuse (already good)

- `packages/proof-replay` end to end: the bundle schema, the clock reducer,
  `buildReplayRenderPlan`, `interpolateActorPosition`, `cameraPoseFor`,
  `cameraCueAt`, and the shipment gate. This is the moment->sceneState
  foundation; do not rebuild it.
- The bundle data + Worker endpoints as the moment source. "Pick a moment" =
  pick a bundle slug + a `[startSecond, endSecond]` window (or an `eventRef`).
- `three-effect` as the renderer of record (per AGENTS.md), with one addition
  (offscreen render + pixel readback) made **in three-effect first**, not in
  app code.

### Drop (the owner does not want it)

- The interactive control suite in `tassadarProofReplayElement.ts`:
  play/pause, scrubber, speed select, camera-mode select, clickable
  event/stage/actor buttons, inspector. Keep these out of the clip pipeline
  entirely. (They can remain as a separate web exhibit if desired, but they are
  not part of, and must not gate, the clip generator.) The desktop embedding of
  this same widget suite (`apps/autopilot-desktop/src/ui/view.ts`) is likewise
  out of scope for the clip path.

### Build (in sequence)

1. **Programmatic camera-path input (capability 2, finish it).** Extend the
   package with a caller-supplied camera plan: an explicit ordered list of
   camera keyframes `{ second, position, target }` (and/or simple verbs like
   "frame actor X", "orbit stage Y", "hold here") that `cameraPoseFor` can
   honor over (or instead of) the baked cues. This is pure, testable math and
   stays inside `proof-replay`. Output: given `(bundle, second, cameraPlan)`,
   a deterministic `{position, target, fov}`.

2. **Headless render -> single frame (capability 3, the hard unknown — spike
   FIRST).** Before committing to a sequence pipeline, prove that
   `three-effect` can render one frame to an image without an interactive
   browser. Two candidate routes, spike both cheaply:
   - Node + headless GL (`gl` / `headless-gl` or a WebGPU/Node canvas) feeding
     a Three.js `WebGLRenderer`, then `readRenderTargetPixels` -> PNG.
   - Headless browser (Playwright) mounting a minimal three-effect scene,
     stepping the deterministic clock, and `readPixels`/`toBlob` per frame.
     Add the offscreen-render + readback primitive **to three-effect** (its own
     audit already lists OffscreenCanvas as a planned option). This step is the
     make-or-break; sequence work is meaningless until one frame renders
     headlessly. **This is where to spend the first real effort.**

3. **Frame sequence (capability 3, complete it).** Drive the deterministic
   clock from `startSecond` to `endSecond` at a fixed fps (e.g. 30), and for
   each tick: compute scene state from the render plan + camera plan, render one
   headless frame, write `frame_%05d.png`. Determinism is already guaranteed by
   the pure clock + interpolation, so identical inputs must produce identical
   frames (add a golden-frame test).

4. **Frames -> clip (capability 4).** Shell out to `ffmpeg` to encode the PNG
   sequence to a short mp4 (and optionally a gif), with the existing
   `captions` burned in as overlays if wanted. ffmpeg-as-subprocess is the
   lowest-risk encoder; WebCodecs is an alternative only if the render route is
   in-browser.

5. **One entrypoint (capability 5).** A CLI/API:
   `render-replay-clip --slug <moment> --start <s> --end <s>
--camera <path.json> --out clip.mp4`. It composes steps 1-4: load bundle ->
   gate -> build plan -> apply camera plan -> render frame sequence -> ffmpeg
   encode. This is the owner-facing surface; no widgets.

### Sequencing rationale and risk

Do step 2 (the headless-render spike) before anything else downstream — it is
the single biggest unknown and it gates steps 3-5 entirely. If neither headless
route is viable quickly, that is the decision point to escalate, because the
whole vision rests on it. Steps 1, 4, and 5 are comparatively low-risk
(pure math, a well-known encoder, and a thin composition CLI). The data model
(step 0, already done) means the project does not start from zero.

**Recommended first concrete step:** a throwaway spike that renders a _single_
frame of an existing replay bundle's scene state to a PNG with no browser UI,
using three-effect + a headless GL/canvas (or Playwright `readPixels`). If that
PNG comes out, the rest of the pipeline is mostly assembly; if it cannot, the
camera/clip vision needs a renderer decision before any further build.
