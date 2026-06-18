# @openagentsinc/replay-clips

Schema-only contracts for the **Replay clips production service**
(EPIC [#5411](https://github.com/OpenAgentsInc/openagents/issues/5411)).

This package defines the public-safe programmatic contracts an agent or operator
uses to request a directed replay clip and the manifest the render box produces.
It is **types and validation only** and grants no settlement, payout,
deployment, accepted-work, provider, wallet, or public-claim authority.

Rendering itself runs on owned local/CI/Container infrastructure
([#5431](https://github.com/OpenAgentsInc/openagents/issues/5431)), never inside
the Cloudflare Worker. The Worker may host job records and serve finished refs
only ([#5432](https://github.com/OpenAgentsInc/openagents/issues/5432)).

## What it covers

- **Clip job + manifest** (`clip-job.ts`,
  [#5430](https://github.com/OpenAgentsInc/openagents/issues/5430)): the typed
  request (`ReplayClipJobRequest`), the persisted public read projection
  (`ReplayClipJobRecord`), the output `ReplayClipManifest`, the lifecycle state
  machine, and public-safe validation helpers.
- **Camera-path DSL** (`camera-path.ts`,
  [#5433](https://github.com/OpenAgentsInc/openagents/issues/5433)): a compact,
  bounded JSON grammar (`hold`, `orbit`, `follow`, `frame_actor`,
  `frame_settlement`) that an agent emits to direct the camera, with a
  validator/clamp and a compiler into the existing render-box camera-path input.

## Camera-path DSL

```ts
import {
  makeReplayCameraPath,
  compileReplayCameraPath,
} from "@openagentsinc/replay-clips"

const path = makeReplayCameraPath([
  { second: 0, verb: "hold" },
  { second: 2, verb: "orbit", fov: 55 },
  { second: 4, verb: "frame_settlement" },
])

// Compiles into the `{keyframes:[{second, mode, fov?}]}` shape the render box
// (`apps/openagents.com/apps/web/spike/replay-r1/render-clip.mjs`) consumes.
const compiled = compileReplayCameraPath(path)
```

Verbs map to the proof-replay renderer's existing camera modes:

| Verb               | Render mode      | Requires `actorRef` |
| ------------------ | ---------------- | ------------------- |
| `hold`             | `director_track` | no                  |
| `orbit`            | `orbit_proof`    | no                  |
| `follow`           | `follow_actor`   | yes                 |
| `frame_actor`      | `follow_actor`   | yes                 |
| `frame_settlement` | `zap_focus`      | no                  |

Bounds (fail-closed): at most 32 keyframes, `second` in `[0, 600]`, `fov`
clamped to `[10, 120]`. Unknown verbs, missing/forbidden `actorRef`, and
raw/private material are rejected with a useful error.

## Clip job + manifest

```ts
import {
  assertReplayClipJobRequestSafe,
  assertReplayClipManifestSafe,
} from "@openagentsinc/replay-clips"

const job = assertReplayClipJobRequestSafe(submittedJson) // throws on unsafe input
```

A job sources frames from a published replay bundle (`replay_bundle`) or a
public activity-timeline cursor range (`timeline_range`). The lifecycle is
`queued -> rendering -> succeeded | failed | blocked` (with `blocked -> queued`
re-queue). `blocked`/`failed` records must carry `blockerRefs`; `succeeded`
records must carry a `manifestRef`.

The manifest is source-ref complete: bundle ref, source, camera path, renderer
version, per-artifact sha256, and a public `https` storage URL, so a skeptic can
verify a clip without trusting the renderer.

## Public-safety

All job and manifest refs must be public-safe (slugs, public timeline cursors,
sha256 digests, public storage URLs, ISO timestamps, source refs). Raw traces,
prompts, seeds, provider material, payout targets, invoices, preimages, tokens,
wallet material, mnemonics, local filesystem paths, and customer-private data
are rejected before a record is treated as public-safe.

## Test

```sh
bun run --cwd packages/replay-clips test
bun run --cwd packages/replay-clips typecheck
```
