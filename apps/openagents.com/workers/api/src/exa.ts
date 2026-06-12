import { redactProviderAccountSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Redacted, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  type ExaConfig,
  type ExaSearchType,
  OpenAgentsWorkerConfig,
  type OpenAgentsWorkerConfigEnv,
  type WorkerSecret,
  redactedValue,
} from './config'

type ExaEndpoint = '/contents' | '/search'
type ExaFetchResponse = globalThis.Response
type ExaFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<ExaFetchResponse>

const EXA_ERROR_SUMMARY_LIMIT = 240

export const ExaSearchTypeSchema = S.Literals([
  'auto',
  'fast',
  'instant',
  'deep-lite',
  'deep',
  'deep-reasoning',
])

export const ExaSearchCategory = S.Literals([
  'company',
  'github',
  'linkedin profile',
  'news',
  'pdf',
  'people',
  'personal site',
  'research paper',
  'tweet',
])
export type ExaSearchCategory = typeof ExaSearchCategory.Type

export const ExaHighlightsOptions = S.Struct({
  maxCharacters: S.optionalKey(S.Number),
  numSentences: S.optionalKey(S.Number),
})
export type ExaHighlightsOptions = typeof ExaHighlightsOptions.Type

export const ExaTextOptions = S.Struct({
  maxCharacters: S.optionalKey(S.Number),
})
export type ExaTextOptions = typeof ExaTextOptions.Type

export const ExaSummaryOptions = S.Struct({
  query: S.optionalKey(S.String),
})
export type ExaSummaryOptions = typeof ExaSummaryOptions.Type

export const ExaContentsOptions = S.Struct({
  highlights: S.optionalKey(S.Union([S.Boolean, ExaHighlightsOptions])),
  maxAgeHours: S.optionalKey(S.Number),
  summary: S.optionalKey(S.Union([S.Boolean, ExaSummaryOptions])),
  text: S.optionalKey(S.Union([S.Boolean, ExaTextOptions])),
})
export type ExaContentsOptions = typeof ExaContentsOptions.Type

export const ExaSearchInput = S.Struct({
  category: S.optionalKey(ExaSearchCategory),
  contents: S.optionalKey(ExaContentsOptions),
  excludeDomains: S.optionalKey(S.Array(S.String)),
  includeDomains: S.optionalKey(S.Array(S.String)),
  numResults: S.optionalKey(S.Number),
  query: S.String,
  type: S.optionalKey(ExaSearchTypeSchema),
})
export type ExaSearchInput = typeof ExaSearchInput.Type

export const ExaContentsInput = S.Struct({
  contents: ExaContentsOptions,
  ids: S.optionalKey(S.Array(S.String)),
  urls: S.optionalKey(S.Array(S.String)),
})
export type ExaContentsInput = typeof ExaContentsInput.Type

export const ExaResultContents = S.Struct({
  highlights: S.optionalKey(S.Array(S.String)),
  summary: S.optionalKey(S.String),
  text: S.optionalKey(S.String),
})
export type ExaResultContents = typeof ExaResultContents.Type

export const ExaSearchResult = S.Struct({
  author: S.optionalKey(S.String),
  contents: S.optionalKey(ExaResultContents),
  domain: S.optionalKey(S.String),
  entities: S.optionalKey(S.Unknown),
  favicon: S.optionalKey(S.String),
  highlightScores: S.optionalKey(S.Array(S.Number)),
  highlights: S.optionalKey(S.Array(S.String)),
  id: S.optionalKey(S.String),
  image: S.optionalKey(S.String),
  publishedDate: S.optionalKey(S.String),
  score: S.optionalKey(S.Number),
  summary: S.optionalKey(S.String),
  text: S.optionalKey(S.String),
  title: S.optionalKey(S.String),
  url: S.String,
})
export type ExaSearchResult = typeof ExaSearchResult.Type

export const ExaContentsResult = S.Struct({
  author: S.optionalKey(S.String),
  highlights: S.optionalKey(S.Array(S.String)),
  id: S.optionalKey(S.String),
  publishedDate: S.optionalKey(S.String),
  summary: S.optionalKey(S.String),
  text: S.optionalKey(S.String),
  title: S.optionalKey(S.String),
  url: S.String,
})
export type ExaContentsResult = typeof ExaContentsResult.Type

