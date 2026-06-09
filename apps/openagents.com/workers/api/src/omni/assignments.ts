import { Context, Effect, Layer } from 'effect'

import {
  type AgentRunRecord,
  type DeploymentRecord,
  type OmniEventRecord,
  createQueuedAgentRun,
  createQueuedDeployment,
} from '../omni-runs'
import { type OmniError, omniAssignmentErrorFromUnknown } from './errors'

export type QueuedAgentRun = Readonly<{
  events: ReadonlyArray<OmniEventRecord>
  run: AgentRunRecord
}>

export type QueuedDeployment = Readonly<{
  deployment: DeploymentRecord
  events: ReadonlyArray<OmniEventRecord>
}>

export type OmniAssignmentServiceShape = Readonly<{
  createQueuedAgentRun: (
    input: Parameters<typeof createQueuedAgentRun>[0],
  ) => Effect.Effect<QueuedAgentRun, OmniError>
  createQueuedDeployment: (
    input: Parameters<typeof createQueuedDeployment>[0],
  ) => Effect.Effect<QueuedDeployment, OmniError>
}>

export class OmniAssignmentService extends Context.Service<
  OmniAssignmentService,
  OmniAssignmentServiceShape
>()('openagents/OmniAssignmentService') {}

const assignmentEffect = <A>(
  operation: string,
  run: () => A,
): Effect.Effect<A, OmniError> =>
  Effect.try({
    try: run,
    catch: error => omniAssignmentErrorFromUnknown(operation, error),
  })

export const makeOmniAssignmentService = (): OmniAssignmentServiceShape => ({
  createQueuedAgentRun: input =>
    assignmentEffect('create_queued_agent_run', () =>
      createQueuedAgentRun(input),
    ).pipe(Effect.withSpan('OmniAssignmentService.createQueuedAgentRun')),
  createQueuedDeployment: input =>
    assignmentEffect('create_queued_deployment', () =>
      createQueuedDeployment(input),
    ).pipe(Effect.withSpan('OmniAssignmentService.createQueuedDeployment')),
})

export const OmniAssignmentServiceLive = Layer.succeed(
  OmniAssignmentService,
  makeOmniAssignmentService(),
)
