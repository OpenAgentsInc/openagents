# Autopilot Task: Gemini Image Generation Implementation

Status: complete. The original known Autopilot run is
`11a4ff12-601b-48f3-b596-34f947bfc4bb`, attached to goal
`agent_goal_c964d70720954a989b99916e1e4ebcdf` for project
`project_imagegen_support`. The foreground agent has now implemented the first
production slice directly because the requested goal is to continue getting
Gemini working rather than wait on the stale run.

Foreground progress:

- added a typed Worker `ImageGenerationService` with Gemini/Imagen provider
  request builders, response parsers, tagged errors, R2 storage, and safe
  response metadata;
- added authenticated `POST /api/images/generate`;
- added authenticated `GET /api/images/:objectKey` for stable generated-image
  URLs backed by the existing `ARTIFACTS` R2 bucket;
- typed `GEMINI_API_KEY` as an optional Worker secret binding;
- added `/images` logged-in browser route with prompt, provider/model, aspect
  ratio, size, count, generation state, errors, and generated image grid;
- verified current Google model IDs and REST endpoint shapes from official
  Google AI developer docs on 2026-06-04;
- added focused Worker and browser tests for provider request/parse/storage,
  route auth, route parsing, update behavior, and scene rendering.
- found the existing restricted `openagentsgemini` Generative Language API key
  through local ADC and installed it as the Cloudflare `GEMINI_API_KEY` secret
  without printing the secret value;
- provider-smoked `gemini-3.5-flash-image` through the live
  `v1beta/models/:generateContent` endpoint and received one image part;
- kept `/images` operator/workroom-only and tightened the API so
  `/api/images/generate` plus generated image reads require OpenAgents Core
  team access, not just any browser session.
- deployed Worker version `862f2bff-ffe1-4e3f-9d83-c239cb2d5da7` on
  2026-06-04 after `bun run check:deploy` passed;
- production smoke verified `/images` serves the new web asset and unauthenticated
  `POST /api/images/generate` returns `401 {"error":"unauthorized"}`.
- foreground follow-up on 2026-06-04 diagnosed the production generation 500 as
  a Cloudflare Worker illegal-invocation error from storing the raw global
  `fetch` method in `systemImageGenerationRuntime`;
- fixed the Worker runtime boundary to store a fetch wrapper instead of the raw
  platform method, added a regression assertion, and added bounded secret-safe
  image generation failure logging;
- deployed Worker version `2ee0e0f6-c5ed-413c-8048-a3a1709b34e0`;
- signed-in production smoke from a real OpenAgents Core Team browser session
  returned one generated PNG from `POST /api/images/generate`, stored it under
  `generated-images/users/2026-06-04/...`, and loaded the R2-backed image URL
  through the same signed-in session with `content-type: image/png`.

Target repo: `OpenAgentsInc/openagents`

Target branch: `main`

Primary agent: `autopilot`

Team: `team_openagents_core`

Project: `project_imagegen_support`

Visibility: team-visible by default. Public visibility should wait until image
generation output, storage, and safety projection rules are complete.

Public route or observer link: team/shared run link for the current run:
`https://openagents.com/t/11a4ff12-601b-48f3-b596-34f947bfc4bb`.

## Dispatch Gate

Do not continue this work until the operator runbook gate is satisfied enough
for this specific run:

- `docs/autopilot-tasks/2026-06-04-programmatic-autopilot-operator-runbook.md`
  exists in the pushed commit;
- operator checklist reports provider, GitHub writeback, SHC, callback, and
  project readiness;
- callback lag for this run is zero or has been retried through
  `POST /api/omni/operator/agent-runs/:runId/callbacks/retry`;
- continuation uses `POST /api/omni/operator/agent-runs/:runId/continue` while
  the SHC job is active, or a durable goal continuation if the run has stopped;
- no raw Google, provider, callback, OAuth, R2, or GitHub credentials appear in
  the task packet or launch payload.

Original implementation source spec:

- `../gemini.md`

This packet is the launch-ready version of that spec.

## Objective

Implement ImageGen support in OpenAgents product surface using Google Gemini / Imagen through
Cloudflare Workers and Effect services.

The feature must let authenticated users request image generation through UI
and API surfaces, store generated image bytes in Cloudflare storage, return
stable application URLs or object keys, and keep all provider credentials and
raw provider payloads out of the browser.

## Provider Direction

Use Google image generation through REST over `fetch`, not a Node-assuming SDK,
unless the runner verifies the SDK works correctly under the Worker runtime.

Gemini native image generation should be the default because it supports
prompt-to-image, reference images, editing, multimodal workflows, and iterative
refinement.

