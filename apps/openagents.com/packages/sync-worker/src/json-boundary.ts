import { Effect, Schema as S } from 'effect'

const JsonValue = S.Json
const decodeJsonValue = S.decodeUnknownEffect(JsonValue)

const errorReason = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

export const decodeJsonValueEffect = <E>(
  value: string,
  makeError: (reason: string) => E,
): Effect.Effect<unknown, E> =>
  Effect.try({
    catch: error => makeError(errorReason(error)),
    try: () => JSON.parse(value),
  }).pipe(
    Effect.flatMap(parsed =>
      decodeJsonValue(parsed).pipe(
        Effect.mapError(error => makeError(errorReason(error))),
      ),
    ),
  )
