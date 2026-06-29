import {
  Config,
  ConfigProvider,
  Cause,
  Effect,
  Layer,
  Random,
  Redacted,
  Schema as S,
} from "effect"

export class EffectBoundaryError extends S.TaggedErrorClass<EffectBoundaryError>()(
  "EffectBoundaryError",
  {
    boundary: S.Literals(["json", "request_json", "file_json", "row", "config"]),
    contentRedacted: S.Literal(true),
    operation: S.String,
    reasonKind: S.Literals(["malformed_json", "schema_decode", "read_failed", "config_failed"]),
    reasonRef: S.String,
  },
) {}

export type EffectBoundaryKind = EffectBoundaryError["boundary"]

const sanitizeOperation = (operation: string): string =>
  operation
    .trim()
    .replace(/[^a-zA-Z0-9._:-]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 96) || "unknown"

const reasonRefFor = (
  boundary: EffectBoundaryKind,
  operation: string,
  reason: string,
): string => {
  const normalizedReason = reason.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "")
  const shortReason = normalizedReason.slice(0, 64) || "decode_failed"
  return `boundary.${boundary}.${sanitizeOperation(operation)}.${shortReason}`
}

const errorReason = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const boundaryError = (
  boundary: EffectBoundaryKind,
  operation: string,
  reasonKind: EffectBoundaryError["reasonKind"],
  reason: string,
): EffectBoundaryError =>
  new EffectBoundaryError({
    boundary,
    contentRedacted: true,
    operation,
    reasonKind,
    reasonRef: reasonRefFor(boundary, operation, reason),
  })

export const parseJsonEffect = <A>(
  schema: S.Decoder<A>,
  text: string,
  operation: string,
): Effect.Effect<A, EffectBoundaryError> =>
  Effect.try({
    catch: error => boundaryError("json", operation, "malformed_json", errorReason(error)),
    try: () => JSON.parse(text) as unknown,
  }).pipe(
    Effect.flatMap(value =>
      S.decodeUnknownEffect(schema)(value).pipe(
        Effect.mapError(error => boundaryError("json", operation, "schema_decode", errorReason(error))),
      ),
    ),
  )

export const readRequestJsonEffect = <A>(
  schema: S.Decoder<A>,
  request: Request,
  operation: string,
): Effect.Effect<A, EffectBoundaryError> =>
  Effect.tryPromise({
    catch: error => boundaryError("request_json", operation, "read_failed", errorReason(error)),
    try: () => request.text(),
  }).pipe(
    Effect.flatMap(text =>
      parseJsonEffect(schema, text, operation).pipe(
        Effect.mapError(error =>
          new EffectBoundaryError({
            ...error,
            boundary: "request_json",
            reasonRef: error.reasonRef.replace("boundary.json.", "boundary.request_json."),
          }),
        ),
      ),
    ),
  )

export const readFileJsonEffect = <A>(
  schema: S.Decoder<A>,
  readText: Effect.Effect<string, unknown>,
  operation: string,
): Effect.Effect<A, EffectBoundaryError> =>
  readText.pipe(
    Effect.mapError(error => boundaryError("file_json", operation, "read_failed", errorReason(error))),
    Effect.flatMap(text =>
      parseJsonEffect(schema, text, operation).pipe(
        Effect.mapError(error =>
          new EffectBoundaryError({
            ...error,
            boundary: "file_json",
            reasonRef: error.reasonRef.replace("boundary.json.", "boundary.file_json."),
          }),
        ),
      ),
    ),
  )

export const decodeRowEffect = <A>(
  schema: S.Decoder<A>,
  row: unknown,
  operation: string,
): Effect.Effect<A, EffectBoundaryError> =>
  S.decodeUnknownEffect(schema)(row).pipe(
    Effect.mapError(error => boundaryError("row", operation, "schema_decode", errorReason(error))),
  )

export const readConfigEffect = <A>(
  config: Config.Config<A>,
  operation: string,
): Effect.Effect<A, EffectBoundaryError> =>
  config.pipe(
    Effect.mapError(error => boundaryError("config", operation, "config_failed", errorReason(error))),
  )

export const readRedactedStringConfigEffect = (
  name: string,
  operation: string = `config.${name}`,
): Effect.Effect<Redacted.Redacted<string>, EffectBoundaryError> =>
  readConfigEffect(Config.redacted(name), operation)

export const configOverridesLayer = (
  values: Record<string, unknown>,
): Layer.Layer<never> => ConfigProvider.layer(ConfigProvider.fromUnknown(values))

export const redactedFixture = <A>(value: A): Redacted.Redacted<A> =>
  Redacted.make(value)

export const effectFailure = async <E>(
  effect: Effect.Effect<unknown, E>,
): Promise<E> => {
  const exit = await Effect.runPromiseExit(effect)
  if (exit._tag === "Failure") {
    const fail = exit.cause.reasons.find(Cause.isFailReason)
    if (fail !== undefined) {
      return fail.error
    }
  }
  throw new Error("Expected Effect to fail with a typed error")
}

export const effectFailureTag = async (
  effect: Effect.Effect<unknown, { readonly _tag: string }>,
): Promise<string> => (await effectFailure(effect))._tag

export const withDeterministicRandom = <A, E, R>(
  effect: Effect.Effect<A, E, R>,
  seed: string,
): Effect.Effect<A, E, R> => Random.withSeed(effect, seed)

export const deterministicNowEffect = (iso: string): Effect.Effect<Date> =>
  Effect.succeed(new Date(iso))
