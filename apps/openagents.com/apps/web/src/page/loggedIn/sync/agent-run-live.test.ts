import {
  ChangelogEntry,
  EntityId as KhalaSyncEntityId,
  EntityType,
  SyncScope as KhalaSyncScope,
  SyncVersion,
} from '@openagentsinc/khala-sync'
import { describe, expect, test } from 'vitest'

import {
  activeChatRunWithSyncedEventPatch,
  activeChatRunWithSyncedRunPatch,
  syncWithPatch,
} from './projection'
import {
  AGENT_RUN_LIVE_EVENTS_COLLECTION,
  AGENT_RUN_LIVE_RUNS_COLLECTION,
  agentRunLiveMessagesFromLiveFramePayload,
  agentRunLivePatchFromChangelogEntry,
  agentRunLiveWireScope,
} from './agent-run-live'
import {
  ActiveChatRun,
  SyncClientModel,
  agentRunExternalRefFromNullable,
} from '../model'

const WIRE_SCOPE = 'scope.agent_run.agent_run_1'
const LEGACY_SCOPE = 'agent-run:agent_run_1'

const entry = (
  input: Readonly<{
    entityId: string
    entityType: string
    op: 'upsert' | 'delete'
    postImageJson?: string
    version: number
  }>,
): ChangelogEntry =>
  new ChangelogEntry({
    scope: KhalaSyncScope.make(WIRE_SCOPE),
    version: SyncVersion.make(input.version),
    entityType: EntityType.make(input.entityType),
    entityId: KhalaSyncEntityId.make(input.entityId),
    op: input.op,
    committedAt: '2026-07-05T00:00:00.000Z',
    ...(input.postImageJson === undefined
      ? {}
      : { postImageJson: input.postImageJson }),
  })

const agentRunPostImage = (
  overrides: Readonly<{ status?: string; updatedAt?: string }> = {},
): string =>
  JSON.stringify({
    runId: 'agent_run_1',
    routeId: 'agent_run_1',
    userId: 'github:14167547',
    teamId: 'team_openagents_core',
    projectId: null,
    runtime: 'opencode_codex',
    backend: 'shc_vm',
    status: overrides.status ?? 'running',
    goalId: null,
    goal: 'Run the smoke test',
    repository: {
      provider: 'github',
      owner: 'OpenAgentsInc',
      repo: 'autopilot-omega',
      ref: 'main',
    },
    createdAt: '2026-07-05T00:00:00.000Z',
    updatedAt: overrides.updatedAt ?? '2026-07-05T00:00:01.000Z',
    startedAt: null,
    completedAt: null,
    failedAt: null,
    canceledAt: null,
  })

const agentRunEventPostImage = (
  overrides: Readonly<{ id?: string; sequence?: number }> = {},
): string =>
  JSON.stringify({
    id: overrides.id ?? 'event_1',
    runId: 'agent_run_1',
    sequence: overrides.sequence ?? 1,
    type: 'tool_call',
    summary: 'Ran the test suite',
    status: 'complete',
    source: 'runner',
    payloadJson: null,
    artifactRefs: [],
    externalEventId: null,
    createdAt: '2026-07-05T00:00:02.000Z',
  })

const activeRunModel = ActiveChatRun({
  events: [],
  metadata: {
    backend: 'shc_vm',
    createdAt: '2026-06-03T00:00:00.000Z',
    eventCursor: 2,
    externalRunRef: agentRunExternalRefFromNullable(null),
    goal: 'Run tests',
    repository: 'OpenAgentsInc/autopilot-omega@main',
    runId: 'agent_run_1',
    displayRunId: 'agent_run_1',
    runnerId: 'oa-shc-katy-01',
    runtime: 'opencode_codex',
    status: 'queued',
    statusUrl: '/api/omni/agent-runs/agent_run_1',
    streamUrl: '/api/omni/agent-runs/agent_run_1/events',
    tokenTotal: 0,
    tokenUsageEvents: 0,
    updatedAt: '2026-06-03T00:00:00.000Z',
  },
})

describe('agentRunLiveWireScope', () => {
  test('builds the new engine dotted scope from a run id', () => {
    expect(agentRunLiveWireScope('agent_run_1')).toBe(WIRE_SCOPE)
  })
})

describe('agentRunLivePatchFromChangelogEntry', () => {
  test('adapts an agent_run upsert into an agent_runs PATCH (not a put)', () => {
    const patch = agentRunLivePatchFromChangelogEntry(
      entry({
        entityId: 'agent_run_1',
        entityType: 'agent_run',
        op: 'upsert',
        postImageJson: agentRunPostImage(),
        version: 5,
      }),
      LEGACY_SCOPE,
    )

    expect(patch).toBeDefined()
    expect(patch?.collection).toBe(AGENT_RUN_LIVE_RUNS_COLLECTION)
    expect(patch?.op).toBe('patch')
    expect(patch?.scope).toBe(LEGACY_SCOPE)
    expect(patch?.seq).toBe(5)
    expect(patch?.id).toBe('agent_run_1')
    expect(patch?.patch).toMatchObject({
      id: 'agent_run_1',
      status: 'running',
    })
    // Deliberately does NOT carry these — see the module doc.
    expect(patch?.patch).not.toHaveProperty('runnerId')
    expect(patch?.patch).not.toHaveProperty('eventCursor')
  })

  test('a patch round-trips through syncWithPatch preserving runnerId/eventCursor from the previously seeded legacy record', () => {
    const seeded = SyncClientModel({
      collectionByScope: {
        [LEGACY_SCOPE]: {
          agent_runs: {
            agent_run_1: {
              id: 'agent_run_1',
              runtime: 'opencode_codex',
              backend: 'shc_vm',
              runnerId: 'oa-shc-katy-01',
              userId: 'github:14167547',
              teamId: 'team_openagents_core',
              projectId: null,
              repository: {
                provider: 'github',
                owner: 'OpenAgentsInc',
                repo: 'autopilot-omega',
                ref: 'main',
              },
              goal: 'Run the smoke test',
              externalRunId: 'shc:oa-shc-katy-01:agent_run_1',
              status: 'queued',
              eventCursor: 2,
              createdAt: '2026-07-05T00:00:00.000Z',
              updatedAt: '2026-07-05T00:00:00.000Z',
            },
          },
        },
      },
      connectionByScope: {},
      cursors: { [LEGACY_SCOPE]: 1 },
      pendingMutations: {},
      workspaceScope: 'workspace:github:14167547',
    })

    const patch = agentRunLivePatchFromChangelogEntry(
      entry({
        entityId: 'agent_run_1',
        entityType: 'agent_run',
        op: 'upsert',
        postImageJson: agentRunPostImage({ status: 'running' }),
        version: 2,
      }),
      LEGACY_SCOPE,
    )!

    const nextSync = syncWithPatch(seeded, patch)
    const mergedRecord = nextSync.collectionByScope[LEGACY_SCOPE]?.agent_runs?.[
      'agent_run_1'
    ] as Record<string, unknown>

    // The new engine's fields applied...
    expect(mergedRecord.status).toBe('running')
    // ...and the legacy-only fields the new entity never carries survived
    // the merge untouched.
    expect(mergedRecord.runnerId).toBe('oa-shc-katy-01')
    expect(mergedRecord.eventCursor).toBe(2)
    expect(mergedRecord.externalRunId).toBe('shc:oa-shc-katy-01:agent_run_1')

    const nextChatRun = activeChatRunWithSyncedRunPatch(
      activeRunModel,
      patch,
      mergedRecord,
    )
    expect(nextChatRun?.metadata.status).toBe('running')
    expect(nextChatRun?.metadata.runnerId).toBe('oa-shc-katy-01')
  })

  test('adapts an agent_run_event upsert into an agent_run_events PUT that the legacy reducer accepts directly', () => {
    const patch = agentRunLivePatchFromChangelogEntry(
      entry({
        entityId: 'event_1',
        entityType: 'agent_run_event',
        op: 'upsert',
        postImageJson: agentRunEventPostImage(),
        version: 7,
      }),
      LEGACY_SCOPE,
    )!

    expect(patch.collection).toBe(AGENT_RUN_LIVE_EVENTS_COLLECTION)
    expect(patch.op).toBe('put')
    expect(patch.id).toBe('event_1')

    const nextChatRun = activeChatRunWithSyncedEventPatch(
      activeRunModel,
      patch,
      patch.value,
    )
    expect(nextChatRun?.events).toHaveLength(1)
    expect(nextChatRun?.events[0]).toMatchObject({
      id: 'event_1',
      summary: 'Ran the test suite',
    })
  })

  test('adapts a delete op with no value', () => {
    const patch = agentRunLivePatchFromChangelogEntry(
      entry({
        entityId: 'event_stale',
        entityType: 'agent_run_event',
        op: 'delete',
        version: 9,
      }),
      LEGACY_SCOPE,
    )

    expect(patch?.op).toBe('delete')
    expect(patch?.value).toBeUndefined()
  })

  test('returns undefined for an unrecognized entity type', () => {
    const patch = agentRunLivePatchFromChangelogEntry(
      entry({
        entityId: 'whatever',
        entityType: 'something_else',
        op: 'upsert',
        postImageJson: '{}',
        version: 1,
      }),
      LEGACY_SCOPE,
    )

    expect(patch).toBeUndefined()
  })

  test('returns undefined for an unparseable upsert post-image', () => {
    const patch = agentRunLivePatchFromChangelogEntry(
      entry({
        entityId: 'agent_run_1',
        entityType: 'agent_run',
        op: 'upsert',
        postImageJson: '{not json',
        version: 1,
      }),
      LEGACY_SCOPE,
    )

    expect(patch).toBeUndefined()
  })

  test('returns undefined when the post-image fails the entity contract decode', () => {
    const patch = agentRunLivePatchFromChangelogEntry(
      entry({
        entityId: 'agent_run_1',
        entityType: 'agent_run',
        op: 'upsert',
        postImageJson: JSON.stringify({ runId: 'agent_run_1' }),
        version: 1,
      }),
      LEGACY_SCOPE,
    )

    expect(patch).toBeUndefined()
  })
})

describe('agentRunLiveMessagesFromLiveFramePayload', () => {
  test('fans out a DeltaFrame with a run + event entry into two ReceivedSyncPatch messages', () => {
    const payload = JSON.stringify({
      _tag: 'DeltaFrame',
      scope: WIRE_SCOPE,
      cursor: 2,
      entries: [
        {
          scope: WIRE_SCOPE,
          version: 1,
          entityType: 'agent_run',
          entityId: 'agent_run_1',
          op: 'upsert',
          postImageJson: agentRunPostImage(),
          committedAt: '2026-07-05T00:00:00.000Z',
        },
        {
          scope: WIRE_SCOPE,
          version: 2,
          entityType: 'agent_run_event',
          entityId: 'event_1',
          op: 'upsert',
          postImageJson: agentRunEventPostImage(),
          committedAt: '2026-07-05T00:00:02.000Z',
        },
      ],
    })

    const messages = agentRunLiveMessagesFromLiveFramePayload(
      payload,
      LEGACY_SCOPE,
    )

    expect(messages).toHaveLength(2)
    expect(messages[0]).toMatchObject({
      _tag: 'ReceivedSyncPatch',
      patch: { collection: AGENT_RUN_LIVE_RUNS_COLLECTION },
    })
    expect(messages[1]).toMatchObject({
      _tag: 'ReceivedSyncPatch',
      patch: { collection: AGENT_RUN_LIVE_EVENTS_COLLECTION },
    })
  })

  test('ignores ping and mutation-ack frames', () => {
    expect(
      agentRunLiveMessagesFromLiveFramePayload(
        JSON.stringify({ _tag: 'PingFrame' }),
        LEGACY_SCOPE,
      ),
    ).toEqual([])
    expect(
      agentRunLiveMessagesFromLiveFramePayload(
        JSON.stringify({
          _tag: 'MutationAckFrame',
          clientId: 'client-1',
          lastMutationId: 1,
        }),
        LEGACY_SCOPE,
      ),
    ).toEqual([])
  })

  test('degrades a MustRefetchFrame to a scoped FailedSyncStream message', () => {
    const messages = agentRunLiveMessagesFromLiveFramePayload(
      JSON.stringify({
        _tag: 'MustRefetchFrame',
        scope: WIRE_SCOPE,
        reason: 'scope_reset',
      }),
      LEGACY_SCOPE,
    )

    expect(messages).toEqual([
      {
        _tag: 'FailedSyncStream',
        error: 'Agent run live-tail requested a refetch (scope_reset).',
        scope: LEGACY_SCOPE,
      },
    ])
  })

  test('returns a scoped FailedSyncStream message for an undecodable payload', () => {
    expect(
      agentRunLiveMessagesFromLiveFramePayload('{not json', LEGACY_SCOPE),
    ).toEqual([
      {
        _tag: 'FailedSyncStream',
        error: 'Agent run live-tail message could not be decoded.',
        scope: LEGACY_SCOPE,
      },
    ])
  })
})
