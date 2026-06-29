import { Config, ConfigProvider, Effect, Layer, Schema as S } from "effect"

export type BoundaryKind =
  | "json"
  | "request_json"
  | "file_json"
  | "row"
  | "config"
  | "test"

export class OpenAgentsBoundaryError extends S.TaggedErrorClass<OpenAgentsBoundaryError>()(
  "OpenAgentsBoundaryError",
  {
    boundary: S.Literals(["json", "request_json", "file_json", "row", "config", "test"]),
    operation: S.String,
    reason: S.String,
    reasonRef: S.String,
    sourceRef: S.optional(S.String),
  },
) {}

export type BoundaryDecodeEffect<A> = Effect.Effect<A, OpenAgentsBoundaryError>

const safeRefSegment = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._:-]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 96) || "unknown"

export const boundaryReasonRef = (
  boundary: BoundaryKind,
  operation: string,
  reason: string,
): string =>
  `boundary.${safeRefSegment(boundary)}.${safeRefSegment(operation)}.${safeRefSegment(reason)}`

const boundaryError = (
  boundary: BoundaryKind,
  operation: string,
  reason: string,
  sourceRef?: string,
): OpenAgentsBoundaryError =>
  new OpenAgentsBoundaryError({
    boundary,
    operation,
    reason,
    reasonRef: boundaryReasonRef(boundary, operation, reason),
    ...(sourceRef === undefined ? {} : { sourceRef }),
  })

export const parseJsonEffect = <A>(
  schema: S.Decoder<A>,
  text: string,
  operation: string,
): BoundaryDecodeEffect<A> =>
  Effect.gen(function* () {
    const parsed = yield* Effect.try({
      try: () => JSON.parse(text) as unknown,
      catch: () => boundaryError("json", operation, "malformed_json"),
    })

    return yield* decodeUnknownEffect(schema, parsed, operation, "json")
  })

export const readRequestJsonEffect = <A>(
  schema: S.Decoder<A>,
  request: Request,
  operation: string,
): BoundaryDecodeEffect<A> =>
  Effect.gen(function* () {
    const text = yield* Effect.tryPromise({
      try: () => request.text(),
      catch: () => boundaryError("request_json", operation, "body_unreadable"),
    })
    return yield* parseJsonEffect(schema, text.trim() === "" ? "{}" : text, operation).pipe(
      Effect.mapError(error =>
        error.reason === "malformed_json"
          ? boundaryError("request_json", operation, "malformed_json")
          : boundaryError("request_json", operation, error.reason),
      ),
    )
  })

export const parseLocalStateJsonEffect = <A>(
  schema: S.Decoder<A>,
  text: string,
  operation: string,
  sourceRef?: string,
): BoundaryDecodeEffect<A> =>
  parseJsonEffect(schema, text, operation).pipe(
    Effect.mapError(error =>
      boundaryError(
        "file_json",
        operation,
        error.reason === "malformed_json" ? "malformed_json" : error.reason,
        sourceRef,
      ),
    ),
  )

export const decodeUnknownEffect = <A>(
  schema: S.Decoder<A>,
  value: unknown,
  operation: string,
  boundary: BoundaryKind = "json",
): BoundaryDecodeEffect<A> =>
  Effect.try({
    try: () => S.decodeUnknownSync(schema)(value),
    catch: () => boundaryError(boundary, operation, "schema_mismatch"),
  })

export const decodeRowEffect = <A>(
  schema: S.Decoder<A>,
  row: unknown,
  operation: string,
): BoundaryDecodeEffect<A> => decodeUnknownEffect(schema, row, operation, "row")

export const readRedactedConfigEffect = <A>(
  config: Config.Config<A>,
  operation: string,
): BoundaryDecodeEffect<A> =>
  config.pipe(
    Effect.mapError(() => boundaryError("config", operation, "config_unavailable")),
  )

export const boundaryConfigLayer = (
  values: Record<string, unknown>,
): Layer.Layer<never> => ConfigProvider.layer(ConfigProvider.fromUnknown(values))

export const redactedFixture = (
  key: string,
): { readonly key: string; readonly value: "<redacted>"; readonly reasonRef: string } => ({
  key,
  value: "<redacted>",
  reasonRef: boundaryReasonRef("test", key, "redacted_fixture"),
})

export const deterministicBoundaryTestContext = (
  seed: string,
): { readonly seed: string; readonly now: Date; readonly random: () => number } => {
  let state = 0
  for (let index = 0; index < seed.length; index += 1) {
    state = (state * 31 + seed.charCodeAt(index)) >>> 0
  }
  return {
    seed,
    now: new Date("2026-01-01T00:00:00.000Z"),
    random: () => {
      state = (1664525 * state + 1013904223) >>> 0
      return state / 0x100000000
    },
  }
}

export const assertEffectFailsWithBoundaryError = async <A, E, R>(
  effect: Effect.Effect<A, E, R>,
): Promise<OpenAgentsBoundaryError> => {
  try {
    await Effect.runPromise(effect as Effect.Effect<A, unknown>)
  } catch (error) {
    if (error instanceof OpenAgentsBoundaryError) {
      return error
    }
    const maybeCause = error as { readonly cause?: { readonly error?: unknown } }
    if (maybeCause.cause?.error instanceof OpenAgentsBoundaryError) {
      return maybeCause.cause.error
    }
  }
  throw boundaryError("test", "assertEffectFailsWithBoundaryError", "expected_boundary_failure")
}
