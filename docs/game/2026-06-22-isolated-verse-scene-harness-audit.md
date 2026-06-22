# Isolated Verse scene harness — audit & plan

*2026-06-22. How to run individual Verse / `three-effect` scenes (e.g. the Khala
"crackling energy" inference effect) **in isolation** from the command line,
fed by **stubs/synthetic events** instead of the real Region DO / D1 / Worker /
live receipts — so we can develop and eyeball one behavior at a time. Grounds the
GitHub issues opened alongside it.*

## Why

Today the only way to see the M5 crackling-energy / gateway-portal effect is to
run the whole Autopilot desktop app (Electrobun webview) against live or recorded
world data. That couples a one-primitive visual change to the entire stack. We
want **scene slices**: `run a command → a browser window opens showing only that
effect`, driven by a synthetic (simulated) inference event, with the
evidence-bound motion contract intact.

## What exists today (grounded)

**Primitives — `three-effect/packages/core/src/inferenceGatewayPrimitives.ts`:**
- `createCracklingArc(options: CracklingArcOptions): CracklingArcHandle` — a
  branching electric bolt between two points. Options: `from`/`to`, `strandCount`
  (≈4), `segments`, `bend`, `jitter`, `rate`, `opacity`, `color` (default
  `0x93c5fd`), `secondaryColor`, `seed`. Handle: `{ object3D: Three.Group,
  update(dt), setEndpoints(from,to), dispose() }` — **plain Three.js objects, no
  Effect DI**.
- `createGatewayPortal(options): GatewayPortalHandle` — the offworld portal (`update`,
  `setPosition`, `setStatus`, `dispose`).
- `createEvidenceBackedCracklingArc` / `createEvidenceBackedGatewayPortal` — wrap
  the above with the **evidence-bound motion contract**: `sourceRefs`, `motionId`,
  `motionKind`, `generatedAt`, `simulated?: boolean`, `evidenceMode: "optional" |
  "required"`. With `evidenceMode:"required"` a render is **refused** unless
  `sourceRefs.length > 0`; demo scenes pass `simulated:true` + `evidenceMode:"optional"`.

**Scene mount — `three-effect/packages/core/src/trainingRun.ts`:**
`mountTrainingRunVisualization(element, options): Effect.Effect<Handle>` owns the
canvas, the managed frame-clock render loop, and a resource scope; `dispose` is an
Effect. Effect is used for the **mount lifecycle only**, not as a service/Layer DI.

**Examples pattern — `three-effect/examples/*` (training-run, hud-gallery, moksha,
bezier-nodes):** each mounts to a DOM element and runs the mount Effect; built with
`bun build … --target browser` and served via Vite (`dev:demo:*` scripts). This is
the closest thing to a scene runner — but each is hand-rolled and none isolates an
inference primitive.

**Desktop integration (consumers, for reference):**
`apps/autopilot-desktop/src/shared/chat-world-scene.ts` (`activityEventToParticle`,
`projectChatWorldPylonScene`) and `chat-world-visualization.ts` map world events →
beams (`style:"crackling_arc"`, `sourceRefs`) under the evidence contract.

**World data seam for stubbing — `packages/world-client/src/index.ts`:** `WorldClient`
is plain functions returning `Effect` over a lower-level `WorldClientTransport`
(`connect`/`command`/`disconnect`). The `WorldReadModel` (`packages/world-contract`)
carries `events[]` with `WorldInferenceEventPayload` (`requestRef`, `receiptRef`,
`model`, `route`, `workers[]`, `verification`, `settled`, `sourceRefs`). **A stub
`WorldClientTransport` (synthetic deltas) is the clean injection point** for scenes
that want to exercise the real read-model → scene path without a backend.

## Gaps

1. **No standalone scene runner.** No `bun run scene:<name>` that builds + serves +
   opens a single isolated scene. Examples exist but are bespoke.
2. **No headless capture** for `three-effect` scenes (no Playwright/screenshot for CI;
   the wgpui `HEADLESS_CAPTURE` path is a different renderer).
3. **No Effect Layer/`Context.Tag` DI** for the renderer/world in scenes — stubbing is
   done by passing synthetic options or a stub `WorldClientTransport`, not by swapping
   a Layer. (Fine; we standardize on a stub transport + fixture options, and may add a
   thin `Context.Tag` for the world feed if scenes proliferate.)
4. **Inference-event → arc mapper** is partial (`CHAT_WORLD_INFERENCE_NODE_PREFIX`
   exists); a clean `WorldInferenceEventPayload → crackling arc` mapper + unit test
   would let the same fixture drive both the isolated scene and the desktop.

## Plan (→ issues)

- **A — Standalone crackling-energy scene + CLI (first, highest value).** A
  `three-effect/examples/crackling-arc-standalone` page that mounts ONLY
  `createEvidenceBackedCracklingArc` driven by a **synthetic simulated inference
  event**, plus a `bun run scene:crackling` script that builds and serves it and
  opens the browser to the dedicated scene. `evidenceMode:"optional"`,
  `simulated:true`. Knobs (strandCount/color/rate/endpoints) exposed for eyeballing.
  Also render the gateway-portal variant as a second toggle.
- **B — Reusable isolated-scene harness + stub world feed.** Generalize A into a
  small scene-runner (one entry per scene, a shared mount + a `scene:<name>` script
  convention) and a **stub `WorldClientTransport` / synthetic `WorldReadModel`** so
  any scene slice can be fed fixture world events (inference, payment, pylon) with no
  Region DO / D1 / Worker. Lets us isolate any future effect the same way.
- **C — Headless capture for CI.** A Playwright screenshot script
  (`capture-scene-headless`) so scenes can be smoke-rendered + image-compared in CI,
  and so an agent can prove a scene renders without a human opening a window.
- **D — Inference-event → arc mapper + test (M5 richness).** A pure
  `WorldInferenceEventPayload → crackling-arc beam` mapper with a unit test, shared by
  the isolated scene and the desktop, enforcing the evidence contract.

## One line

The primitives + an examples pattern already exist; what's missing is a
`bun run scene:<name>` runner, a stub world feed, and headless capture — start with
the crackling-energy scene so we can see one effect in isolation immediately.