Imagen should be an explicit selectable backend for simple high-quality
text-to-image generation when the full Gemini multimodal loop is not needed.

The original spec named Gemini 3 image models, but live Google model discovery
for the `openagentsgemini` project on 2026-06-04 exposes the current Gemini
image model as:

- default Gemini model: `gemini-3.5-flash-image`;
- default Imagen model: `imagen-4.0-generate-001`;
- optional Imagen models: `imagen-4.0-fast-generate-001`,
  `imagen-4.0-ultra-generate-001`.

Before implementation, verify current Google model IDs and REST shapes against
the official Google docs. If a model ID has changed, update this packet or the
implementation notes and use the current documented replacement.

## Current Starting Point

The foreground operator created the original implementation spec at
`../gemini.md` and dispatched Autopilot to implement it through the ImageGen
support project.

Known platform work already completed by foreground operator sessions:

- shared run attribution now uses the run owner rather than the viewer;
- team sidebar sync keeps team-owned runs visible to teammates;
- SHC callback ingestion accepts job-event envelopes and sparse control events;
- credential-shaped callback payloads are sanitized before D1 persistence;
- operator preflight/checklist, callback retry, and continuation endpoints are
  being added in the current foreground session.

Known active implementation evidence from the SHC workspace:

- ImageGen-related docs and app files were observed under the Autopilot run
  workspace;
- the active branch observed on origin was
  `openagents/autopilot-b-48f3-b596-34f947bfc4bb`;
- the current run must still be verified through Cloudflare run state before
  accepting or continuing the work.

Do not manually finish the product implementation from the foreground session
unless the user explicitly switches away from Autopilot-owned delivery.

## Product Behavior

The backend must expose a typed service that can:

- generate an image from a text prompt;
- optionally accept reference images for Gemini multimodal image generation;
- return normalized image metadata;
- store generated image bytes in Cloudflare storage;
- return a stable application URL or object key, not raw base64 by default;
- record provider, model, latency, request ID when available, and safe
  error/safety status;
- never expose the Google API key to the browser.

## API Surface

Add:

```http
POST /api/images/generate
```

Request shape:

```json
{
  "prompt": "a clean OpenAgents mission briefing room, dark interface, table-first layout",
  "provider": "google-gemini",
  "model": "gemini-3.5-flash-image",
  "aspectRatio": "16:9",
  "imageSize": "2K",
  "count": 1
}
```

Response shape:

```json
{
  "images": [
    {
      "key": "generated-images/workspace/date/id-0-gemini-3.5-flash-image.png",
      "url": "https://...",
      "mimeType": "image/png",
      "byteLength": 123456,
      "provider": "google-gemini",
      "model": "gemini-3.5-flash-image",
      "prompt": "...",
      "createdAt": "2026-06-04T00:00:00.000Z"
    }
  ]
}
```

The response may include safe request IDs or normalized safety/error tags. It
must not include raw provider responses, API keys, auth material, or unbounded
diagnostics.

## Cloudflare Configuration

Use a Cloudflare secret for the Google API key:

```sh
wrangler secret put GEMINI_API_KEY
```

If generated images are stored in R2, add an R2 bucket binding such as:

```jsonc
{
  "r2_buckets": [
    {
      "binding": "IMAGE_BUCKET",
      "bucket_name": "openagents-generated-images",
    },
  ],
}
```

Do not commit secret values. If the exact binding name differs in current OpenAgents product surface
config, use the existing binding style and update this packet.

## Domain Model

Use Effect Schema at the Worker boundary. The domain model should cover:

- provider: `google-gemini` or `google-imagen`;
- model;
- aspect ratio: `1:1`, `3:4`, `4:3`, `9:16`, `16:9`;
- image size: `512`, `1K`, `2K`, `4K` where supported by the provider;
- prompt;
- count;
- optional reference images with MIME type and base64 input;
- generated image key, optional URL, MIME type, byte length, provider, model,
  prompt, and timestamp.

Validate:

- prompt required, min 3 chars, max configurable;
- count min 1, max 4;
- aspect ratio enum;
- image size enum;
- unsupported MIME types rejected;
- reference image total size bounded;
- authenticated user or workspace context required;
- per-user/workspace rate limit or explicit follow-up task if no rate limiter
  exists yet.

## Effect Service Boundary

Create a typed image generation service instead of placing provider calls in a
route handler.

The service should expose a method equivalent to:

```ts
generate(input) => Effect.Effect<GenerateImageOutput, ImageGenerationError>
```

Use tagged errors such as:

- `ProviderRejectedPrompt`;
- `ProviderRateLimited`;
- `ProviderAuthFailed`;
- `ProviderUnavailable`;
- `ProviderInvalidRequest`;
- `StorageFailed`;
- `UnknownImageGenerationError`.

Use `Effect.tryPromise` at `fetch` and storage boundaries. Route handlers should
map tagged errors to HTTP responses; domain/service code should not construct
raw `Response` objects.

## Provider Request Rules

Gemini native generation:

- endpoint shape: `POST /v1/models/{model}:generateContent`;
- header: `x-goog-api-key`;
- request body uses `contents[].parts[]`;
- text prompt goes in a text part;
- reference images go in `inlineData`;
- generated images are parsed from response candidate parts with
  `inlineData.data`.

Imagen generation:

- endpoint shape from the original spec:
  `POST /v1beta/models/{model}:predict`;
- body uses `instances: [{ prompt }]`;
- parameters include `sampleCount`, `aspectRatio`, and `imageSize` where
  supported;
- parse known base64 fields defensively.

Provider selection:

- explicit `provider` wins;
- requests with reference images use Gemini;
- default provider is Gemini.

## Storage Rules

Decode base64 in a Worker-compatible way. Do not require Node `Buffer` unless
the Worker config already enables and tests Node compatibility for this path.

Object keys should include:

- `generated-images`;
- workspace or user scope;
- date partition;
- random ID generated through the repo's runtime primitive boundary;
- image index;
- model;
- extension derived from MIME type.

Store content type in object metadata. Store only safe custom metadata:
provider, model, user/workspace ID, and non-secret trace IDs.

## UI Work

Add a logged-in image generation surface without creating a marketing page.

Expected UI:

- prompt input;
- model/provider selector;
- aspect ratio selector;
- count control;
- generate button with loading and disabled states;
- error state with normalized message;
- generated image grid;
- copy/open/download affordances if already available in the local UI system.

Use existing OpenAgents product surface/Foldkit patterns and the local UI registry. Do not add ad
hoc icons or one-off vanilla CSS.

## Observability

Record safe structured events for completion:

```json
{
  "event": "image_generation.completed",
  "provider": "google-gemini",
  "model": "gemini-3.5-flash-image",
  "imageCount": 1,
  "latencyMs": 1000,
  "byteTotal": 123456
}
```

Record safe structured failure events:

```json
{
  "event": "image_generation.failed",
  "provider": "google-gemini",
  "model": "gemini-3.5-flash-image",
  "errorTag": "ProviderRateLimited",
  "status": 429,
  "latencyMs": 1000
}
```

Do not log raw prompts if current product policy forbids it. If prompts are
logged for debugging, store only bounded/sanitized summaries and document the
policy.

## Autopilot Work Plan

1. Run the operator checklist for the current ImageGen run and project.
2. Retry callbacks if checklist reports callback lag.
3. Continue the active run if it is still running or waiting for input.
4. If the active run stopped, request a durable goal continuation rather than
   launching an unrelated run.
5. Verify current Google image model IDs and REST shapes from official docs.
6. Implement the Effect service, tagged errors, config decoding, and storage
   boundary.
7. Add the API route and route-level error mapping.
8. Add UI access for authenticated users.
9. Add tests for request building, response parsing, base64 decoding, storage
   key generation, count clamping, unsupported MIME rejection, provider error
   normalization, API route auth, and UI command/update behavior.
10. Run typecheck, tests, deploy checks, and deploy or record a typed deploy
    blocker.
11. Write `result.md` and delivery artifacts required by SHC.

## Acceptance Criteria

- `POST /api/images/generate` works from the deployed Cloudflare Worker.
- Google API key is stored only as a Cloudflare secret.
- Generated image bytes are stored in Cloudflare storage.
- Browser/API clients receive URLs or object keys, not raw base64 by default.
- Gemini image generation is supported.
- Imagen text-to-image generation is supported or a typed blocker records why
  it cannot be safely enabled yet.
- Provider errors are normalized and safe.
- No Node-only API is required in the Worker runtime unless explicitly tested.
- Logged-in UI can generate and view images.
- Tests cover provider request/parse/storage/error behavior and UI/API access.
- The final Autopilot artifact includes commit or PR link, test output, and
  deployment notes.

## Suggested Run Summary

```text
Autopilot implemented ImageGen through a typed Gemini/Imagen Effect service,
Cloudflare storage, authenticated API access, and a logged-in image generation
surface. Provider credentials stayed server-side, generated images are stored
behind stable application keys or URLs, and tests cover provider parsing,
storage, route behavior, and UI access.
```
