import { Effect } from 'effect'

import { projectEnergyFlexibleLoadProof } from './energy-flexible-load-proof'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export const handleEnergyFlexibleLoadProofApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectEnergyFlexibleLoadProof()))
