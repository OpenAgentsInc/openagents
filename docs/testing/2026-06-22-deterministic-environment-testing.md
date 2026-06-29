# Deterministic environment testing for `apps/autopilot-desktop` + `three-effect`

Date: 2026-06-22
Scope: `apps/autopilot-desktop` (the Electrobun desktop UI) and the
`@openagentsinc/three-effect` render path it consumes.

## Why this exists: green tests, broken features

Every failure below is real and happened in this codebase. In each case a unit
test was **green** while the actual user-facing behavior was **broken**, because
the test exercised a *fragment* of the path rather than the *whole* path, or
asserted on a *model* instead of an *observable outcome*.

### Mode 1 — reducer-only tests pass, the live key does nothing

The Verse spawn keybinding (⌘⇧E / ⌘⇧P, #6033) had a green reducer test:

```ts
const [next] = update(exploreModel, PressedKey({ key: "e", meta: true, shift: true, ... }))
expect(next.verseSpawnedScenes...).toEqual([CRACKLING])  // GREEN
```

…but the live key did nothing (#6041). The real desktop path is:

```
keydown (DOM)
  → keyboardForwardDecision(...)   ← FORWARD GATE (subscriptions.ts)
    → PressedKey message queued
      → interpretKey(model, event)  ← keyboard.ts
        → reducer (update.ts)
          → resulting Model
            → verseSceneVisualization(model)  ← the render-feeding viz
```

The bug lived entirely in the **forward gate**: the spawn keys were never
registered as input bindings, so `keyboardForwardDecision` resolved them to
**zero** action ids → `forward: false` → the keydown was **dropped before
`interpretKey` ever ran**. The reducer-only test started *after* the gate, so it
could never see the bug. Tests that begin in the middle of a path cannot catch
bugs at the start of it.

### Mode 2 — data-model tests pass, nothing renders

The spawned crackling arc had green tests asserting the beam is "in the model":

```ts
expect((after.beams ?? []).some(b => b.style === "crackling_arc")).toBe(true)  // GREEN
```

…but no test ever rendered the scene and looked at pixels. A beam can be in the
`TrainingRunVisualizationOptions` and still be invisible on screen: the renderer
applies `motionAllowedByPolicy(beam, motionPolicy)` (an `evidence: "required"`
gate), positions are resolved from `entityPositions`, and the animation only
advances on frame ticks. Any of those can silently drop the geometry while the
model-level test stays green. **No model assertion is a render assertion.**

### Mode 3 — `verified: true` on a non-running artifact

A verifier reported `verified: true` for an artifact that did not actually run.
The fix was the headless **acceptance runner**
(`apps/openagents.com/workers/api/src/inference/acceptance-runner/`): it stops
inspecting declared shape and instead **executes + observes**. That is the right
model and this harness generalizes it to the desktop render path: *run the real
thing in a real engine and observe the real output.*

### Constraint A — `bun test` wedges (#5026)

Loading the whole desktop suite into one `bun test` process deadlocks at module
graph load (`__ulock_wait2`). The sanctioned runner is per-file:

```
bash apps/autopilot-desktop/scripts/run-tests.sh
```

which runs each `tests/*.test.ts` in its own `bun test` process. All harness
tests here are per-file safe and pick up automatically.

### Constraint B — the Electrobun native bridge is not faithfully headless

Electrobun's native webview bridge cannot be driven headlessly with fidelity, so
"mount the actual Electrobun app and press a key" is not a reliable CI gate. The
harness works around this by splitting the path at the seam that is already
faithful:

- The **input path** (`keyboardForwardDecision → interpretKey → reducer →
  visualization`) is **pure TypeScript** with no Electrobun dependency. The only
  thing the native bridge adds is the literal `keydown` → forward-gate dispatch,
  and `subscriptions.ts` already factors that decision into a pure
  `keyboardForwardDecision`. So the harness reconstructs the *exact* DOM-handler
  logic (forward gate → queue `PressedKey` → reducer) in-process and asserts the
  whole chain. This is faithful because it calls the **same** `keyboardForward
  Decision` and the **same** reducer the live handler calls.
- The **render path** is driven in **headless Chromium via CDP** against the
  **real** `@openagentsinc/three-effect` custom element (`oa-training-run`), the
  same element the desktop app mounts. WebGL runs under SwiftShader so no GPU is
  required.

## Architecture

### 1. Effect Layer-injected deterministic environment

`src/testing/deterministic-env.ts` provides a small `TestEnvironment` service
plus a `TestEnvironmentLayer` built from Effect's `TestClock` + a deterministic
seeded RNG service. The rule (per `effect-solutions show testing`): **no
`Date.now`, no `Math.random`, no `requestAnimationFrame`/wall-clock in tested
logic** — those come from injected services so a test fully controls them.

- `Clock` → Effect `TestClock` (advance time explicitly; no real sleeps).
- `DeterministicRandom` → a seeded splitmix64 service (`next`, `nextInt`,
  `seedOf`) so "random" is reproducible and `seed` is part of the recorded
  evidence.
- Transports/RPC → stubbed via Layer so a service under test talks to a
  scripted fake, not the network.

These are demonstrated by migrating two seams onto them (see §4) and are reused
by the harnesses for any timing/seed they need.

### 2. Full-input-path harness (`src/testing/full-input-path.ts`)

A single reusable function:

```ts
runKeyEventThroughFullPath(model, keyEvent, { actionMap? })
  → {
      forwarded: boolean,          // keyboardForwardDecision.forward
      preventDefault: boolean,
      dispatched: boolean,         // did a PressedKey actually reach the reducer?
      intent: KeyIntent,           // what interpretKey resolved
      nextModel: Model,            // reducer output
      visualization,               // verseSceneVisualization(nextModel)
      spawnedSceneIds: string[],   // convenience projections
    }
```

It runs the **real** `keyboardForwardDecision`; **only if it forwards** does it
build the `PressedKey` message and run the **real** reducer (`update`), then
project the **real** `verseSceneVisualization`. This is the exact ordering of the
live DOM handler in `subscriptions.ts`, so a feature tested through it is tested
end to end. Crucially, if the forward gate drops the key, `dispatched` is
`false` and `nextModel === model` — which is precisely the #6041 bug, now
observable. The harness accepts an optional `actionMap` so a test can prove the
harness *fails on a deliberately-broken binding map* and *passes on the real one*
(determinism + fail-on-broken proof).

### 3. Headless deterministic pixel verification (`src/testing/headless-pixel.ts`)

Generalizes `scripts/training-scene-canvas-smoke.ts` and the acceptance-runner
"execute + observe" model into a reusable helper:

```ts
renderVisualizationAndProbe({
  entryModulePath,       // a module that mounts oa-training-run with a viz
  frameSteps,            // N FIXED deterministic frame steps
  frameDeltaMs,          // ms advanced per step (fixed, not wall-clock)
  pageQuery?,            // optional query string (e.g. "broken=1") for variants
}) → { canvasWidth, canvasHeight, framesAdvanced, image,
       score(region?) → { brightPixels, distinctLumaBuckets, sampledPixels } }
```

The entry module (`scripts/crackling-arc-entry.ts`) lives under `scripts/`
because it imports the *real* three-effect element and is built standalone by
`bun build`, while `src/` is typechecked against local `.d.ts` shims that do not
declare the element registration functions.

Determinism comes from **replacing the page's `requestAnimationFrame` and
`performance.now` with a driver-controlled fake clock before the element
mounts**. The three-effect element builds its render loop with
`createManagedFrameClock({ mode: "always" })`, which calls the *global* rAF and
`performance.now`; by installing a fake rAF queue + a monotonic fake clock, the
harness advances **exactly `frameSteps` frames of exactly `frameDeltaMs` each**,
with no real animation frames and no wall-clock — identical pixels every run.
This is the "drive the managed frame clock N fixed steps" requirement, achieved
without modifying three-effect (it already reads the injectable globals).

The helper screenshots via CDP `Page.captureScreenshot`, decodes the PNG
in-process (no image deps), and scores bright pixels **inside a region** so a
test can assert e.g. "the crackling arc lights up bright pixels in the upper-mid
band of the frame." A regression test asserts the arc is visible; a
deliberately-broken variant (beam dropped / evidence ref removed so the
`evidence: "required"` gate suppresses it) renders a dark region and the test
**fails** — catching Mode 2.

### 4. Demonstrated service migrations

Two seams consume the deterministic layer to model the pattern (seeded RNG +
TestClock), with tests that run twice and assert identical output.

## How this would have caught the two bugs

- **Keybinding (#6041):** the full-input-path harness runs
  `keyboardForwardDecision` first; on broken bindings `forwarded === false`,
  `dispatched === false`, and the scene never spawns — a hard failure at the
  exact layer the bug lived in, instead of a reducer-only green.
- **No-render:** the headless pixel harness steps deterministic frames and scores
  pixels in the arc's region; an invisible arc (geometry dropped, evidence gate,
  zero frames advanced) yields too-few bright pixels and fails, instead of a
  model-shape green.

### 5. Headless app replica (`src/testing/app-replica.ts`) — the real renderer

The full-input-path harness reconstructs the input chain in pure TS, and the
pixel harness drives a single three-effect element. The **app replica** closes
the last gap: it boots the **whole real renderer** — the SAME `Model` / `view` /
`update` / `subscriptions` + the real Foldkit `Runtime.run` mount the live app
uses — in headless Chromium, and drives + asserts it through real DOM events. It
is now the standing way to verify the real desktop app headless.

```ts
const replica = await launchAppReplica()
await replica.click(".hotbar-slot-2")          // real Foldkit OnClick fires
await replica.pressKey("2")                     // real keydown → forward gate → reducer
await replica.type(".verse-khala-input", "hi") // real keystrokes into the field
const box = await replica.boundingBox(".verse-khala-bar")
const text = await replica.text(".verse-khala-bubble-body")
await replica.stepFrames(n)                      // deterministic frame stepping
await replica.screenshot(path)
await replica.close()
```

Two things historically made "mount the real Electrobun app and press a key"
impossible. Both are solved here:

1. **Component styles.** The old StyleX compile-plugin path was removed by
   #6046. The replica serves the same generated `src/ui/styles.out.css` as the
   packaged app, including the central `--oa-*` token block and shared component
   classes. The real, styled view mounts with no runtime shim and no style
   compiler. (`getComputedStyle(...)` confirms the real CSS is applied, e.g.
   `.app-shell` is `position: fixed`.)
2. **The Electrobun bridge.** `window.bun` / `getRequest()` (which `khalaTurn`,
   `shellTurn`, token resolution, etc. call) is absent in a plain browser. The
   entry installs a **test-controlled stub through the same `setRequest`/
   `pushInbound` seam** the live `main.ts` uses (`bridge.ts`), so the real Effect
   Commands reach a scripted fake, not the network; the live `khalaToken` push is
   driven for streaming. `scriptKhala({ deltas, text, resolveBeforeStream })`
   even reproduces the terminal-answer-first **race** that doubled the Khala
   response. Reuses the #6045 deterministic-env layers for any service timing/seed.

**Determinism.** The page installs a driver-controlled fake `requestAnimationFrame`
+ `performance.now` (the same trick as the pixel harness) BEFORE the entry runs.
Foldkit batches its DOM patch through `requestAnimationFrame`, so the runtime
NEVER repaints on its own — a render happens only when the driver pumps frames.
The same scenario pumps the same frames and yields identical DOM, boxes, and
text every run (`app-replica.test.ts` asserts re-running a scenario is
byte-identical).

**The driver drives the REAL DOM event path** via CDP: `pressKey` dispatches a
real `keydown` (so it flows through the real keyboard subscription → forward gate
→ `interpretKey` → reducer → re-render), and `click` dispatches a real mouse
click at the element's box (so a real Foldkit `OnClick` fires). Nothing calls the
reducer directly.

**It proved-then-fixed three live bugs** (`tests/app-replica.test.ts`), each
fail-before on current `main` / pass-after the fix, asserted on the real DOM:

- **Hotbar 2/3 with the Ask box focused.** Fail-before: a bare `2`/`3` typed a
  digit into the focused `.verse-khala-input` and the hotbar "did nothing".
  Fixed (forward gate + `interpretKey`, scoped to the Ask box via
  `inVerseAskInput`): the wired slots (1/2/3) fire even while the Ask box is
  focused and the digit is swallowed (input stays empty), the slot buttons fire
  on click regardless of focus, and a bare number fires when unfocused — asserted
  by `count('[data-verse-spawned-scene="active"]')` and the input value. Digits
  still type in every other field (composer/terminal/palette) and unwired slots.
- **Khala response renders twice.** Fail-before (terminal-answer-first race):
  `RespondedVerseKhala` set the full answer, then late streamed deltas appended
  it again → the answer appeared twice. Fixed in `update.ts` (`GotVerseKhalaToken`
  now drops deltas once the turn is no longer in flight) — asserted via
  `text(".verse-khala-bubble-body")` (the answer substring occurs exactly once).
- **Ask box behind the hotbar.** Fail-before: the overlay was centered
  (`left: 50%`) at the bottom, overlapping the bottom-left hotbar. Fixed in
  `styles.css` (the overlay is pinned beside the hotbar at the same bottom and
  height) — asserted via `boundingBox()` (the bar and hotbar do not overlap, the
  bar is to the right of the hotbar, same height, bottoms aligned).

Proof screenshots are committed under `docs/testing/proof/2026-06-22-replica-*`.

## Running

- Pure harness tests (input path + deterministic layers): per-file via
  `bash apps/autopilot-desktop/scripts/run-tests.sh`, or individually with
  `bun test apps/autopilot-desktop/tests/full-input-path-harness.test.ts`.
- Headless pixel regression (needs Chrome/Chromium; set `CHROME_PATH` if not at
  the default macOS location): `bun apps/autopilot-desktop/scripts/crackling-arc-pixel-regression.ts`.
  Gated behind a binary check so it skips cleanly where no Chromium is present.
- **App replica (needs Chrome/Chromium + `bun run build:css` first):**
  `bun test apps/autopilot-desktop/tests/app-replica.test.ts`. It skips cleanly
  where no Chromium is present or the compiled stylesheet is missing. Each test
  carries a 60s budget (several CDP round-trips), so it passes under the bare
  `bun test <file>` runner `run-tests.sh` uses without `--timeout`.
