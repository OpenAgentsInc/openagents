# Remotion Port/Adopt Audit for Replay Clip Generation

Date: 2026-06-18
Scope: whether porting/adopting **Remotion** (`remotion-dev/remotion`, v4.0.481,
read at `projects/repos/remotion`) closes the replay clip-generation blocker
identified in `docs/launch/2026-06-18-replay-clip-generation-audit-gap-analysis.md`.
Type: read/audit + recommendation. No feature code was written.

## TL;DR

- **Does Remotion solve the technical blocker?** Yes, in principle. Remotion's
  render pipeline is exactly the missing half: it renders each frame of a React
  composition in **headless Chrome** (so WebGL/canvas/DOM all become pixels with
  no live-browser-and-human required), pipes the screenshots to a bundled
  **ffmpeg**, and exposes a single programmatic entrypoint (`renderMedia()` /
  `renderStill()` / the `remotion render` CLI). That is the "no pixel readback
  outside a browser" problem solved by *using* a headless browser as the
  renderer. Capabilities 3, 4, and 5 of the gap analysis map cleanly onto
  Remotion features that already exist and ship.
- **But the recommendation is NOT to depend on or fork it.** Two hard blockers:
  1. **Licensing.** Remotion is **not** MIT/permissive. It uses a custom
     **Remotion License** with a free tier capped at "individuals and for-profit
     orgs with up to 3 employees," and a paid **Company License** required for
     any larger for-profit org. OpenAgents is a commercial entity that will
     exceed that threshold, so production use requires a paid per-seat license,
     and the renderer **phones home usage telemetry** to `remotion.pro` when a
     license key is set. This is a recurring commercial dependency, not a
     one-time port.
  2. **Stack fit.** Remotion's composition layer is **React + React Three Fiber**
     (`@remotion/three` wraps r3f's `<Canvas>`). Our replay renderer of record
     is **`three-effect`** (imperative Effect/Three.js, not r3f) per
     `apps/openagents.com/AGENTS.md`. Dropping our scene into a Remotion
     composition means either rewriting the scene in r3f or hand-bridging
     three-effect into a Remotion canvas - non-trivial either way.
- **Recommendation: (c) port the *pattern*, not the framework.** The genuinely
  missing piece is small and well-understood: "step a deterministic clock,
  screenshot each frame in a headless browser, pipe PNGs to ffmpeg." Remotion is
  a ~30-package React-video framework wrapped around that ~50-line idea plus a
  commercial license. We should reproduce the pattern (Playwright/headless
  Chrome + our existing `three-effect` scene + our existing `proof-replay`
  deterministic clock + system/bundled ffmpeg) and own it, using Remotion only
  as a **proven reference design**. The license forbids copying/modifying
  Remotion code "for the purpose of selling a derivative," but the *technique*
  (headless-render -> ffmpeg) is generic and not Remotion-proprietary.
- **Recommended first step:** the same one-frame headless spike the gap
  analysis recommended - but informed by Remotion's `renderStill` design
  (`seekToFrame` + `takeFrame`). Reading Remotion makes the spike *lower-risk*
  because it proves the approach works; it does not make us need the package.

## 1. What Remotion actually provides, mapped to our 5-capability gap

The gap analysis listed five capabilities. Status was: 1 EXISTS, 2 PARTIAL,
3/4/5 MISSING. Here is what Remotion provides against each.

| Gap capability | Remotion feature | Where in Remotion | Verdict |
|---|---|---|---|
| 1. Moment selection (event/ts -> scene state) | N/A - Remotion has no notion of "moments"; it expects you to bring your own time->state mapping inside the React component. | - | We already own this (`proof-replay`). Remotion adds nothing here. |
| 2. Programmatic camera direction (camera path input) | No camera-path primitive per se, but `useCurrentFrame()` + props let a composition compute any per-frame value (incl. camera). With `@remotion/three`, `ThreeCanvas` calls r3f `advance()` once per frame, so a camera driven by `useCurrentFrame()` is fully deterministic. | `packages/core` (`useCurrentFrame`), `packages/three/src/ThreeCanvas.tsx` | We still own the camera math (`cameraPoseFor`). Remotion provides the per-frame *drive*, not the camera grammar. |
| 3. **Headless render -> frames** (the blocker) | **`@remotion/renderer`** renders each frame in **headless Chrome** via puppeteer and screenshots it (`takeFrame` -> `screenshot`). Frame N is produced by `seekToFrame` (sets `window.remotion_setFrame(N)`, waits for `remotion_renderReady`, waits on `document.fonts.ready`) then `takeFrame` (puppeteer `page.screenshot`). | `packages/renderer/src/seek-to-frame.ts`, `take-frame.ts`, `puppeteer-screenshot.ts`, `render-frames.ts` | **This is the missing piece, and it exists and ships.** Headless Chrome renders WebGL/canvas -> pixels, no live browser/human. |
| 4. **Frames -> video** (encoder -> mp4) | Frames are piped to **ffmpeg** (`stitch-frames-to-video.ts`, `call-ffmpeg.ts`). ffmpeg ships *inside* Remotion's native Rust **compositor** binary (`@remotion/compositor-darwin-arm64` etc.), so no system ffmpeg is required. Supports mp4/webm/gif/prores, CRF, pixel format, codecs. | `packages/renderer/src/stitch-frames-to-video.ts`, `call-ffmpeg.ts`, `packages/compositor-*` | **Exists and ships.** |
| 5. **One programmatic entrypoint** | `renderMedia({serveUrl, composition, codec, outputLocation, inputProps, ...})` (Node/Bun), `renderStill(...)` for a single frame, and the `remotion render <id> out.mp4` CLI. Also `@remotion/lambda` and `@remotion/cloudrun` for hosted scale-out. | `packages/renderer/src/render-media.ts`, `render-still.ts`, `packages/cli`, `packages/lambda`, `packages/cloudrun` | **Exists and ships.** Runs on Node **or Bun** (renderer package description: "Render Remotion videos using Node.js or Bun"). |

So Remotion supplies exactly capabilities 3, 4, and 5 - the entire missing half.
Capabilities 1 and 2 stay ours regardless (Remotion has no opinion about
"moments" or our camera grammar; it just runs a React function per frame).

### Does headless Chrome actually solve "no pixel readback outside a browser"?

Yes - by sidestepping the premise. The gap analysis's blocker was that
`three-effect` is browser-canvas-only with no `readPixels`/`toDataURL`/Node
context. Remotion does not add Node pixel readback to a WebGL context; instead
it runs the *whole* render inside a real (headless) Chromium and uses
`page.screenshot()` to read pixels. Anything Chrome can paint - WebGL, 2D
canvas, DOM/CSS - becomes a PNG. The deterministic part is that Remotion freezes
the r3f frameloop (`frameloop='never'` while rendering) and advances exactly one
tick per `useCurrentFrame()` change, so frame N is reproducible. This is the
generic, portable insight regardless of whether we adopt the package.

## 2. Fit with our stack (`three-effect` + `proof-replay`)

Our replay scene is **not** r3f. Per `apps/openagents.com/AGENTS.md`, training
and proof-replay visuals must use **`@openagentsinc/three-effect`** (imperative
Effect/Three.js renderer), and `@openagentsinc/proof-replay` owns the bundle
shape, clocks, source gates, and render plans (it "is not a visual renderer").
The current `apps/web/src/scene/tassadarProofReplayElement.ts` is an explicitly
temporary 2.5D DOM bridge.

What we already have that a clip pipeline consumes (verified in this worktree at
`packages/proof-replay/src/index.ts`):

- `buildReplayRenderPlan(bundle)` -> deterministic stage placements + actor
  tracks + camera cues.
- `interpolateActorPosition(track, second)` -> every actor's world position at
  any second.
- `cameraPoseFor(plan, second, requestedMode?)` -> concrete `{position, target}`.
- `ReplayCameraMode` and the clock reducer (pure, no DOM, no wall clock).

Integration shapes, in increasing fidelity to our actual stack:

- **(A) Remotion + `@remotion/three` (r3f).** Would require re-authoring the
  replay scene in React Three Fiber and feeding `cameraPoseFor(plan, frame/fps)`
  off `useCurrentFrame()`. This is the path Remotion is designed for and is the
  least work *if we were an r3f shop* - but we are not. It forks our scene
  language away from `three-effect`, which AGENTS.md forbids extending
  elsewhere. **Rejected on architecture grounds.**
- **(B) Remotion wrapping a plain `<canvas>` that hosts our three-effect scene.**
  Remotion can screenshot any DOM, including a canvas we drive imperatively. We
  could mount a three-effect renderer into a canvas inside a Remotion
  composition and step it from `useCurrentFrame()`. Technically possible, but we
  inherit the React/Remotion runtime, the webpack/esbuild bundle step, and the
  license, *just* to get the screenshot+ffmpeg loop. High overhead for the
  benefit.
- **(C) No Remotion package - our own headless harness around three-effect.**
  Playwright (or puppeteer) opens a minimal page that mounts the **existing**
  three-effect scene, exposes a `window.setReplaySecond(s)` hook that drives our
  `proof-replay` clock + `cameraPoseFor`, and we `page.screenshot()` per tick,
  then shell ffmpeg over the PNGs. This reuses our scene and our math verbatim
  and adds only the harness. **This is the right shape for us.**

The common denominator across B and C is the *exact same pattern Remotion
implements* in `seek-to-frame.ts` + `take-frame.ts` + `stitch-frames-to-video.ts`.
Option C reproduces it without the framework or the license.

## 3. Port vs adopt vs depend

The owner framed it as "fork - by which I mean port to our system." Three
concrete options:

### (a) Depend on Remotion's npm packages directly

- **Pros:** fastest to first mp4; battle-tested; ffmpeg + Chrome provisioning
  handled; Lambda/Cloudrun scale-out available; `renderStill` makes the spike
  near-trivial.
- **Cons:** **commercial license required at our size** (see Section 4); usage
  telemetry to `remotion.pro` when licensed; pulls a large dependency tree
  (own Chrome Headless Shell download + per-platform native compositor binaries
  ~tens of MB); forces React + r3f for the composition layer, which fights our
  `three-effect`/Effect/Foldkit stack and AGENTS.md visual-ownership rule;
  ongoing version-lockstep discipline ("align all `remotion`/`@remotion/*`
  versions").
- **Verdict:** No. The license cost + telemetry + r3f coupling outweigh the
  time saved, for a pipeline whose hard part we can own.

### (b) Fork / vendor Remotion

- **Pros:** control over the code; no version drift surprises.
- **Cons:** **The Remotion License explicitly prohibits this for our purpose.**
  Free-license "Disallowed use cases": *"It is not allowed to copy or modify
  Remotion code for the purpose of selling, renting, licensing, relicensing, or
  sublicensing your own derivative of Remotion."* OpenAgents intends to ship a
  commercial product whose value includes the clip generator; a vendored fork of
  Remotion's renderer baked into that product is squarely the disallowed case
  and is not cured by the Company License (which grants *use*, not the right to
  resell a derivative). Also inherits the whole 30-package React-video framework
  we don't want. **Legally and architecturally wrong.** Reject.

### (c) Port just the pattern (headless-Chrome-frames -> ffmpeg)

- **Pros:** the actually-missing logic is small; reuses our `three-effect` scene
  and `proof-replay` math unchanged; no React/r3f coupling; **no Remotion
  license obligation** (the headless-render->ffmpeg *technique* is generic prior
  art, not Remotion IP - Remotion itself is "puppeteer screenshot + ffmpeg");
  no telemetry; fits Bun/Effect; we own and can evolve it.
- **Cons:** we provision Chrome (Playwright handles this) and ffmpeg ourselves;
  we write the determinism/seek harness (Remotion's `seek-to-frame.ts` is a good
  reference for the wait-for-ready + fonts-ready handshake); we don't get
  Lambda/Cloudrun scale-out for free (not needed for launch-scale clips).
- **Verdict:** **Recommended.** Use Remotion's source as the reference design
  for the harness, implement against our own stack, take on no license.

## 4. LICENSING - read this before any "depend" or "fork" decision

**Remotion is NOT MIT.** From `projects/repos/remotion/LICENSE.md` (the "Remotion
License", (c) 2026 Remotion; `package.json` files declare `"license": "SEE LICENSE
IN LICENSE.md"`):

- **Two-tier model.** Free License vs Company License.
- **Free License eligibility:** an individual; **a for-profit organization with
  up to 3 employees**; a non-profit/not-for-profit; or someone *evaluating* and
  not yet using it commercially.
- **Company License required** for any for-profit org **above the small-team
  threshold.** Pricing/purchase at `remotion.pro/license` (per-seat developer
  license model). The Company License grants *use* for the allowed use cases and
  prioritized support.
- **Disallowed even under the Free License:** *"It is not allowed to copy or
  modify Remotion code for the purpose of selling, renting, licensing,
  relicensing, or sublicensing your own derivative of Remotion."*
- **Upcoming change:** the file notes the license "will slightly change" in
  Remotion 5.0 (PR #3750). Treat the exact terms as version-dependent and
  re-verify before any commercial commitment.
- **Telemetry / metering:** the renderer registers usage events with
  `remotion.pro` when a `licenseKey` is set
  (`@remotion/licensing` -> `internalRegisterUsageEvent`, POST to
  `https://www.remotion.pro`, classified `billable | development | failed`;
  `packages/renderer/src/render-media.ts` calls this on each render). In v5,
  `ENABLE_V5_BREAKING_CHANGES` makes `licenseKey` a required field. So licensed
  use is **metered and phones home**.

**Implication for OpenAgents-scale commercial use:** OpenAgents is a for-profit
organization that will exceed the 3-employee free threshold and intends to ship
this capability inside a commercial product. That means:

- **Depend (a):** would require purchasing per-seat Company Licenses *and*
  accepting metered usage telemetry to a third party for a core product
  pipeline. Recurring cost + external dependency on `remotion.pro` availability
  + data egress about our render volume.
- **Fork/vendor (b):** the disallowed-use clause prohibits a resold derivative;
  this is the worst option legally.
- **Port the pattern (c):** carries **no Remotion license obligation** because
  we copy *no* Remotion code - the headless-Chrome-screenshot + ffmpeg technique
  is generic and independently implementable (Playwright + ffmpeg are
  permissively/separately licensed). This is the only option that keeps a core
  launch capability free of a third-party commercial license and telemetry.

This licensing reality is the single biggest reason the recommendation is (c),
not (a) or (b), even though the technical fit of (a) is good.

## 5. Bun / Effect / Cloudflare-Worker reality - where clips render

Remotion's renderer needs **Node or Bun + headless Chrome + ffmpeg**. It is
**not** a Cloudflare Worker workload, and neither is our equivalent:

- Headless Chrome (Playwright/puppeteer) and ffmpeg are heavyweight native
  processes that cannot run inside a CF Worker (no full browser/process spawn,
  no large native binaries, CPU/time limits). Remotion provisions its own
  Chrome Headless Shell download and ships ffmpeg inside per-platform native
  Rust compositor binaries - both incompatible with the edge worker model.
- Our deploy model (per `apps/openagents.com/AGENTS.md` /
  `docs/2026-06-15-openagents-web-deploy-runbook.md`) is a CF Worker + static
  assets, plus Containers for orchestration. **Clip rendering belongs in a
  local/CI render service or a Container/VM render box**, *not* the edge Worker.
  The Worker can *trigger* a render job and serve the resulting mp4 from R2; it
  must not try to render.
- Concretely: the clip generator is a Bun/Node CLI or service (Playwright +
  ffmpeg) that runs on a developer machine, CI runner, or a Container, writes
  `clip.mp4`, and uploads to R2/object storage for the Worker to serve. This is
  true whether we use Remotion or our own harness - Remotion's `@remotion/lambda`
  / `@remotion/cloudrun` exist precisely because the same constraint applies to
  them.

## 6. Recommendation + minimal path

**Recommendation: option (c) - build our own headless-render -> ffmpeg harness,
using Remotion's renderer as the reference design. Do not depend on or fork
Remotion.** The technical blocker (capability 3) is real and Remotion proves the
solution shape, but the license cost, telemetry, and r3f/React coupling make
adopting the package the wrong call for a core, commercial, `three-effect`-based
pipeline whose hard part is small enough to own.

Shortest route from today's gap to a working "moment + camera-path -> mp4":

0. **(already done)** Moment -> scene state + camera math: `proof-replay`'s
   `buildReplayRenderPlan` / `interpolateActorPosition` / `cameraPoseFor` /
   clock reducer. Reuse verbatim.

1. **One-frame headless spike (do FIRST - the make-or-break, now de-risked).**
   A throwaway Bun script: Playwright launches headless Chromium, opens a
   minimal page that mounts the **existing** `three-effect` scene built from a
   replay bundle, exposes `window.setReplaySecond(s)` that drives the
   `proof-replay` clock + applies `cameraPoseFor(plan, s)` to the camera, then
   `page.screenshot({path: 'frame.png'})`. If that PNG comes out correct, the
   rest is assembly. Reading Remotion's `render-still.ts` (`seekToFrame` +
   `takeFrame`) and `seek-to-frame.ts` (the wait-for-`renderReady` +
   `document.fonts.ready` handshake) makes this spike *low-risk* - it is a
   proven pattern, just reimplemented on our stack. **This is the same first
   step the gap analysis recommended; Remotion makes it trivial to design, not
   trivial to skip.**

2. **Camera-path input (finish capability 2).** Extend `proof-replay` (pure,
   testable) to accept a caller-supplied ordered camera plan
   `{ second, position, target, fov }[]` (and/or verbs like "frame actor X",
   "orbit stage Y", "hold") that `cameraPoseFor` honors over baked cues. Stays
   inside the package; no renderer involved.

3. **Frame sequence (complete capability 3).** Loop the deterministic clock from
   `startSecond` to `endSecond` at fixed fps (e.g. 30), call
   `setReplaySecond` + `screenshot` per tick -> `frame_%05d.png`. Determinism is
   guaranteed by the pure clock + interpolation; add a golden-frame test (same
   discipline Remotion relies on).

4. **Frames -> clip (capability 4).** Shell out to **ffmpeg** to encode the PNG
   sequence to mp4 (and optionally gif), burning `captions` as overlays if
   wanted. Use a vendored/pinned ffmpeg binary or a system dependency on the
   render box (not the Worker). ffmpeg-as-subprocess is the lowest-risk encoder
   and is exactly what Remotion does internally.

5. **One entrypoint (capability 5).** A Bun CLI / service:
   `render-replay-clip --slug <moment> --start <s> --end <s>
   --camera <path.json> --out clip.mp4` that composes 0-4: load bundle -> run
   `assertProofReplayBundleShipmentGate` -> build plan -> apply camera plan ->
   render frame sequence headlessly -> ffmpeg encode. No widgets. Runs in
   local/CI/Container, uploads the mp4 to R2; the Worker serves it.

### What to build vs reuse

- **Reuse unchanged:** `proof-replay` (bundle schema, clock, render plan, actor
  interpolation, `cameraPoseFor`, shipment gate); the `three-effect` scene as
  the renderer of record (any missing 3D primitive goes into `three-effect`
  first, per AGENTS.md); the bundle Worker endpoints as the moment source.
- **Build (new, owned):** the headless harness (Playwright page + `setReplaySecond`
  hook + screenshot loop), the camera-path extension to `proof-replay`, the
  ffmpeg encode step, and the CLI/service entrypoint. Use Remotion's
  `render-still.ts` / `seek-to-frame.ts` / `stitch-frames-to-video.ts` as the
  reference for the seek handshake and frame-ordering discipline only.
- **Drop / keep out of the clip path:** the interactive widget suite in
  `tassadarProofReplayElement.ts` and its desktop embedding (play/pause,
  scrubber, speed/camera selects, clickable lists, inspector). These are the
  "bunch of UI widgets" the owner does not want in the clip generator.

## Verdict

**Remotion does solve the technical blocker** - headless Chrome + ffmpeg is a
proven way to turn a scene into a video clip with no live browser and no human,
and `renderStill`/`renderMedia` are the single-entrypoint shape we want. **But
we should port the pattern, not the package**, because (1) the Remotion License
requires a paid Company License at our size and forbids reselling a derivative,
and the licensed path is metered/phones-home; and (2) Remotion's composition
layer is React/r3f, which fights our `three-effect`/Effect/Foldkit stack and
AGENTS.md visual-ownership rules. The missing logic is small enough to own:
Playwright + our existing `three-effect` scene + `proof-replay`'s deterministic
clock and camera math + ffmpeg, fronted by one CLI, running on a local/CI/Container
render box (never the CF Worker). **First step: the one-frame headless spike** -
which reading Remotion has now de-risked into a known-good design.
