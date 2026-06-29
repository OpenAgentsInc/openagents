import { Config, Effect, Schema as S } from 'effect'

export const BoundaryReasonRef = S.Literals([
  'boundary.json.malformed',
  'boundary.schema.invalid',
  'boundary.request.unreadable',
  'boundary.config.invalid',
])
export type BoundaryReasonRef = typeof BoundaryReasonRef.Type

export class BoundaryParseError extends S.TaggedErrorClass<BoundaryParseError>()(
  'BoundaryParseError',
  {
    operation: S.String,
    reasonRef: BoundaryReasonRef,
  },
) {}

const jsonParseError = (operation: string) =>
  new BoundaryParseError({
    operation,
    reasonRef: 'boundary.json.malformed',
  })

const schemaError = (operation: string) =>
  new BoundaryParseError({
    operation,
    reasonRef: 'boundary.schema.invalid',
  })

export const parseJsonEffect = <A>(
  schema: S.Decoder<A>,
  text: string,
  operation: string,
): Effect.Effect<A, BoundaryParseError> =>
  Effect.try({
    catch: () => jsonParseError(operation),
    try: () => JSON.parse(text) as unknown,
  }).pipe(
    Effect.flatMap(value =>
      S.decodeUnknownEffect(schema)(value).pipe(
        Effect.mapError(() => schemaError(operation)),
      ),
    ),
  )

export const readRequestJsonEffect = <A>(
  schema: S.Decoder<A>,
  request: Request,
  operation: string,
): Effect.Effect<A, BoundaryParseError> =>
  Effect.tryPromise({
    catch: () =>
      new BoundaryParseError({
        operation,
        reasonRef: 'boundary.request.unreadable',
      }),
    try: () => request.text(),
  }).pipe(Effect.flatMap(text => parseJsonEffect(schema, text, operation)))

export const decodeRowEffect = <A>(
  schema: S.Decoder<A>,
  row: unknown,
  operation: string,
): Effect.Effect<A, BoundaryParseError> =>
  S.decodeUnknownEffect(schema)(row).pipe(
    Effect.mapError(() => schemaError(operation)),
  )

export const readConfigEffect = <A>(
  config: Config.Config<A>,
  operation: string,
): Effect.Effect<A, BoundaryParseError> =>
  config.pipe(
    Effect.mapError(
      () =>
        new BoundaryParseError({
          operation,
          reasonRef: 'boundary.config.invalid',
        }),
    ),
  )

export const expectBoundaryParseError = (
  value: unknown,
  expected: {
    operation: string
    reasonRef: BoundaryReasonRef
  },
): BoundaryParseError => {
  if (
    value instanceof BoundaryParseError &&
    value.operation === expected.operation &&
    value.reasonRef === expected.reasonRef
  ) {
    return value
  }

  throw new Error(
    `Expected BoundaryParseError(${expected.operation}, ${expected.reasonRef})`,
  )
}
