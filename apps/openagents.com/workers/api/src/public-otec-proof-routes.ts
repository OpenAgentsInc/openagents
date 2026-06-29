import { Effect, Match as M } from 'effect'

import {
  PublicOtecProofNotFound,
  PublicOtecProofStorageError,
  PublicOtecProofUnsafe,
  publicOtecProofCloseout,
} from './public-otec-proof'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { openAgentsDatabase } from './runtime'

type PublicOtecProofEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type PublicOtecProofRouteError =
  | PublicOtecProofNotFound
  | PublicOtecProofStorageError
  | PublicOtecProofUnsafe

const routeErrorResponse = (error: PublicOtecProofRouteError) =>
  M.value(error).pipe(
    M.tags({
      PublicOtecProofNotFound: () =>
        noStoreJsonResponse(
          { error: 'public_otec_proof_not_found' },
          { status: 404 },
        ),
      PublicOtecProofStorageError: () =>
        noStoreJsonResponse(
          { error: 'public_otec_proof_unavailable' },
          { status: 500 },
        ),
      PublicOtecProofUnsafe: () =>
        noStoreJsonResponse(
          { error: 'public_otec_proof_unsafe' },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

export const handlePublicOtecProofApi = (
  request: Request,
  env: PublicOtecProofEnv,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : publicOtecProofCloseout(openAgentsDatabase(env)).pipe(
        Effect.map(proof => noStoreJsonResponse(proof)),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
