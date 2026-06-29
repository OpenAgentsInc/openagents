import { Effect, Match as M } from 'effect'

import {
  PublicAdjutantActivityStorageError,
  PublicAdjutantActivityUnsafe,
  publicAdjutantActivity,
} from './adjutant-public-activity'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'

type PublicAdjutantActivityEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type PublicAdjutantActivityRouteError =
  | PublicAdjutantActivityStorageError
  | PublicAdjutantActivityUnsafe

type HttpResponse = globalThis.Response

const routeErrorResponse = (
  error: PublicAdjutantActivityRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      PublicAdjutantActivityStorageError: () =>
        noStoreJsonResponse(
          { error: 'public_adjutant_activity_unavailable' },
          { status: 500 },
        ),
      PublicAdjutantActivityUnsafe: () =>
        noStoreJsonResponse(
          { error: 'public_adjutant_activity_unsafe' },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

export const handlePublicAdjutantActivityApi = (
  request: Request,
  env: PublicAdjutantActivityEnv,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : publicAdjutantActivity(openAgentsDatabase(env)).pipe(
        Effect.map(activity => noStoreJsonResponse(activity)),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
