import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { artifactsBucketForEnv } from './artifacts-binding'
import { decodeUnknownWithSchema } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type ImageGenerationEnv = Readonly<{
  // Optional since #8516 (account-level R2 disabled); resolved through
  // `artifactsBucketForEnv`, which rejects per-call when absent.
  ARTIFACTS?: R2Bucket | undefined
  GEMINI_API_KEY?: string
  appUrl?: string
}>

export type ImageGenerationRuntime = Readonly<{
  fetch: typeof fetch
  makeImageId: () => string
  nowIso: () => string
}>

const workerFetch: typeof fetch = (input, init) => fetch(input, init)

export const systemImageGenerationRuntime: ImageGenerationRuntime = {
  fetch: workerFetch,
  makeImageId: () => compactRandomId('generated_image'),
  nowIso: currentIsoTimestamp,
}

export const ImageProvider = S.Literals(['google-gemini', 'google-imagen'])
export type ImageProvider = typeof ImageProvider.Type

export const GeminiImageModel = S.Literals(['gemini-2.5-flash-image'])
export type GeminiImageModel = typeof GeminiImageModel.Type

export const ImagenImageModel = S.Literals([
  'imagen-4.0-generate-001',
  'imagen-4.0-fast-generate-001',
  'imagen-4.0-ultra-generate-001',
])
export type ImagenImageModel = typeof ImagenImageModel.Type

export const ImageGenerationModel = S.Union([
  GeminiImageModel,
  ImagenImageModel,
])
export type ImageGenerationModel = typeof ImageGenerationModel.Type

export const ImageAspectRatio = S.Literals([
  '1:1',
  '3:4',
  '4:3',
  '9:16',
  '16:9',
])
export type ImageAspectRatio = typeof ImageAspectRatio.Type

export const ImageSize = S.Literals(['512', '1K', '2K', '4K'])
export type ImageSize = typeof ImageSize.Type

export const ReferenceImageInput = S.Struct({
  data: S.String,
  mimeType: S.Literals(['image/png', 'image/jpeg', 'image/webp']),
})
export type ReferenceImageInput = typeof ReferenceImageInput.Type

export const GenerateImageRequest = S.Struct({
  aspectRatio: S.optional(ImageAspectRatio),
  count: S.optional(S.Number),
  imageSize: S.optional(ImageSize),
  model: S.optional(ImageGenerationModel),
  prompt: S.String,
  provider: S.optional(ImageProvider),
  referenceImages: S.optional(S.Array(ReferenceImageInput)),
})
export type GenerateImageRequest = typeof GenerateImageRequest.Type

export const GeneratedImage = S.Struct({
  byteLength: S.Number,
  createdAt: S.String,
  key: S.String,
  mimeType: S.String,
  model: S.String,
  prompt: S.String,
  provider: ImageProvider,
  url: S.String,
})
export type GeneratedImage = typeof GeneratedImage.Type

export const GenerateImageResponse = S.Struct({
  images: S.Array(GeneratedImage),
})
export type GenerateImageResponse = typeof GenerateImageResponse.Type

export class ImageGenerationInvalidRequest extends S.TaggedErrorClass<ImageGenerationInvalidRequest>()(
  'ImageGenerationInvalidRequest',
  {
    reason: S.String,
  },
) {}

export class ProviderRejectedPrompt extends S.TaggedErrorClass<ProviderRejectedPrompt>()(
  'ProviderRejectedPrompt',
  {
    status: S.Number,
  },
) {}

export class ProviderRateLimited extends S.TaggedErrorClass<ProviderRateLimited>()(
  'ProviderRateLimited',
  {
    status: S.Number,
  },
) {}

export class ProviderAuthFailed extends S.TaggedErrorClass<ProviderAuthFailed>()(
  'ProviderAuthFailed',
  {
    status: S.Number,
  },
) {}

export class ProviderUnavailable extends S.TaggedErrorClass<ProviderUnavailable>()(
  'ProviderUnavailable',
  {
    status: S.Number,
  },
) {}

export class ProviderInvalidRequest extends S.TaggedErrorClass<ProviderInvalidRequest>()(
  'ProviderInvalidRequest',
  {
    status: S.Number,
  },
) {}

export class StorageFailed extends S.TaggedErrorClass<StorageFailed>()(
  'StorageFailed',
  {
    error: S.Defect,
  },
) {}

export class UnknownImageGenerationError extends S.TaggedErrorClass<UnknownImageGenerationError>()(
  'UnknownImageGenerationError',
  {
    error: S.Defect,
  },
) {}

export class ProviderNoImageReturned extends S.TaggedErrorClass<ProviderNoImageReturned>()(
  'ProviderNoImageReturned',
  {},
) {}

export const ImageGenerationError = S.Union([
  ImageGenerationInvalidRequest,
  ProviderRejectedPrompt,
  ProviderRateLimited,
  ProviderAuthFailed,
  ProviderUnavailable,
  ProviderInvalidRequest,
  StorageFailed,
  UnknownImageGenerationError,
  ProviderNoImageReturned,
])
export type ImageGenerationError = typeof ImageGenerationError.Type

type NormalizedInput = Readonly<{
  aspectRatio: ImageAspectRatio
  count: number
  imageSize: ImageSize
  model: ImageGenerationModel
  prompt: string
  provider: ImageProvider
  referenceImages: ReadonlyArray<ReferenceImageInput>
}>

type ProviderImage = Readonly<{
  data: string
  mimeType: string
}>

const MAX_PROMPT_CHARS = 4000
const MAX_REFERENCE_IMAGES = 4
const MAX_REFERENCE_IMAGE_BASE64_CHARS = 8 * 1024 * 1024

const isGeminiModel = (
  model: ImageGenerationModel,
): model is GeminiImageModel => model === 'gemini-2.5-flash-image'

const defaultModelForProvider = (
  provider: ImageProvider,
): ImageGenerationModel =>
  provider === 'google-imagen'
    ? 'imagen-4.0-generate-001'
    : 'gemini-2.5-flash-image'

const selectedProvider = (input: GenerateImageRequest): ImageProvider => {
  if ((input.referenceImages?.length ?? 0) > 0) {
    return 'google-gemini'
  }

  return input.provider ?? 'google-gemini'
}

export const normalizeGenerateImageRequest = (
  input: GenerateImageRequest,
): Effect.Effect<NormalizedInput, ImageGenerationInvalidRequest> =>
  Effect.gen(function* () {
    const prompt = input.prompt.trim()

    if (prompt.length < 3) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'prompt_min_length',
      })
    }

    if (prompt.length > MAX_PROMPT_CHARS) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'prompt_max_length',
      })
    }

    const provider = selectedProvider(input)
    const model = input.model ?? defaultModelForProvider(provider)
    const referenceImages = input.referenceImages ?? []

    if (provider === 'google-imagen' && referenceImages.length > 0) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'imagen_reference_images_unsupported',
      })
    }

    if (provider === 'google-imagen' && isGeminiModel(model)) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'imagen_model_required',
      })
    }

    if (provider === 'google-gemini' && !isGeminiModel(model)) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'gemini_model_required',
      })
    }

    if (referenceImages.length > MAX_REFERENCE_IMAGES) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'reference_images_max_count',
      })
    }

    if (
      referenceImages.some(
        image => image.data.length > MAX_REFERENCE_IMAGE_BASE64_CHARS,
      )
    ) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'reference_images_max_bytes',
      })
    }

    const rawCount = input.count ?? 1

    if (!Number.isInteger(rawCount) || rawCount < 1 || rawCount > 4) {
      return yield* new ImageGenerationInvalidRequest({
        reason: 'count_range',
      })
    }

    return {
      aspectRatio: input.aspectRatio ?? '1:1',
      count: rawCount,
      imageSize: input.imageSize ?? '1K',
      model,
      prompt,
      provider,
      referenceImages,
    }
  })

export const buildGeminiGenerateContentBody = (
  input: NormalizedInput,
): Record<string, unknown> => ({
  contents: [
    {
      parts: [
        { text: input.prompt },
        ...input.referenceImages.map(image => ({
          inline_data: {
            data: image.data,
            mime_type: image.mimeType,
          },
        })),
      ],
    },
  ],
  generationConfig: {
    responseModalities: ['Image'],
  },
})

export const buildImagenPredictBody = (
  input: NormalizedInput,
): Record<string, unknown> => ({
  instances: [{ prompt: input.prompt }],
  parameters: {
    aspectRatio: input.aspectRatio,
    imageSize: input.imageSize,
    sampleCount: input.count,
  },
})

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const recordsFromUnknown = (
  value: unknown,
): ReadonlyArray<Record<string, unknown>> =>
  Array.isArray(value) ? value.filter(isRecord) : []

const optionalString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value : undefined

export const extractGeminiImages = (
  value: unknown,
): ReadonlyArray<ProviderImage> => {
  if (!isRecord(value)) {
    return []
  }

  return recordsFromUnknown(value.candidates).flatMap(candidate =>
    recordsFromUnknown(
      isRecord(candidate.content) ? candidate.content.parts : [],
    )
      .map(part =>
        isRecord(part.inlineData) ? part.inlineData : part.inline_data,
      )
      .filter(isRecord)
      .map(inlineData => ({
        data: optionalString(inlineData.data),
        mimeType:
          optionalString(inlineData.mimeType) ??
          optionalString(inlineData.mime_type) ??
          'image/png',
      }))
      .filter((image): image is ProviderImage => image.data !== undefined),
  )
}

export const extractImagenImages = (
  value: unknown,
): ReadonlyArray<ProviderImage> => {
  if (!isRecord(value)) {
    return []
  }

  return recordsFromUnknown(value.predictions)
    .map(prediction => {
      const nestedImage = isRecord(prediction.image) ? prediction.image : {}
      const data =
        optionalString(prediction.bytesBase64Encoded) ??
        optionalString(prediction.bytes_base64_encoded) ??
        optionalString(prediction.imageBytes) ??
        optionalString(nestedImage.imageBytes) ??
        optionalString(nestedImage.bytesBase64Encoded)

      return {
        data,
        mimeType:
          optionalString(prediction.mimeType) ??
          optionalString(nestedImage.mimeType) ??
          'image/png',
      }
    })
    .filter((image): image is ProviderImage => image.data !== undefined)
}

const providerErrorFromStatus = (status: number): ImageGenerationError => {
  if (status === 400) {
    return new ProviderInvalidRequest({ status })
  }

  if (status === 401 || status === 403) {
    return new ProviderAuthFailed({ status })
  }

  if (status === 429) {
    return new ProviderRateLimited({ status })
  }

  if (status >= 500) {
    return new ProviderUnavailable({ status })
  }

  return new ProviderRejectedPrompt({ status })
}

const fetchJson = (
  runtime: ImageGenerationRuntime,
  url: string,
  apiKey: string,
  body: Record<string, unknown>,
): Effect.Effect<unknown, ImageGenerationError> =>
  Effect.tryPromise({
    catch: error => new UnknownImageGenerationError({ error }),
    try: () =>
      runtime.fetch(url, {
        body: JSON.stringify(body),
        headers: {
          'content-type': 'application/json',
          'x-goog-api-key': apiKey,
        },
        method: 'POST',
      }),
  }).pipe(
    Effect.flatMap(response => {
      if (!response.ok) {
        return Effect.fail(providerErrorFromStatus(response.status))
      }

      return Effect.tryPromise({
        catch: error => new UnknownImageGenerationError({ error }),
        try: () => response.json(),
      })
    }),
  )

