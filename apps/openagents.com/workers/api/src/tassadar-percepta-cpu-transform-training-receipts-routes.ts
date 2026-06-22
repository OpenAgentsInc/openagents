import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
  projectTassadarPerceptaCpuTransformTrainingReceipts,
} from './tassadar-percepta-cpu-transform-training-receipts'

export { TassadarPerceptaCpuTransformTrainingReceiptsEndpoint }

export const handleTassadarPerceptaCpuTransformTrainingReceiptsApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse(
          projectTassadarPerceptaCpuTransformTrainingReceipts(),
        ),
      )
