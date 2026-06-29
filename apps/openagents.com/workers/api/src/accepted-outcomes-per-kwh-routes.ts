import { Effect } from 'effect'

import { projectAcceptedOutcomesPerKwh } from './accepted-outcomes-per-kwh'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export const handleAcceptedOutcomesPerKwhApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectAcceptedOutcomesPerKwh()))
