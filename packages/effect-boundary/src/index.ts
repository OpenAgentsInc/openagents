import { Cause, Config, ConfigProvider, Effect, Redacted, Schema as S } from "effect"

export type BoundaryKind =
  | "config"
  | "file_json"
  | "json"
  | "request_json"
  | "row"
  | "unknown"

export class OpenAgentsBoundaryError extends S.TaggedErrorClass<OpenAgentsBoundaryError>()(
  "OpenAgentsBoundaryError",
  {
    boundary: S.Literals(["config", "file_json", "json", "request_json", "row", "unknown"]),
    operation: S.String,
    reasonRef: S.String,
    message: S.String,
    cause: S.optional(S.Defect),
  },
) {}

export type BoundaryDecoder<A> = S.Decoder<A>

const boundaryError = (input: {
  boundary: BoundaryKind
  cause?: unknown
  message: string
  operation: string
  reasonRef: string
}): OpenAgentsBoundaryError =>
  new OpenAgentsBoundaryError({
    boundary: input.boundary,
    operation: input.operation,
    reasonRef: input.reasonRef,
    message: input.message,
    ...(input.cause === undefined ? {} : { cause: input.cause }),
  })

const decodeBoundaryEffect = <A>(
  schema: BoundaryDecoder<A>,
  value: unknown,
  operation: string,
  boundary: BoundaryKind,
): Effect.Effect<A, OpenAgentsBoundaryError> =>
  S.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(error =>
      boundaryError({
        boundary,
        operation,
        reasonRef: "boundary.schema.invalid",
        message: "Boundary payload did not match the expected schema.",
        cause: error,
      })
    ),
  )

export const decodeUnknownEffect = <A>(
  schema: BoundaryDecoder<A>,
  value: unknown,
  operation: string,
): Effect.Effect<A, OpenAgentsBoundaryError> =>
  decodeBoundaryEffect(schema, value, operation, "unknown")

export const parseJsonEffect = <A>(
  schema: BoundaryDecoder<A>,
  text: string,
  operation: string,
): Effect.Effect<A, OpenAgentsBoundaryError> =>
  Effect.try({
    try: () => JSON.parse(text) as unknown,
    catch: error =>
      boundaryError({
        boundary: "json",
        operation,
        reasonRef: "boundary.json.malformed",
        message: "Boundary JSON could not be parsed.",
        cause: error,
      }),
  }).pipe(
    Effect.flatMap(value => decodeBoundaryEffect(schema, value, operation, "json")),
  )

export const readRequestJsonEffect = <A>(
  schema: BoundaryDecoder<A>,
  request: { readonly text: () => Promise<string> },
  operation: string,
): Effect.Effect<A, OpenAgentsBoundaryError> =>
  Effect.tryPromise({
    try: () => request.text(),
    catch: error =>
      boundaryError({
        boundary: "request_json",
        operation,
        reasonRef: "boundary.request.unreadable",
        message: "Request body could not be read.",
        cause: error,
      }),
  }).pipe(
    Effect.flatMap(text =>
      parseJsonEffect(schema, text, operation).pipe(
        Effect.mapError(error =>
          new OpenAgentsBoundaryError({
            boundary: "request_json",
            operation: error.operation,
            reasonRef: error.reasonRef,
            message: error.message,
            ...(error.cause === undefined ? {} : { cause: error.cause }),
          })
        ),
      )
    ),
  )

export const readJsonFileEffect = <A>(
  schema: BoundaryDecoder<A>,
  readText: () => Promise<string>,
  operation: string,
): Effect.Effect<A, OpenAgentsBoundaryError> =>
  Effect.tryPromise({
    try: readText,
    catch: error =>
      boundaryError({
        boundary: "file_json",
        operation,
        reasonRef: "boundary.file.unreadable",
        message: "JSON file could not be read.",
        cause: error,
      }),
  }).pipe(
    Effect.flatMap(text =>
      parseJsonEffect(schema, text, operation).pipe(
        Effect.mapError(error =>
          new OpenAgentsBoundaryError({
            boundary: "file_json",
            operation: error.operation,
            reasonRef: error.reasonRef,
            message: error.message,
            ...(error.cause === undefined ? {} : { cause: error.cause }),
          })
        ),
      )
    ),
  )

export const decodeRowEffect = <A>(
  schema: BoundaryDecoder<A>,
  row: unknown,
  operation: string,
): Effect.Effect<A, OpenAgentsBoundaryError> =>
  decodeBoundaryEffect(schema, row, operation, "row").pipe(
    Effect.mapError(error =>
      new OpenAgentsBoundaryError({
        boundary: error.boundary,
        operation: error.operation,
        reasonRef: "boundary.row.invalid",
        message: error.message,
        ...(error.cause === undefined ? {} : { cause: error.cause }),
      })
    ),
  )

export const readRedactedConfigEffect = (
  name: string,
  operation: string,
): Effect.Effect<Redacted.Redacted, OpenAgentsBoundaryError> =>
  Config.redacted(name).pipe(
    Effect.mapError(error =>
      boundaryError({
        boundary: "config",
        operation,
        reasonRef: "boundary.config.missing_or_invalid",
        message: "Required redacted config value is missing or invalid.",
        cause: error,
      })
    ),
  )

export const boundaryTestConfigLayer = (
  values: Record<string, unknown>,
) => ConfigProvider.layer(ConfigProvider.fromUnknown(values))

export const expectBoundaryFailure = <A, E, R>(
  effect: Effect.Effect<A, E, R>,
  expected?: Partial<Pick<OpenAgentsBoundaryError, "operation" | "reasonRef" | "boundary">>,
): Effect.Effect<OpenAgentsBoundaryError, OpenAgentsBoundaryError, R> =>
  Effect.exit(effect).pipe(
    Effect.flatMap(exit => {
      if (exit._tag === "Failure") {
        const reason = exit.cause.reasons.find(Cause.isFailReason)
        const failure = reason?.error
        if (failure instanceof OpenAgentsBoundaryError) {
          const mismatches = [
            expected?.operation !== undefined && failure.operation !== expected.operation
              ? "operation"
              : null,
            expected?.reasonRef !== undefined && failure.reasonRef !== expected.reasonRef
              ? "reasonRef"
              : null,
            expected?.boundary !== undefined && failure.boundary !== expected.boundary
              ? "boundary"
              : null,
          ].filter((item): item is string => item !== null)
          return mismatches.length === 0
            ? Effect.succeed(failure)
            : Effect.fail(
              boundaryError({
                boundary: "unknown",
                operation: expected?.operation ?? failure.operation,
                reasonRef: "boundary.test.failure_mismatch",
                message: `Boundary failure assertion mismatch: ${mismatches.join(", ")}`,
              }),
            )
        }
      }

      return Effect.fail(
        boundaryError({
          boundary: "unknown",
          operation: expected?.operation ?? "boundary.test",
          reasonRef: "boundary.test.expected_failure",
          message: "Expected an OpenAgentsBoundaryError failure.",
        }),
      )
    }),
  )
