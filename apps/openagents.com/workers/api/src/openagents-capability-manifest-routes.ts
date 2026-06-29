import { Effect, Match as M } from 'effect'

import {
  OpenAgentsCapabilityManifestUnsafe,
  openAgentsCapabilityManifest,
} from './openagents-capability-manifest'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

const routeErrorResponse = (error: OpenAgentsCapabilityManifestUnsafe) =>
  M.value(error).pipe(
    M.tags({
      OpenAgentsCapabilityManifestUnsafe: () =>
        noStoreJsonResponse(
          { error: 'openagents_capability_manifest_unsafe' },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

export const handleOpenAgentsCapabilityManifestApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : openAgentsCapabilityManifest().pipe(
        Effect.map(manifest => noStoreJsonResponse(manifest)),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
