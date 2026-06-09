import { Context, Effect, Layer } from 'effect'

import {
  type AgentRunBundle,
  type AgentRunRecord,
  type OmniEventRecord,
  type OmniRunStore,
} from '../omni-runs'
import { type OmniError, omniErrorFromUnknown } from './errors'

export type OmniRunRepositoryShape = Readonly<{
  appendAgentRunEvents: (
    runId: string,
    events: ReadonlyArray<OmniEventRecord>,
    status?: AgentRunRecord['status'],
    externalRunId?: string,
  ) => Effect.Effect<void, OmniError>
  findAgentRunForUser: (
    userId: string,
    runId: string,
  ) => Effect.Effect<AgentRunBundle | undefined, OmniError>
  listAgentRunsForUser: (
    userId: string,
    limit: number,
  ) => Effect.Effect<ReadonlyArray<AgentRunBundle>, OmniError>
  saveAgentRun: (
    run: AgentRunRecord,
    events: ReadonlyArray<OmniEventRecord>,
  ) => Effect.Effect<void, OmniError>
}>

export class OmniRunRepository extends Context.Service<
  OmniRunRepository,
  OmniRunRepositoryShape
>()('openagents/OmniRunRepository') {}

const repositoryEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OmniError> =>
  Effect.tryPromise({
    try: run,
    catch: error => omniErrorFromUnknown(operation, error),
  })

export const makeOmniRunRepository = (
  store: OmniRunStore,
): OmniRunRepositoryShape => ({
  appendAgentRunEvents: (runId, events, status, externalRunId) =>
    repositoryEffect('append_agent_run_events', () =>
      store.appendAgentRunEvents(runId, events, status, externalRunId),
    ).pipe(Effect.withSpan('OmniRunRepository.appendAgentRunEvents')),
  findAgentRunForUser: (userId, runId) =>
    repositoryEffect('find_agent_run_for_user', () =>
      store.findAgentRunForUser(userId, runId),
    ).pipe(Effect.withSpan('OmniRunRepository.findAgentRunForUser')),
  listAgentRunsForUser: (userId, limit) =>
    repositoryEffect('list_agent_runs_for_user', () =>
      store.listAgentRunsForUser(userId, limit),
    ).pipe(Effect.withSpan('OmniRunRepository.listAgentRunsForUser')),
  saveAgentRun: (run, events) =>
    repositoryEffect('save_agent_run', () =>
      store.saveAgentRun(run, events),
    ).pipe(Effect.withSpan('OmniRunRepository.saveAgentRun')),
})

export const makeOmniRunRepositoryLayer = (store: OmniRunStore) =>
  Layer.succeed(OmniRunRepository, makeOmniRunRepository(store))
