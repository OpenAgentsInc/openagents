import {
  CollectionName,
  EntityId,
  IsoTimestamp,
  MutationId,
  SyncPatch,
  SyncScope,
  SyncSequence,
} from '@openagentsinc/sync-schema'
import { describe, expect, test } from 'vitest'

import {
  applySyncPatch,
  collectionsForScope,
  cursorForScope,
  emptySyncClientState,
} from './index'

const patch = (
  input: Readonly<{
    collection?: string
    id?: string
    mutationId?: string
    op?: 'put' | 'patch' | 'delete' | 'invalidate'
    patch?: unknown
    scope: string
    seq: number
    value?: unknown
  }>,
): SyncPatch =>
  new SyncPatch({
    scope: SyncScope.make(input.scope),
    seq: SyncSequence.make(input.seq),
    collection: CollectionName.make(input.collection ?? 'agent_runs'),
    op: input.op ?? 'put',
    id: EntityId.make(input.id ?? 'agent_run_1'),
    value: input.value,
    patch: input.patch,
    serverTime: IsoTimestamp.make('2026-06-04T00:00:00.000Z'),
    ...(input.mutationId === undefined
      ? {}
      : { mutationId: MutationId.make(input.mutationId) }),
  })

describe('sync client state', () => {
  test('keeps same collection and id isolated by scope', () => {
    const workspaceState = applySyncPatch(
      emptySyncClientState,
      patch({
        scope: 'workspace:team_openagents',
        seq: 1,
        value: { goal: 'Workspace run' },
      }),
    )
    const threadState = applySyncPatch(
      workspaceState,
      patch({
        scope: 'thread:agent_run_1',
        seq: 2,
        value: { goal: 'Thread run' },
      }),
    )

    expect(
      threadState.collectionsByScope['workspace:team_openagents']?.agent_runs
        ?.agent_run_1,
    ).toEqual({ goal: 'Workspace run' })
    expect(
      threadState.collectionsByScope['thread:agent_run_1']?.agent_runs
        ?.agent_run_1,
    ).toEqual({ goal: 'Thread run' })
    expect(cursorForScope(threadState, 'workspace:team_openagents')).toBe(1)
    expect(cursorForScope(threadState, 'thread:agent_run_1')).toBe(2)
  })

  test('patches records within only the addressed scope', () => {
    const initialState = applySyncPatch(
      applySyncPatch(
        emptySyncClientState,
        patch({
          scope: 'workspace:team_openagents',
          seq: 1,
          value: { goal: 'Workspace run', status: 'queued' },
        }),
      ),
      patch({
        scope: 'thread:agent_run_1',
        seq: 1,
        value: { goal: 'Thread run', status: 'queued' },
      }),
    )
    const nextState = applySyncPatch(
      initialState,
      patch({
        scope: 'thread:agent_run_1',
        seq: 2,
        op: 'patch',
        patch: { status: 'running' },
      }),
    )

    expect(
      nextState.collectionsByScope['workspace:team_openagents']?.agent_runs
        ?.agent_run_1,
    ).toEqual({ goal: 'Workspace run', status: 'queued' })
    expect(
      nextState.collectionsByScope['thread:agent_run_1']?.agent_runs
        ?.agent_run_1,
    ).toEqual({ goal: 'Thread run', status: 'running' })
  })

  test('deletes records and clears matching pending mutations by scope patch', () => {
    const withRecord = applySyncPatch(
      {
        ...emptySyncClientState,
        pendingMutations: {
          mutation_1: {
            command: 'create',
            mutationId: 'mutation_1',
            payload: {},
            scope: 'thread:agent_run_1',
          },
        },
      },
      patch({
        mutationId: 'mutation_1',
        scope: 'thread:agent_run_1',
        seq: 1,
        value: { goal: 'Thread run' },
      }),
    )
    const withoutRecord = applySyncPatch(
      withRecord,
      patch({
        scope: 'thread:agent_run_1',
        seq: 2,
        op: 'delete',
      }),
    )

    expect(
      withoutRecord.collectionsByScope['thread:agent_run_1']?.agent_runs
        ?.agent_run_1,
    ).toBeUndefined()
    expect(withRecord.pendingMutations.mutation_1).toBeUndefined()
  })

  test('returns empty collections and cursor zero for missing scopes', () => {
    expect(collectionsForScope(emptySyncClientState, 'missing:scope')).toEqual(
      {},
    )
    expect(cursorForScope(emptySyncClientState, 'missing:scope')).toBe(0)
  })
})
