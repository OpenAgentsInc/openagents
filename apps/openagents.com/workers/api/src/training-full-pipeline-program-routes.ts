import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingFullPipelineProgramEndpoint,
  projectTrainingFullPipelineProgram,
} from './training-full-pipeline-program'

export { TrainingFullPipelineProgramEndpoint }

export const handleTrainingFullPipelineProgramApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectTrainingFullPipelineProgram()))
