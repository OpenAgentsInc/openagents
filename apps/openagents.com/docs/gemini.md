Yes. **Image generation now lives inside the same broad Google GenAI/Gemini stack**, but there are **two API shapes** you should distinguish:

1. **Gemini native image generation**: use Gemini image-capable models through `generateContent`, e.g. `gemini-3.1-flash-image`, `gemini-3-pro-image`, and `gemini-2.5-flash-image`. This is best for conversational generation, editing, reference images, multimodal prompts, text + image responses, and iterative workflows. Google documents these as Gemini’s “Nano Banana” image models, with generated images carrying SynthID watermarks. ([Google AI for Developers][1])

2. **Imagen 4 generation**: use specialized Imagen endpoints through `generateImages` in the SDK or `:predict` in REST, with model IDs like `imagen-4.0-generate-001`, `imagen-4.0-ultra-generate-001`, and `imagen-4.0-fast-generate-001`. This is best for fast high-quality text-to-image generation when you do not need the full Gemini multimodal/conversational editing loop. ([Google AI for Developers][2])

For your stack, I would **not start by relying on the Google JS SDK inside Cloudflare Workers**. Google’s official GenAI SDK supports TypeScript and is GA, but Cloudflare Workers is not Node, and edge runtimes are happiest with plain `fetch`. Google’s own docs show REST endpoints for both Gemini image generation and Imagen generation, so I’d implement a thin Effect service over REST first. ([Google AI for Developers][3])

If you already have a Google Cloud/Gemini project, the cleanest path is:

Use your existing Google Cloud project/billing, create or restrict a Gemini API key for server-side use, store it as a Cloudflare secret, and call `generativelanguage.googleapis.com` from your Worker backend. Google notes that Gemini API keys are associated with Google Cloud projects, can be created/imported through AI Studio, and should be restricted; Cloudflare says secrets should be used for sensitive API keys and are exposed to Workers through `env`. ([Google AI for Developers][4])

---

# Markdown spec for coding agent

````md
# Add Gemini / Imagen Image Generation

Status: implementation spec
Target stack: TypeScript backend, Effect, Cloudflare Workers
Primary provider: Google Gemini API / Google GenAI image models
Storage: Cloudflare R2 or existing object store
Default model: `gemini-3.1-flash-image`
Fallback/simple model: `imagen-4.0-generate-001`

## 1. Product behavior

Add backend support for AI image generation.

The backend must expose a typed service that can:

1. Generate an image from a text prompt.
2. Optionally accept reference images for Gemini multimodal image generation.
3. Return normalized image metadata.
4. Store generated image bytes in object storage.
5. Return a stable application URL or object key, not raw base64 by default.
6. Log provider, model, latency, request ID, and safety/error status.
7. Never expose the Google API key to the browser.

## 2. Provider decision

Implement two Google image generation modes.

### Mode A: Gemini native image generation

Use this when the request needs:

- Prompt + reference images.
- Image editing.
- Multi-turn or conversational refinement.
- Text and image interleaved output.
- Better instruction following.
- Character or object consistency.
- Diagrams, infographics, product mockups, or images with text.

Default model:

- `gemini-3.1-flash-image`

Optional higher quality model:

- `gemini-3-pro-image`

API shape:

- `POST https://generativelanguage.googleapis.com/v1/models/{model}:generateContent`
- Header: `x-goog-api-key: <GEMINI_API_KEY>`
- Body uses `contents[].parts[]`
- Generated image bytes appear in response candidate parts as `inlineData.data` base64.

### Mode B: Imagen 4 image generation

Use this when the request is simple text-to-image and needs fast, high-quality generation.

Default model:

- `imagen-4.0-generate-001`

Optional models:

- `imagen-4.0-fast-generate-001`
- `imagen-4.0-ultra-generate-001`

API shape:

- `POST https://generativelanguage.googleapis.com/v1beta/models/{model}:predict`
- Header: `x-goog-api-key: <GEMINI_API_KEY>`
- Body:
  - `instances: [{ prompt }]`
  - `parameters.sampleCount`
  - `parameters.aspectRatio`
  - `parameters.imageSize`

## 3. Cloudflare configuration

Add Cloudflare secrets:

```bash
wrangler secret put GEMINI_API_KEY
````

If using R2, add an R2 bucket binding:

```toml
[[r2_buckets]]
binding = "IMAGE_BUCKET"
bucket_name = "openagents-generated-images"
```

Add env type:

```ts
export interface Env {
  GEMINI_API_KEY: string
  IMAGE_BUCKET: R2Bucket
  PUBLIC_IMAGE_BASE_URL?: string
}
```

Run:

``[118;1:3u`bash
wrangler types
```

## 4. Domain model

Create `src/domain/image-generation.ts`.

```ts
export type ImageProvider = "google-gemini" | "google-imagen"

export type ImageModel =
  | "gemini-3.1-flash-image"
  | "gemini-3-pro-image"
  | "gemini-2.5-flash-image"
  | "imagen-4.0-generate-001"
  | "imagen-4.0-fast-generate-001"
  | "imagen-4.0-ultra-generate-001"

export type ImageAspectRatio =
  | "1:1"
  | "3:4"
  | "4:3"
  | "9:16"
  | "16:9"

export type ImageSize = "512" | "1K" | "2K" | "4K"

export interface ReferenceImageInput {
  mimeType: "image/png" | "image/jpeg" | "image/webp"
  base64: string
}

export interface GenerateImageInput {
  prompt: string
  provider?: ImageProvider
  model?: ImageModel
  aspectRatio?: ImageAspectRatio
  imageSize?: ImageSize
  count?: number
  referenceImages?: ReadonlyArray<ReferenceImageInput>
  userId?: string
  workspaceId?: string
}

export interface GeneratedImage {
  key: string
  url?: string
  mimeType: string
  byteLength: number
  provider: ImageProvider
  model: ImageModel
  prompt: string
  createdAt: string
}

export interface GenerateImageOutput {
  images: ReadonlyArray<GeneratedImage>
  providerRequestId?: string
  rawText?: string
}
```

## 5. Effect service interface

Create `src/services/ImageGenerationService.ts`.

```ts
import { Context, Effect } from "effect"
import type { GenerateImageInput, GenerateImageOutput } from "../domain/image-generation"

export class ImageGenerationError {
  readonly _tag = "ImageGenerationError"
  constructor(
    readonly message: string,
    readonly cause?: unknown,
    readonly status?: number,
  ) {}
}

export class ImageGenerationService extends Context.Tag("ImageGenerationService")<
  ImageGenerationService,
  {
    readonly generate: (
      input: GenerateImageInput,
    ) => Effect.Effect<GenerateImageOutput, ImageGenerationError>
  }
>() {}
```

## 6. Google REST client

Create `src/services/google/GoogleImageGenerationLive.ts`.

Implementation requirements:

* Use `fetch`, not Node-only libraries.
* Read `GEMINI_API_KEY` from Cloudflare `env`.
* Use `Effect.tryPromise`.
* Validate prompt length.
* Clamp `count` to 1–4.
* Default to Gemini if `referenceImages.length > 0`.
* Default to Imagen for simple fast text-to-image only if explicitly selected or configured.
* Decode base64 using Worker-compatible APIs.
* Store images in R2.
* Return image keys and URLs.

### Gemini request

```ts
function buildGeminiBody(input: GenerateImageInput) {
  const parts: Array<any> = [{ text: input.prompt }]

  for (const image of input.referenceImages ?? []) {
    parts.push({
      inlineData: {
        mimeType: image.mimeType,
        data: image.base64,
      },
    })
  }

  return {
    contents: [
      {
        role: "user",
        parts,
      },
    ],
    generationConfig: {
      responseModalities: ["TEXT", "IMAGE"],
      responseFormat: {
        image: {
          aspectRatio: input.aspectRatio ?? "1:1",
          ...(input.imageSize ? { imageSize: input.imageSize } : {}),
        },
      },
    },
  }
}
```

### Gemini fetch

```ts
async function callGeminiGenerateContent(args: {
  apiKey: string
  model: string
  body: unknown
}) {
  const response = await fetch(
    `https://generativelanguage.googleapis.com/v1/models/${args.model}:generateContent`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-goog-api-key": args.apiKey,
      },
      body: JSON.stringify(args.body),
    },
  )

  const json = await response.json()

  if (!response.ok) {
    throw new Error(`Gemini image generation failed: ${response.status} ${JSON.stringify(json)}`)
  }

  return json
}
```

### Imagen request

```ts
function buildImagenBody(input: GenerateImageInput) {
  return {
    instances: [
      {
        prompt: input.prompt,
      },
    ],
    parameters: {
      sampleCount: Math.min(Math.max(input.count ?? 1, 1), 4),
      aspectRatio: input.aspectRatio ?? "1:1",
      ...(input.imageSize ? { imageSize: input.imageSize } : {}),
    },
  }
}
```

### Imagen fetch

```ts
async function callImagenPredict(args: {
  apiKey: string
  model: string
  body: unknown
}) {
  const response = await fetch(
    `https://generativelanguage.googleapis.com/v1beta/models/${args.model}:predict`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-goog-api-key": args.apiKey,
      },
      body: JSON.stringify(args.body),
    },
  )

  const json = await response.json()

  if (!response.ok) {
    throw new Error(`Imagen generation failed: ${response.status} ${JSON.stringify(json)}`)
  }

  return json
}
```

## 7. Response parsing

Implement two parsers.

### Gemini parser

Expected structure:

* `candidates[].content.parts[]`
* Text parts may contain `text`
* Image parts contain `inlineData.data` and `inlineData.mimeType`

Extract all image parts:

```ts
function extractGeminiImages(json: any) {
  const parts = json?.candidates?.flatMap((c: any) => c?.content?.parts ?? []) ?? []

  return parts
    .filter((part: any) => part.inlineData?.data)
    .map((part: any) => ({
      base64: part.inlineData.data,
      mimeType: part.inlineData.mimeType ?? "image/png",
    }))
}
```

Also collect text parts for `rawText`.

### Imagen parser

Google Imagen REST responses usually return base64 image bytes in prediction objects. Support both known field variants defensively:

```ts
function extractImagenImages(json: any) {
  const predictions = json?.predictions ?? []

  return predictions
    .map((p: any) => ({
      base64:
        p.bytesBase64Encoded ??
        p.image?.bytesBase64Encoded ??
        p.imageBytes ??
        p.bytesBase64,
      mimeType: p.mimeType ?? "image/png",
    }))
    .filter((x: any) => x.base64)
}
```

## 8. Base64 to bytes in Cloudflare Workers

Do not use `Buffer` unless the project already enables Node compatibility and tests it.

Use Worker-compatible conversion:

```ts
function base64ToUint8Array(base64: string): Uint8Array {
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)

  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i)
  }

  return bytes
}
```

## 9. R2 storage

Generate stable object keys:

```ts
function imageKey(args: {
  workspaceId?: string
  userId?: string
  model: string
  index: number
  extension: string
}) {
  const now = new Date()
  const date = now.toISOString().slice(0, 10)
  const id = crypto.randomUUID()

  return [
    "generated-images",
    args.workspaceId ?? "global",
    date,
    `${id}-${args.index}-${args.model}.${args.extension}`,
  ].join("/")
}
```

MIME mapping:

```ts
function extensionForMime(mimeType: string) {
  if (mimeType === "image/jpeg") return "jpg"
  if (mimeType === "image/webp") return "webp"
  return "png"
}
```

Store:

```ts
await env.IMAGE_BUCKET.put(key, bytes, {
  httpMetadata: {
    contentType: mimeType,
  },
  customMetadata: {
    provider,
    model,
    userId: input.userId ?? "",
    workspaceId: input.workspaceId ?? "",
  },
})
```

## 10. HTTP route

Add route:

```http
POST /api/images/generate
```

Request:

```json
{
  "prompt": "a clean OpenAgents mission briefing room, dark interface, table-first layout",
  "provider": "google-gemini",
  "model": "gemini-3.1-flash-image",
  "aspectRatio": "16:9",
  "imageSize": "2K",
  "count": 1
}
```

Response:

```json
{
  "images": [
    {
      "key": "generated-images/workspace/date/id-0-gemini-3.1-flash-image.png",
      "url": "https://...",
      "mimeType": "image/png",
      "byteLength": 123456,
      "provider": "google-gemini",
      "model": "gemini-3.1-flash-image",
      "prompt": "...",
      "createdAt": "2026-06-04T00:00:00.000Z"
    }
  ]
}
```

## 11. Validation

Use the project’s existing schema library. If none exists, add a small schema with Effect Schema.

Rules:

* `prompt` required, min 3 chars, max configurable.
* `count` min 1, max 4.
* `aspectRatio` enum.
* `imageSize` enum.
* Reject reference images larger than configured limit.
* Reject unsupported MIME types.
* Require authenticated user or workspace context.
* Rate-limit per user/workspace.

## 12. Safety and policy handling

Provider errors must be normalized.

Create error tags:

* `ProviderRejectedPrompt`
* `ProviderRateLimited`
* `ProviderAuthFailed`
* `ProviderUnavailable`
* `ProviderInvalidRequest`
* `StorageFailed`
* `UnknownImageGenerationError`

Never return raw provider error payloads to the client if they include prompt text, safety annotations, or internals. Log safely.

## 13. Observability

Log structured events:

```ts
{
  event: "image_generation.completed",
  provider,
  model,
  workspaceId,
  userId,
  imageCount,
  latencyMs,
  byteTotal
}
```

Also log failures:

```ts
{
  event: "image_generation.failed",
  provider,
  model,
  workspaceId,
  userId,
  errorTag,
  status,
  latencyMs
}
```

## 14. Tests

Add unit tests for:

* Default model selection.
* Gemini request body.
* Imagen request body.
* Gemini response parsing.
* Imagen response parsing.
* Base64 decoding.
* R2 key generation.
* Count clamping.
* Unsupported MIME rejection.
* Provider error normalization.

Add integration test using mocked `fetch`.

Do not hit the real Google API in CI unless `GEMINI_API_KEY` and `RUN_PROVIDER_TESTS=true` are present.

## 15. Recommended defaults

Use these defaults:

```ts
const DEFAULT_GEMINI_IMAGE_MODEL = "gemini-3.1-flash-image"
const DEFAULT_IMAGEN_MODEL = "imagen-4.0-generate-001"
const DEFAULT_PROVIDER = "google-gemini"
const DEFAULT_ASPECT_RATIO = "1:1"
const DEFAULT_IMAGE_COUNT = 1
```

Routing rule:

```ts
function chooseProvider(input: GenerateImageInput): ImageProvider {
  if (input.provider) return input.provider
  if ((input.referenceImages?.length ?? 0) > 0) return "google-gemini"
  return "google-gemini"
}
```

Keep Gemini as the default because it supports more workflows. Add Imagen as an explicit performance/quality option.

## 16. Acceptance criteria

The feature is complete when:

* `POST /api/images/generate` works from the deployed Cloudflare Worker.
* The Google API key is only stored as a Cloudflare secret.
* Generated images are stored in R2.
* The client receives URLs or object keys, not base64 blobs.
* The service supports Gemini image generation.
* The service supports Imagen text-to-image generation.
* Tests cover request building, parsing, storage, and errors.
* Provider errors are normalized.
* No Node-only API is required in the Worker runtime.

```

## My recommendation

Start with **Gemini native image generation** as the default because it gives you the most product surface: text-to-image, editing, reference images, multimodal flows, and future “agent briefing / after-action report” UI generation. Add **Imagen 4** as a selectable backend for simple “make me four fast high-quality images from this prompt” jobs.

For Cloudflare, the important implementation choice is: **use REST + `fetch` + Cloudflare secrets + R2**, not a Node-assuming SDK path, unless you explicitly test the SDK under your Worker compatibility flags.
```

[1]: https://ai.google.dev/gemini-api/docs/image-generation "Gemini API  |  Google AI for Developers"
[2]: https://ai.google.dev/gemini-api/docs/imagen "Generate images using Imagen  |  Gemini API  |  Google AI for Developers"
[3]: https://ai.google.dev/gemini-api/docs/libraries "Gemini API libraries  |  Google AI for Developers"
[4]: https://ai.google.dev/gemini-api/docs/api-key "Using Gemini API keys  |  Google AI for Developers"
