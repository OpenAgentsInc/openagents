import { Effect } from 'effect'

import { projectVerifiedOutcomeReputation } from './verified-outcome-reputation'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

export const handleVerifiedOutcomeReputationApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectVerifiedOutcomeReputation()))
