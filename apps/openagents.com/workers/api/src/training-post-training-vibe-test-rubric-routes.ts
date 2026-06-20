import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingPostTrainingVibeTestRubricEndpoint,
  projectTrainingPostTrainingVibeTestRubric,
} from './training-post-training-vibe-test-rubric'

export { TrainingPostTrainingVibeTestRubricEndpoint }

export const handleTrainingPostTrainingVibeTestRubricApi = (
  request: Request,
) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.promise(() => projectTrainingPostTrainingVibeTestRubric()).pipe(
        Effect.map(noStoreJsonResponse),
      )
