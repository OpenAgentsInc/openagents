import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  ListSubscriberRecord,
  NativeListsServiceShape,
  SubscriberListRecord,
} from './native-lists'
import { makeNativeListsRoutes } from './native-lists-routes'

const fixtureNowIso = '2026-06-14T12:00:00.000Z'

type Bindings = Readonly<{ operatorToken?: string }>

const ctx = {
  waitUntil: () => {},
  passThroughOnException: () => {},
} as unknown as ExecutionContext

// In-memory store implementing the service shape so the route test exercises
// routing, public capture, idempotent replay, auth gating, and reads.
class MemoryNativeListsStore implements NativeListsServiceShape {
  readonly lists = new Map<string, SubscriberListRecord>()
  readonly subscribers = new Map<string, ListSubscriberRecord>()

  seedList(record: SubscriberListRecord): void {
    this.lists.set(record.id, record)
  }

  createList = async (input: Parameters<NativeListsServiceShape['createList']>[0]) => {
    const record: SubscriberListRecord = {
      id: `subscriber_list_${this.lists.size + 1}`,
      metadataJson: '{}',
      name: input.name,
      ownerUserId: input.ownerUserId ?? null,
      slug: input.slug ?? input.name.toLowerCase(),
      sourceAuthorityRef: input.sourceAuthorityRef,
      status: input.status ?? 'active',
      teamId: input.teamId ?? null,
    }
    this.lists.set(record.id, record)
    return record
  }

  addSubscriber = async (
    input: Parameters<NativeListsServiceShape['addSubscriber']>[0],
  ) => {
    const email = input.email.toLowerCase()
    const key = `list_subscriber:${input.listId}:${email}`
    const existing = this.subscribers.get(key)
    if (existing !== undefined) {
      return { idempotent: true, subscriber: existing }
    }
    const subscriber: ListSubscriberRecord = {
      email,
      id: `list_subscriber_${this.subscribers.size + 1}`,
      idempotencyKey: key,
      listId: input.listId,
      metadataJson: '{}',
      sourceRef: input.sourceRef,
      status: 'active',
    }
    this.subscribers.set(key, subscriber)
    return { idempotent: false, subscriber }
  }

  listSubscribers = async (
    input: Parameters<NativeListsServiceShape['listSubscribers']>[0],
  ) =>
    [...this.subscribers.values()].filter(
      subscriber =>
        subscriber.listId === input.listId &&
        (input.status === undefined || subscriber.status === input.status),
    )

  readList = async (listId: string) => this.lists.get(listId)

  unsubscribe = async (
    input: Parameters<NativeListsServiceShape['unsubscribe']>[0],
  ) => {
    const key = `list_subscriber:${input.listId}:${input.email.toLowerCase()}`
    const existing = this.subscribers.get(key)
    if (existing === undefined) {
      return undefined
    }
    const updated = { ...existing, status: 'unsubscribed' as const }
    this.subscribers.set(key, updated)
    return updated
  }
}

const activeListRecord = (id = 'subscriber_list_1'): SubscriberListRecord => ({
  id,
  metadataJson: '{}',
  name: 'Launch Waitlist',
  ownerUserId: 'github:owner',
  slug: 'launch-waitlist',
  sourceAuthorityRef: 'site.form.v1',
  status: 'active',
  teamId: null,
})

const makeRoutes = (store: MemoryNativeListsStore) =>
  makeNativeListsRoutes<Bindings>({
    makeStore: () => store,
    nowIso: () => fixtureNowIso,
    requireOperator: async (request, env) =>
      env.operatorToken !== undefined &&
      request.headers.get('authorization') === `Bearer ${env.operatorToken}`,
  })

const run = (effect: Effect.Effect<Response> | undefined) => {
  if (effect === undefined) {
    throw new Error('route did not match')
  }
  return Effect.runPromise(effect)
}

describe('native lists routes', () => {
  test('public lead capture creates subscriber, replay is idempotent', async () => {
    const store = new MemoryNativeListsStore()
    store.seedList(activeListRecord())
    const routes = makeRoutes(store)
    const env: Bindings = {}

    const create = await run(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/lists/subscriber_list_1/subscribers', {
          method: 'POST',
          body: JSON.stringify({ email: 'Lead@example.com', sourceRef: 'homepage' }),
        }),
        env,
        ctx,
      ),
    )
    expect(create.status).toBe(201)
    const createBody = (await create.json()) as {
      idempotent: boolean
      subscriber: { email: string; status: string }
    }
    expect(createBody.idempotent).toBe(false)
    expect(createBody.subscriber.email).toBe('lead@example.com')
    expect(createBody.subscriber.status).toBe('active')

    const replay = await run(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/lists/subscriber_list_1/subscribers', {
          method: 'POST',
          body: JSON.stringify({ email: 'lead@example.com' }),
        }),
        env,
        ctx,
      ),
    )
    expect(replay.status).toBe(200)
    const replayBody = (await replay.json()) as { idempotent: boolean }
    expect(replayBody.idempotent).toBe(true)
    expect(store.subscribers.size).toBe(1)
  })

  test('lead capture rejects invalid email', async () => {
    const store = new MemoryNativeListsStore()
    store.seedList(activeListRecord())
    const routes = makeRoutes(store)

    const response = await run(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/lists/subscriber_list_1/subscribers', {
          method: 'POST',
          body: JSON.stringify({ email: 'not-an-email' }),
        }),
        {},
        ctx,
      ),
    )
    expect(response.status).toBe(400)
  })

  test('lead capture 404s on unknown or inactive list', async () => {
    const store = new MemoryNativeListsStore()
    store.seedList({ ...activeListRecord(), status: 'paused' })
    const routes = makeRoutes(store)

    const response = await run(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/lists/subscriber_list_1/subscribers', {
          method: 'POST',
          body: JSON.stringify({ email: 'lead@example.com' }),
        }),
        {},
        ctx,
      ),
    )
    expect(response.status).toBe(404)
  })

  test('operator subscriber read requires auth and returns subscribers', async () => {
    const store = new MemoryNativeListsStore()
    store.seedList(activeListRecord())
    await store.addSubscriber({
      email: 'lead@example.com',
      listId: 'subscriber_list_1',
      sourceRef: 'homepage',
    })
    const routes = makeRoutes(store)
    const env: Bindings = { operatorToken: 'op-secret' }

    const forbidden = await run(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/lists/subscriber_list_1/subscribers'),
        env,
        ctx,
      ),
    )
    expect(forbidden.status).toBe(403)

    const ok = await run(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/lists/subscriber_list_1/subscribers', {
          headers: { authorization: 'Bearer op-secret' },
        }),
        env,
        ctx,
      ),
    )
    expect(ok.status).toBe(200)
    const body = (await ok.json()) as {
      subscribers: ReadonlyArray<{ email: string }>
    }
    expect(body.subscribers.map(s => s.email)).toEqual(['lead@example.com'])
  })

  test('operator list read returns list metadata behind auth', async () => {
    const store = new MemoryNativeListsStore()
    store.seedList(activeListRecord())
    const routes = makeRoutes(store)
    const env: Bindings = { operatorToken: 'op-secret' }

    const ok = await run(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/lists/subscriber_list_1', {
          headers: { authorization: 'Bearer op-secret' },
        }),
        env,
        ctx,
      ),
    )
    expect(ok.status).toBe(200)
    const body = (await ok.json()) as { list: { slug: string } }
    expect(body.list.slug).toBe('launch-waitlist')
  })

  test('unmatched path returns undefined', () => {
    const store = new MemoryNativeListsStore()
    const routes = makeRoutes(store)
    expect(
      routes.routeNativeListsRequest(
        new Request('https://openagents.com/api/other'),
        {},
        ctx,
      ),
    ).toBeUndefined()
  })
})
