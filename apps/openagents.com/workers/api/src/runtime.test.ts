import { IsoTimestamp, OmniRunnerEvent } from '@openagentsinc/sync-schema'
import type { WorkerBindings } from '@openagentsinc/sync-worker'
import { Effect, Layer } from 'effect'
import { QueueBinding, WorkerEnvironment } from 'effect-cf'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsSyncRoomNotifications,
  OpenAgentsWorkerContext,
  OpenAgentsWorkerRequest,
  RunnerEventQueueMessage,
  RunnerEventsQueue,
  WorkerRequestLayer,
  makeOpenAgentsSyncRoomNotifications,
  makeOpenAgentsWorkerContext,
  scheduleBackgroundWork,
  syncScope,
} from './runtime'

const timestamp = IsoTimestamp.make('2026-06-04T00:00:00.000Z')

const durableObjectId = (name: string): DurableObjectId => ({
  equals: other => other.toString() === name,
  name,
  toString: () => name,
})

const makeSyncRoomNamespace = (
  onFetch: (scope: string, request: Request) => void,
): DurableObjectNamespace => {
  const stubForName = (name: string): DurableObjectStub =>
    ({
      fetch: async (input: Request | string | URL) => {
        const request = input instanceof Request ? input : new Request(input)

        onFetch(name, request)

        return new Response(null, { status: 204 })
      },
      id: durableObjectId(name),
      name,
    }) as DurableObjectStub
  const namespace: DurableObjectNamespace = {
    get: () => {
      throw new Error('Expected getByName to own sync room routing')
    },
    getByName: name => stubForName(name),
    idFromName: () => {
      throw new Error('Expected getByName to avoid Durable Object IDs')
    },
    idFromString: name => durableObjectId(name),
    jurisdiction: () => namespace,
    newUniqueId: () => durableObjectId('unique'),
  }

  return namespace
}

describe('OpenAgents Cloudflare runtime services', () => {
  test('encodes runner event queue payloads before enqueue', async () => {
    const sentBodies: Array<unknown> = []
    const queue: QueueBinding.QueueProducer<unknown> = {
      send: async body => {
        sentBodies.push(body)

        return {} as QueueBinding.QueueSendResponse
      },
    }
    const client = QueueBinding.makeClient({
      binding: 'RUNNER_EVENTS',
      message: RunnerEventQueueMessage,
    })(queue)

    await Effect.runPromise(
      RunnerEventsQueue.send(
        new RunnerEventQueueMessage({
          events: [
            new OmniRunnerEvent({
              createdAt: timestamp,
              payload: { ok: true },
              sequence: 1,
              source: 'runner',
              summary: 'runner started',
              type: 'runner.log',
            }),
          ],
          parentId: 'agent_run_1',
          receivedAt: timestamp,
          schemaVersion: 'openagents.runner_event_queue.v1',
        }),
      ).pipe(Effect.provide(Layer.succeed(RunnerEventsQueue, client))),
    )

    expect(sentBodies).toEqual([
      {
        events: [
          {
            createdAt: '2026-06-04T00:00:00.000Z',
            payload: { ok: true },
            sequence: 1,
            source: 'runner',
            summary: 'runner started',
            type: 'runner.log',
          },
        ],
        parentId: 'agent_run_1',
        receivedAt: '2026-06-04T00:00:00.000Z',
        schemaVersion: 'openagents.runner_event_queue.v1',
      },
    ])
  })

  test('routes typed sync scopes through the sync room notification service', async () => {
    const requestedRoomNames: Array<string> = []
    const notifiedScopes: Array<string> = []
    const namespace = makeSyncRoomNamespace((name, request) => {
      requestedRoomNames.push(name)
      notifiedScopes.push(
        request.headers.get('x-openagents-sync-scope') ?? name,
      )
    })
    const notifications = makeOpenAgentsSyncRoomNotifications(namespace)

    await Effect.runPromise(
      notifications.notifyScopes([
        syncScope('team:team_1'),
        syncScope('team:team_1'),
        syncScope('agent-run:run_1'),
      ]),
    )

    expect(requestedRoomNames).toEqual(['team:team_1', 'agent-run:run_1'])
    expect(notifiedScopes).toEqual(['team:team_1', 'agent-run:run_1'])
  })

  test('schedules background work through the Worker context boundary', async () => {
    const scheduled: Array<Promise<unknown>> = []
    const ctx = {
      waitUntil: (promise: Promise<unknown>) => {
        scheduled.push(promise)
      },
    }
    const service = makeOpenAgentsWorkerContext(ctx)
    const first = Promise.resolve('service')
    const second = Promise.resolve('helper')

    await Effect.runPromise(service.waitUntil(first))
    scheduleBackgroundWork(ctx, second)

    expect(scheduled).toEqual([first, second])
    await expect(Promise.all(scheduled)).resolves.toEqual(['service', 'helper'])
  })

  test('composes request-scoped Worker runtime services in one layer', async () => {
    const scheduled: Array<Promise<unknown>> = []
    const roomNames: Array<string> = []
    const syncRoom = makeSyncRoomNamespace((name, _request) => {
      roomNames.push(name)
    })
    const request = new Request('https://openagents.test/api/health')
    const ctx = {
      passThroughOnException: () => undefined,
      props: undefined,
      waitUntil: (promise: Promise<unknown>) => {
        scheduled.push(promise)
      },
    } satisfies ExecutionContext
    const env = {
      OPENAGENTS_DB: {} as D1Database,
      SYNC_ROOM: syncRoom,
    } as WorkerBindings

    await Effect.runPromise(
      Effect.gen(function* () {
        const requestContext = yield* OpenAgentsWorkerRequest
        const workerContext = yield* OpenAgentsWorkerContext
        const notifications = yield* OpenAgentsSyncRoomNotifications
        const workerEnv = yield* WorkerEnvironment

        expect(requestContext.request).toBe(request)
        expect(requestContext.ctx).toBe(ctx)
        expect(requestContext.url.pathname).toBe('/api/health')
        expect(workerEnv).toBe(env)

        yield* workerContext.waitUntil(Promise.resolve('layer'))
        yield* Effect.promise(() =>
          notifications
            .roomForScope(syncScope('team:team_1'))
            .fetch('https://sync.openagents.internal/stream'),
        )
      }).pipe(Effect.provide(WorkerRequestLayer({ ctx, env, request }))),
    )

    expect(scheduled).toHaveLength(1)
    expect(roomNames).toEqual(['team:team_1'])
  })
})
