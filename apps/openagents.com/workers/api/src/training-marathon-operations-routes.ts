import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  TrainingMarathonOperationsEndpoint,
  projectTrainingMarathonOperations,
} from './training-marathon-operations'

export { TrainingMarathonOperationsEndpoint }

export const handleTrainingMarathonOperationsApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(projectTrainingMarathonOperations()))
