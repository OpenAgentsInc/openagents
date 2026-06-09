import { Effect, Match as M } from 'effect'

import {
  OpenAgentsOpenApiUnsafe,
  openAgentsOpenApiDocument,
} from './openagents-openapi'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

const routeErrorResponse = (error: OpenAgentsOpenApiUnsafe) =>
  M.value(error).pipe(
    M.tags({
      OpenAgentsOpenApiUnsafe: () =>
        noStoreJsonResponse(
          { error: 'openagents_openapi_document_unsafe' },
          { status: 500 },
        ),
    }),
    M.exhaustive,
  )

export const handleOpenAgentsOpenApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : openAgentsOpenApiDocument().pipe(
        Effect.map(document => noStoreJsonResponse(document)),
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
