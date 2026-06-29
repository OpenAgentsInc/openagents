import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from './responses'

export class RouteDependencyError extends S.TaggedErrorClass<RouteDependencyError>()(
  'RouteDependencyError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export const RouteError = S.Union([RouteDependencyError])
export type RouteError = typeof RouteError.Type

export type RouteEffect = Effect.Effect<Response, RouteError>

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

export const routeEffect = (
  operation: string,
  run: () => Promise<Response>,
): RouteEffect =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new RouteDependencyError({
        operation,
        message: errorMessage(error),
      }),
  })

export const routeErrorResponse = (error: RouteError): Response =>
  noStoreJsonResponse(
    {
      error: error._tag,
      message: error.message,
      operation: error.operation,
    },
    { status: 500 },
  )

export const routeEffectOrResponse = (
  effect: RouteEffect,
): Effect.Effect<Response> =>
  effect.pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
