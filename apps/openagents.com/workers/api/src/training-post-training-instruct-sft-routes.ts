import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingPostTrainingInstructSftEndpoint,
  projectTrainingPostTrainingInstructSft,
} from './training-post-training-instruct-sft'

export { TrainingPostTrainingInstructSftEndpoint }

export const handleTrainingPostTrainingInstructSftApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse(projectTrainingPostTrainingInstructSft()),
      )