const callProvider = (
  runtime: ImageGenerationRuntime,
  apiKey: string,
  input: NormalizedInput,
): Effect.Effect<ReadonlyArray<ProviderImage>, ImageGenerationError> => {
  if (input.provider === 'google-imagen') {
    return fetchJson(
      runtime,
      `https://generativelanguage.googleapis.com/v1beta/models/${input.model}:predict`,
      apiKey,
      buildImagenPredictBody(input),
    ).pipe(Effect.map(extractImagenImages))
  }

  return fetchJson(
    runtime,
    `https://generativelanguage.googleapis.com/v1beta/models/${input.model}:generateContent`,
    apiKey,
    buildGeminiGenerateContentBody(input),
  ).pipe(Effect.map(extractGeminiImages))
}

const decodeBase64 = (
  value: string,
): Effect.Effect<Uint8Array, ImageGenerationInvalidRequest> =>
  Effect.try({
    catch: () =>
      new ImageGenerationInvalidRequest({ reason: 'invalid_base64_image' }),
    try: () => {
      const binary = atob(value)
      return Uint8Array.from(binary, character => character.charCodeAt(0))
    },
  })

const extensionForMimeType = (mimeType: string): string =>
  mimeType === 'image/jpeg' ? 'jpg' : mimeType === 'image/webp' ? 'webp' : 'png'

const safeModelSegment = (model: string): string =>
  model.replaceAll(/[^a-zA-Z0-9._-]/g, '-')

const imageUrl = (appUrl: string | undefined, key: string): string =>
  `${appUrl ?? 'https://openagents.com'}/api/images/${encodeURIComponent(key)}`

const storeImage = (
  env: ImageGenerationEnv,
  input: NormalizedInput,
  runtime: ImageGenerationRuntime,
  image: ProviderImage,
  index: number,
): Effect.Effect<GeneratedImage, ImageGenerationError> =>
  Effect.gen(function* () {
    const bytes = yield* decodeBase64(image.data)
    const now = runtime.nowIso()
    const date = now.slice(0, 10)
    const key = [
      'generated-images',
      'users',
      date,
      `${runtime.makeImageId()}-${index}-${safeModelSegment(input.model)}.${extensionForMimeType(image.mimeType)}`,
    ].join('/')

    yield* Effect.tryPromise({
      catch: error => new StorageFailed({ error }),
      try: () =>
        artifactsBucketForEnv(env).put(key, bytes, {
          httpMetadata: {
            contentType: image.mimeType,
          },
          customMetadata: {
            model: input.model,
            provider: input.provider,
          },
        }),
    })

    return {
      byteLength: bytes.byteLength,
      createdAt: now,
      key,
      mimeType: image.mimeType,
      model: input.model,
      prompt: input.prompt,
      provider: input.provider,
      url: imageUrl(env.appUrl, key),
    }
  })

export class ImageGenerationService extends Context.Service<
  ImageGenerationService,
  {
    readonly generate: (
      input: GenerateImageRequest,
    ) => Effect.Effect<GenerateImageResponse, ImageGenerationError>
  }
>()('@openagentsinc/autopilot-omega/ImageGenerationService') {
  static layer = (
    env: ImageGenerationEnv,
    runtime: ImageGenerationRuntime = systemImageGenerationRuntime,
  ) =>
    Layer.succeed(ImageGenerationService, {
      generate: Effect.fn('ImageGenerationService.generate')(function* (input) {
        const apiKey = env.GEMINI_API_KEY

        if (apiKey === undefined || apiKey.trim() === '') {
          return yield* new ProviderAuthFailed({ status: 401 })
        }

        const normalized = yield* normalizeGenerateImageRequest(input)
        const providerImages = yield* callProvider(runtime, apiKey, normalized)

        if (providerImages.length === 0) {
          return yield* new ProviderNoImageReturned()
        }

        const images = yield* Effect.all(
          providerImages
            .slice(0, normalized.count)
            .map((image, index) =>
              storeImage(env, normalized, runtime, image, index),
            ),
        )

        return { images }
      }),
    })
}

export const decodeGenerateImageRequest = (
  value: unknown,
): GenerateImageRequest => decodeUnknownWithSchema(GenerateImageRequest, value)
