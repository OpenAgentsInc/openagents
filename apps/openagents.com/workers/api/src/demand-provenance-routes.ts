import { Effect } from 'effect'

import { projectDemandProvenance } from './demand-provenance'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export const handleDemandProvenanceApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectDemandProvenance()))
