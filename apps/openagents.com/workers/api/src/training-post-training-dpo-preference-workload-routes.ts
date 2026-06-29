import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingPostTrainingDpoPreferenceWorkloadEndpoint,
  projectTrainingPostTrainingDpoPreferenceWorkload,
} from './training-post-training-dpo-preference-workload'

export { TrainingPostTrainingDpoPreferenceWorkloadEndpoint }

export const handleTrainingPostTrainingDpoPreferenceWorkloadApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse(
          projectTrainingPostTrainingDpoPreferenceWorkload(),
        ),
      )
