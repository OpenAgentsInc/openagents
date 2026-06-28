import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TraceEconomicUnderwritingEndpoint,
  projectTraceEconomicUnderwriting,
} from './trace-economic-underwriting'

export { TraceEconomicUnderwritingEndpoint }

export const handleTraceEconomicUnderwritingApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectTraceEconomicUnderwriting()))
