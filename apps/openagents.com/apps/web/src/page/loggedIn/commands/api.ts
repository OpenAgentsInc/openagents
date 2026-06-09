import { Data, Effect, Schema as S } from 'effect'

export class ChatApiError extends Data.TaggedError('ChatApiError')<{
  cause: unknown
}> {}

export class ChatApiHttpError extends Data.TaggedError('ChatApiHttpError')<{
  payload: unknown
  status: number
}> {}

export const errorFromUnknown = (cause: unknown): ChatApiError =>
  new ChatApiError({ cause })

export const errorMessageFromUnknown = (error: unknown): string =>
  error instanceof ChatApiHttpError
    ? apiErrorMessage(error.status, error.payload)
    : error instanceof ChatApiError
      ? errorMessageFromUnknown(error.cause)
      : error instanceof Error
        ? error.message
        : String(error)

const jsonPayload = (
  response: Response,
): Effect.Effect<unknown, ChatApiError> =>
  Effect.tryPromise({
    try: () => response.json(),
    catch: errorFromUnknown,
  })

const errorPayload = (
  response: Response,
): Effect.Effect<unknown, ChatApiError> =>
  Effect.tryPromise({
    try: async () => {
      const contentType = response.headers.get('content-type') ?? ''

      if (contentType.includes('application/json')) {
        return await response.json()
      }

      const text = await response.text()

      return text.trim() === '' ? undefined : { error: text.trim() }
    },
    catch: errorFromUnknown,
  })

const recordFromUnknown = (
  value: unknown,
): Record<string, unknown> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value))
    : undefined

const textFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

export const apiErrorMessage = (status: number, payload: unknown): string => {
  const record = recordFromUnknown(payload)
  const message =
    record === undefined
      ? undefined
      : (textFromUnknown(record.message) ?? textFromUnknown(record.error))

  return message ?? `OpenAgents API returned HTTP ${status}.`
}

export const decodeJsonResponse = (
  response: Response,
): Effect.Effect<unknown, ChatApiError | ChatApiHttpError> =>
  Effect.gen(function* () {
    const payload = response.ok
      ? yield* jsonPayload(response)
      : yield* errorPayload(response)

    if (!response.ok) {
      return yield* new ChatApiHttpError({ payload, status: response.status })
    }

    return payload
  })

export const requestJson = <Schema extends S.Top>(options: {
  readonly catch?: (cause: unknown) => unknown
  readonly init?: RequestInit
  readonly name: string
  readonly request: RequestInfo | URL
  readonly schema: Schema
}) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () => fetch(options.request, options.init),
      catch: cause =>
        errorFromUnknown(
          options.catch === undefined ? cause : options.catch(cause),
        ),
    })
    const payload = yield* decodeJsonResponse(response)

    return yield* S.decodeUnknownEffect(options.schema)(payload)
  }).pipe(Effect.withSpan(options.name))

export const requestBlob = (options: {
  readonly init?: RequestInit
  readonly name: string
  readonly request: RequestInfo | URL
}) =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () => fetch(options.request, options.init),
      catch: errorFromUnknown,
    })

    if (!response.ok) {
      const payload = yield* errorPayload(response)

      return yield* new ChatApiHttpError({ payload, status: response.status })
    }

    return yield* Effect.tryPromise({
      try: () => response.blob(),
      catch: errorFromUnknown,
    })
  }).pipe(Effect.withSpan(options.name))