export const ExaSearchResponse = S.Struct({
  autopromptString: S.optionalKey(S.String),
  costDollars: S.optionalKey(S.Number),
  grounding: S.optionalKey(S.Unknown),
  output: S.optionalKey(S.Unknown),
  requestId: S.optionalKey(S.String),
  resolvedSearchType: S.optionalKey(S.String),
  results: S.Array(ExaSearchResult),
  searchType: S.optionalKey(S.String),
})
export type ExaSearchResponse = typeof ExaSearchResponse.Type

export const ExaContentsResponse = S.Struct({
  costDollars: S.optionalKey(S.Number),
  requestId: S.optionalKey(S.String),
  results: S.Array(ExaContentsResult),
})
export type ExaContentsResponse = typeof ExaContentsResponse.Type

export class ExaConfigurationDisabled extends S.TaggedErrorClass<ExaConfigurationDisabled>()(
  'ExaConfigurationDisabled',
  {
    reason: S.String,
  },
) {}

export class ExaProviderHttpError extends S.TaggedErrorClass<ExaProviderHttpError>()(
  'ExaProviderHttpError',
  {
    endpoint: S.String,
    message: S.String,
    status: S.Number,
  },
) {}

export class ExaProviderInvalidJson extends S.TaggedErrorClass<ExaProviderInvalidJson>()(
  'ExaProviderInvalidJson',
  {
    endpoint: S.String,
    error: S.Defect,
  },
) {}

export class ExaProviderFetchError extends S.TaggedErrorClass<ExaProviderFetchError>()(
  'ExaProviderFetchError',
  {
    endpoint: S.String,
    error: S.Defect,
  },
) {}

export class ExaProviderSchemaError extends S.TaggedErrorClass<ExaProviderSchemaError>()(
  'ExaProviderSchemaError',
  {
    endpoint: S.String,
    error: S.Defect,
  },
) {}

export class ExaProviderTimeout extends S.TaggedErrorClass<ExaProviderTimeout>()(
  'ExaProviderTimeout',
  {
    endpoint: S.String,
    timeoutMs: S.Number,
  },
) {}

export type ExaError =
  | ExaConfigurationDisabled
  | ExaProviderFetchError
  | ExaProviderHttpError
  | ExaProviderInvalidJson
  | ExaProviderSchemaError
  | ExaProviderTimeout

export type ExaClientShape = Readonly<{
  getContents: (
    input: ExaContentsInput,
  ) => Effect.Effect<ExaContentsResponse, ExaError>
  search: (input: ExaSearchInput) => Effect.Effect<ExaSearchResponse, ExaError>
}>

export class ExaClient extends Context.Service<ExaClient, ExaClientShape>()(
  '@openagentsinc/autopilot-omega/ExaClient',
) {
  static layer = (fetcher: ExaFetch = globalThis.fetch.bind(globalThis)) =>
    Layer.effect(
      ExaClient,
      Effect.gen(function* () {
        const config = yield* OpenAgentsWorkerConfig

        return makeExaClient(config.exa, fetcher)
      }),
    )

  static envLayer = (
    env: OpenAgentsWorkerConfigEnv,
    fetcher: ExaFetch = globalThis.fetch.bind(globalThis),
  ) =>
    ExaClient.layer(fetcher).pipe(
      Layer.provide(OpenAgentsWorkerConfig.layer(env)),
    )
}

const summarizeProviderPayload = (payload: unknown): string => {
  if (payload === undefined || payload === null) {
    return 'Exa provider returned an HTTP error.'
  }

  const text =
    typeof payload === 'string'
      ? payload
      : JSON.stringify(payload, (_key, value: unknown) =>
          Redacted.isRedacted(value) ? '<redacted>' : value,
        )

  return redactProviderAccountSecretMaterial(text)
    .replace(/\s+/g, ' ')
    .slice(0, EXA_ERROR_SUMMARY_LIMIT)
}

const providerUrl = (config: ExaConfig, endpoint: ExaEndpoint): string =>
  `${config.baseUrl}${endpoint}`

const safeRequestSignal = (timeoutMs: number): AbortSignal | undefined =>
  typeof AbortSignal.timeout === 'function'
    ? AbortSignal.timeout(timeoutMs)
    : undefined

const isTimeoutError = (error: unknown): boolean =>
  error instanceof Error &&
  (error.name === 'AbortError' ||
    error.name === 'TimeoutError' ||
    error.message.toLowerCase().includes('timeout'))

const requireApiKey = (
  config: ExaConfig,
): Effect.Effect<Redacted.Redacted<WorkerSecret>, ExaConfigurationDisabled> =>
  config.enabled && config.apiKey !== undefined
    ? Effect.succeed(config.apiKey)
    : Effect.fail(
        new ExaConfigurationDisabled({
          reason: 'EXA_API_KEY is not configured.',
        }),
      )

const parseProviderJson = (
  endpoint: ExaEndpoint,
  response: ExaFetchResponse,
): Effect.Effect<unknown, ExaProviderInvalidJson> =>
  Effect.tryPromise({
    catch: error => new ExaProviderInvalidJson({ endpoint, error }),
    try: () => response.json(),
  })

const providerCostDollars = (value: unknown): number | undefined => {
  if (typeof value === 'number') {
    return value
  }

  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return undefined
  }

  const total = (value as { readonly total?: unknown }).total

  return typeof total === 'number' ? total : undefined
}

const normalizeProviderPayload = (payload: unknown): unknown => {
  if (
    typeof payload !== 'object' ||
    payload === null ||
    Array.isArray(payload)
  ) {
    return payload
  }

  const costDollars = providerCostDollars(
    (payload as { readonly costDollars?: unknown }).costDollars,
  )

  return costDollars === undefined
    ? payload
    : { ...(payload as Record<string, unknown>), costDollars }
}

const decodeSearchResponse = (
  endpoint: ExaEndpoint,
  payload: unknown,
): Effect.Effect<ExaSearchResponse, ExaProviderSchemaError> =>
  Effect.try({
    catch: error => new ExaProviderSchemaError({ endpoint, error }),
    try: () =>
      S.decodeUnknownSync(ExaSearchResponse)(normalizeProviderPayload(payload)),
  })

const decodeContentsResponse = (
  endpoint: ExaEndpoint,
  payload: unknown,
): Effect.Effect<ExaContentsResponse, ExaProviderSchemaError> =>
  Effect.try({
    catch: error => new ExaProviderSchemaError({ endpoint, error }),
    try: () =>
      S.decodeUnknownSync(ExaContentsResponse)(
        normalizeProviderPayload(payload),
      ),
  })

const providerFetch = (
  config: ExaConfig,
  fetcher: ExaFetch,
  endpoint: ExaEndpoint,
  body: unknown,
): Effect.Effect<unknown, ExaError> =>
  Effect.gen(function* () {
    const apiKey = yield* requireApiKey(config)
    const signal = safeRequestSignal(config.requestTimeoutMs)
    const response = yield* Effect.tryPromise({
      catch: error =>
        isTimeoutError(error)
          ? new ExaProviderTimeout({
              endpoint,
              timeoutMs: config.requestTimeoutMs,
            })
          : new ExaProviderFetchError({ endpoint, error }),
      try: () =>
        fetcher(providerUrl(config, endpoint), {
          body: JSON.stringify(body),
          headers: {
            accept: 'application/json',
            'content-type': 'application/json',
            'x-api-key': redactedValue(apiKey) ?? '',
          },
          method: 'POST',
          ...(signal === undefined ? {} : { signal }),
        }),
    })

    const payload = yield* parseProviderJson(endpoint, response)

    if (!response.ok) {
      return yield* new ExaProviderHttpError({
        endpoint,
        message: summarizeProviderPayload(payload),
        status: response.status,
      })
    }

    return payload
  })

const withSearchDefaults = (
  config: ExaConfig,
  input: ExaSearchInput,
): ExaSearchInput => ({
  ...input,
  contents: input.contents ?? {
    highlights: true,
    maxAgeHours: config.freshnessMaxAgeHours,
  },
  numResults: input.numResults ?? config.defaultNumResults,
  type: input.type ?? (config.defaultSearchType as ExaSearchType),
})

const withContentsDefaults = (
  config: ExaConfig,
  input: ExaContentsInput,
): ExaContentsInput => ({
  ...input,
  contents: {
    ...input.contents,
    maxAgeHours: input.contents.maxAgeHours ?? config.freshnessMaxAgeHours,
  },
})

export const makeExaClient = (
  config: ExaConfig,
  fetcher: ExaFetch = globalThis.fetch.bind(globalThis),
): ExaClientShape => ({
  getContents: Effect.fn('ExaClient.getContents')(function* (input) {
    const payload = yield* providerFetch(
      config,
      fetcher,
      '/contents',
      withContentsDefaults(config, input),
    )

    return yield* decodeContentsResponse('/contents', payload)
  }),
  search: Effect.fn('ExaClient.search')(function* (input) {
    const payload = yield* providerFetch(
      config,
      fetcher,
      '/search',
      withSearchDefaults(config, input),
    )

    return yield* decodeSearchResponse('/search', payload)
  }),
})
