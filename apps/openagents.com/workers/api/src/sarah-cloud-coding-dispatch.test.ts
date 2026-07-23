import type { PushResponse } from '@openagentsinc/khala-sync'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  SarahCloudCodingDispatchError,
  makeSarahCloudCodingDispatch,
  type SarahCloudCodingExecutePush,
} from './sarah-cloud-coding-dispatch'

const input = {
  commit: 'a'.repeat(40),
  harnessId: 'pi' as const,
  objective: 'Repair issue #9191 and run the pinned verification.',
  ownerUserId: 'owner.fixture',
  parentThreadRef: 'thread.sarah.fixture',
  repository: 'OpenAgentsInc/openagents',
  toolCallId: 'call.codex_workers_start',
  turnId: 'turn.sarah.fixture',
}

describe('Sarah cloud coding dispatch', () => {
  test('creates one owner-scoped repository-bound managed-cloud turn', async () => {
    const observed: Array<{
      name: string
      args: Readonly<Record<string, unknown>>
    }> = []
    const dispatch = makeSarahCloudCodingDispatch({
      executePush: ((request: Parameters<SarahCloudCodingExecutePush>[0]) => {
        for (const mutation of request.request.mutations) {
          observed.push({
            args: JSON.parse(mutation.argsJson) as Readonly<
              Record<string, unknown>
            >,
            name: mutation.name,
          })
        }
        return Promise.resolve({
          lastMutationId: 4,
          protocolVersion: 1,
          results: request.request.mutations.map(mutation => ({
            mutationId: mutation.mutationId,
            status: 'applied',
          })),
        } as PushResponse)
      }) as never,
      nowIso: () => '2026-07-23T03:00:00.000Z',
      sql: (() => Promise.resolve([])) as never,
    })

    const receipt = await Effect.runPromise(dispatch(input))

    expect(observed.map(entry => entry.name)).toEqual([
      'chat.createThread',
      'chat.bindThreadRepo',
      'chat.appendMessage',
      'runtime.startTurn',
    ])
    expect(observed[1]?.args).toMatchObject({
      repo: {
        defaultBranch: input.commit,
        name: 'openagents',
        owner: 'OpenAgentsInc',
      },
    })
    expect(observed[2]?.args).toMatchObject({
      body: input.objective,
    })
    expect(observed[3]?.args).toMatchObject({
      bodyRef: expect.stringMatching(/^chat_message\.message\.sarah_cloud\./u),
      redactionClass: 'private_ref',
      target: {
        adapterKind: 'openagents_native',
        executionTargetId: 'harness.pi',
        lane: 'managed_cloud',
      },
      visibility: 'private',
    })
    expect(receipt).toMatchObject({
      cloudTurnRef: expect.stringMatching(/^turn\.sarah_cloud\./u),
      dispatchRef: expect.stringMatching(/^dispatch\.sarah_cloud\./u),
      threadRef: expect.stringMatching(/^thread\.sarah_cloud\./u),
      workContextRef: expect.stringMatching(
        /^work_context\.thread\.thread\.sarah_cloud\./u,
      ),
    })
    expect(JSON.stringify(observed)).not.toContain(input.ownerUserId)
  })

  test('fails closed when any durable mutation is rejected', async () => {
    const dispatch = makeSarahCloudCodingDispatch({
      executePush: ((request: Parameters<SarahCloudCodingExecutePush>[0]) =>
        Promise.resolve({
          lastMutationId: 4,
          protocolVersion: 1,
          results: request.request.mutations.map((mutation, index) => ({
            ...(index === 3
              ? { errorCode: 'runtime_managed_cloud_repository_required' }
              : {}),
            mutationId: mutation.mutationId,
            status: index === 3 ? 'rejected' : 'applied',
          })),
        } as PushResponse)) as never,
      sql: (() => Promise.resolve([])) as never,
    })

    const failure = await Effect.runPromise(
      dispatch(input).pipe(Effect.flip),
    )

    expect(failure).toBeInstanceOf(SarahCloudCodingDispatchError)
    expect(failure.reason).toContain(
      'runtime_managed_cloud_repository_required',
    )
  })
})
