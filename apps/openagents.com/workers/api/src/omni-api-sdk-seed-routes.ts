import { Effect, Match as M } from 'effect'

import {
  OmniApiSdkSeedUnsafe,
  omniApiSdkSeed,
} from './omni-api-sdk-seed'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

const routeErrorResponse = (error: OmniApiSdkSeedUnsafe) =>
  M.value(error).pipe(
    M.tags({
      OmniApiSdkSeedUnsafe: seedError =>
        noStoreJsonResponse(
          {
            error: 'omni_api_sdk_seed_unsafe',
            reason: seedError.reason,
          },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

export const handleOmniApiSdkSeedApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : omniApiSdkSeed().pipe(
        Effect.map(seed => noStoreJsonResponse(seed)),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
