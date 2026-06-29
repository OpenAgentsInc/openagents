import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TassadarPerceptaArchitectureReceiptsEndpoint,
  projectTassadarPerceptaArchitectureReceipts,
} from './tassadar-percepta-architecture-receipts'

export { TassadarPerceptaArchitectureReceiptsEndpoint }

export const handleTassadarPerceptaArchitectureReceiptsApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse(projectTassadarPerceptaArchitectureReceipts()),
      )
