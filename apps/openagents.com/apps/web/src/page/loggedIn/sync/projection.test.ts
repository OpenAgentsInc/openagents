import {
  CollectionName,
  EntityId,
  IsoTimestamp,
  MutationId,
  SyncPatch,
  SyncScope,
  SyncSequence,
  SyncSnapshot,
} from '@openagentsinc/sync-schema'
import { Option } from 'effect'
import { describe, expect, test } from 'vitest'

import { type AgentRunDetailResponse, SyncClientModel } from '../model'
import {
  activeChatRunFromResponse,
  activeChatRunWithSyncedEventPatch,
  activeChatRunWithSyncedRunPatch,
  agentGoalFromSyncCollections,
  agentRunResponseFromSyncCollections,
  displayRunId,
  runAuthorLabel,
  sidebarMissionFromRunResponse,
  syncSnapshotHref,
  syncWithPatch,
  syncWithSnapshot,
} from './projection'

const runResponse = {
  run: {
    id: 'agent_run_1',
    runtime: 'opencode_codex',
    backend: 'shc_vm',
    runnerId: 'oa-shc-katy-01',
    userId: 'github:14167547',
    teamId: 'team_openagents_core',
    repository: {
      provider: 'github',
      owner: 'OpenAgentsInc',
      repo: 'autopilot-omega',
      ref: 'main',
    },
    goal: 'Run the smoke test',
    externalRunId: 'shc:oa-shc-katy-01:agent_run_1',
    status: 'running',
    eventCursor: 2,
    createdAt: '2026-06-03T00:00:00.000Z',
    updatedAt: '2026-06-03T00:00:01.000Z',
  },
  events: [
    {
      id: 'event_1',
      parentId: 'agent_run_1',
      sequence: 1,
      type: 'agent_run.accepted',
      summary: 'Accepted',
      status: 'queued',
      source: 'openagents',
      payloadJson: null,
      artifactRefs: [],
      externalEventId: null,
      createdAt: '2026-06-03T00:00:00.000Z',
    },
    {
      id: 'event_2',
      parentId: 'agent_run_1',
      sequence: 2,
      type: 'runner.usage',
      summary: 'Usage',
      status: 'running',
      source: 'shc',
      payloadJson:
        '{"usage":{"provider":"openai","model":"gpt-5","inputTokens":10,"outputTokens":8,"reasoningTokens":4}}',
      artifactRefs: ['result.md'],
      externalEventId: 'shc-event-2',
      createdAt: '2026-06-03T00:00:01.000Z',
    },
  ],
} satisfies AgentRunDetailResponse

const emptySync = SyncClientModel({
  collectionByScope: {},
  connectionByScope: {},
  cursors: {},
  pendingMutations: {
    mutation_1: {
      command: 'test',
      payload: {},
      scope: 'workspace:team_openagents',
    },
  },
  workspaceScope: 'workspace:team_openagents',
})

describe('logged-in sync projection helpers', () => {
  test('projects token usage from run event payloads', () => {
    const chatRun = activeChatRunFromResponse(runResponse)

    expect(chatRun.metadata.tokenTotal).toBe(22)
    expect(chatRun.metadata.tokenUsageEvents).toBe(1)
    expect(Option.getOrUndefined(chatRun.events[1]!.tokenProvider)).toBe(
      'openai',
    )
    expect(Option.getOrUndefined(chatRun.events[1]!.tokenModel)).toBe('gpt-5')
  })

  test('applies active event sync patches without rebuilding the run response', () => {
    const chatRun = activeChatRunFromResponse(runResponse)
    const putPatch = new SyncPatch({
      scope: SyncScope.make('agent-run:agent_run_1'),
      seq: SyncSequence.make(3),
      collection: CollectionName.make('agent_run_events'),
      op: 'put',
      id: EntityId.make('event_3'),
      value: {},
      serverTime: IsoTimestamp.make('2026-06-03T00:00:03.000Z'),
    })
    const nextRun = activeChatRunWithSyncedEventPatch(chatRun, putPatch, {
      id: 'event_3',
      runId: 'agent_run_1',
      sequence: 3,
      type: 'runner.completed',
      summary: 'Completed',
      status: 'completed',
      source: 'runner',
      payloadJson:
        '{"usage":{"provider":"openai","model":"gpt-5","totalTokens":5}}',
      artifactRefs: [],
      externalEventId: 'shc-event-3',
      createdAt: '2026-06-03T00:00:03.000Z',
    })

    expect(nextRun?.events.map(event => event.id)).toEqual([
      'event_1',
      'event_2',
      'event_3',
    ])
    expect(nextRun?.metadata.tokenTotal).toBe(27)
    expect(nextRun?.metadata.tokenUsageEvents).toBe(2)
    expect(nextRun?.metadata.eventCursor).toBe(3)
  })

  test('applies active run sync patches without replacing events', () => {
    const chatRun = activeChatRunFromResponse(runResponse)
    const runPatch = new SyncPatch({
      scope: SyncScope.make('agent-run:agent_run_1'),
      seq: SyncSequence.make(3),
      collection: CollectionName.make('agent_runs'),
      op: 'put',
      id: EntityId.make('agent_run_1'),
      value: {},
      serverTime: IsoTimestamp.make('2026-06-03T00:00:03.000Z'),
    })
    const nextRun = activeChatRunWithSyncedRunPatch(chatRun, runPatch, {
      ...runResponse.run,
      eventCursor: 3,
      status: 'completed',
      updatedAt: '2026-06-03T00:00:03.000Z',
    })

    expect(nextRun?.events).toEqual(chatRun.events)
    expect(nextRun?.metadata).toMatchObject({
      eventCursor: 3,
      status: 'completed',
      tokenTotal: 22,
      tokenUsageEvents: 1,
      updatedAt: '2026-06-03T00:00:03.000Z',
    })
  })

  test('projects failed runs as failed sidebar missions', () => {
    expect(
      sidebarMissionFromRunResponse({
        ...runResponse,
        run: {
          ...runResponse.run,
          status: 'failed',
          updatedAt: '2026-06-03T00:00:04.000Z',
        },
      }),
    ).toMatchObject({
      attention: false,
      owner: 'team',
      status: 'failed',
      teamId: 'team_openagents_core',
      updatedAt: '2026-06-03T00:00:04.000Z',
    })
  })

  test('projects project runs as team-owned project sidebar missions', () => {
    expect(
      sidebarMissionFromRunResponse({
        ...runResponse,
        run: {
          ...runResponse.run,
          projectId: 'project_artanis',
        },
      }),
    ).toMatchObject({
      owner: 'project',
      ownerUserId: 'github:14167547',
      projectId: 'project_artanis',
      teamId: 'team_openagents_core',
    })
  })

  test('normalizes legacy route run IDs and sync snapshot hrefs', () => {
    expect(displayRunId('agent_run_8a8f5061964845c8a21686a42f85cf12')).toBe(
      '8a8f5061-9648-45c8-a216-86a42f85cf12',
    )
    expect(syncSnapshotHref('thread:agent_run_1')).toBe(
      '/api/sync/thread/agent_run_1/snapshot',
    )
  })

  test('applies snapshots and patches without crossing scopes', () => {
    const snapshot = new SyncSnapshot({
      scope: SyncScope.make('workspace:team_openagents'),
      cursor: SyncSequence.make(1),
      collections: {
        missions: {
          mission_1: { title: 'First' },
        },
      },
    })
    const withSnapshot = syncWithSnapshot(
      emptySync,
      'workspace:team_openagents',
      snapshot,
    )
    const patch = new SyncPatch({
      scope: SyncScope.make('thread:agent_run_1'),
      seq: SyncSequence.make(2),
      collection: CollectionName.make('agent_runs'),
      op: 'put',
      id: EntityId.make('agent_run_1'),
      value: { id: 'agent_run_1', goal: 'Run the smoke test' },
      serverTime: IsoTimestamp.make('2026-06-03T00:00:02.000Z'),
      mutationId: MutationId.make('mutation_1'),
    })
    const withPatch = syncWithPatch(withSnapshot, patch)

    expect(
      withPatch.collectionByScope['workspace:team_openagents']?.missions
        ?.mission_1,
    ).toEqual({ title: 'First' })
    expect(
      withPatch.collectionByScope['thread:agent_run_1']?.agent_runs
        ?.agent_run_1,
    ).toEqual({ id: 'agent_run_1', goal: 'Run the smoke test' })
    expect(withPatch.cursors['thread:agent_run_1']).toBe(2)
    expect(withPatch.pendingMutations.mutation_1).toBeUndefined()
  })

  test('rebuilds run details from sync collections', () => {
    const response = agentRunResponseFromSyncCollections(
      {
        agent_runs: {
          agent_run_1: {
            ...runResponse.run,
            routeId: 'legacy-route-id',
          },
        },
        agent_run_events: {
          event_2: {
            ...runResponse.events[1],
            runId: 'agent_run_1',
            sequence: 2,
          },
          event_1: {
            ...runResponse.events[0],
            runId: 'agent_run_1',
            sequence: 1,
          },
        },
      },
      'legacy-route-id',
    )

    expect(response?.run.id).toBe('agent_run_1')
    expect(response?.events.map(event => event.id)).toEqual([
      'event_1',
      'event_2',
    ])
  })

  test('labels shared thread goal messages with the run owner, not the viewer', () => {
    expect(
      runAuthorLabel(
        runResponse,
        { name: 'Ben Salone', userId: 'github:ben' },
        [
          {
            id: 'team_openagents_core',
            members: [
              { name: 'Christopher David', userId: 'github:14167547' },
              { name: 'Ben Salone', userId: 'github:ben' },
            ],
          },
        ],
      ),
    ).toBe('Christopher David')
  })

  test('projects synced goal records into the goal panel DTO shape', () => {
    const goal = agentGoalFromSyncCollections(
      {
        agent_goals: {
          goal_1: {
            id: 'goal_1',
            agentId: 'autopilot',
            userId: 'github:14167547',
            teamId: null,
            projectId: null,
            objective: 'Keep shipping the goal UI.',
            status: 'complete',
            visibility: 'public',
            currentRunId: 'agent_run_1',
            tokenBudget: 1000,
            tokensUsed: 250,
            timeUsedSeconds: 60,
            remainingTokens: 750,
            createdAt: '2026-06-04T00:00:00.000Z',
            updatedAt: '2026-06-04T00:01:00.000Z',
            completedAt: '2026-06-04T00:01:00.000Z',
            pausedAt: null,
            blockedAt: null,
            canEdit: true,
            canPause: false,
            canResume: false,
            canMakePublic: false,
            publicUrl: '/api/public/goals/goal_1',
          },
        },
      },
      'autopilot:personal:room',
    )

    expect(goal).toMatchObject({
      id: 'goal_1',
      objective: 'Keep shipping the goal UI.',
      status: 'completed',
      remainingTokens: 750,
      visibility: 'public',
    })
  })
})
