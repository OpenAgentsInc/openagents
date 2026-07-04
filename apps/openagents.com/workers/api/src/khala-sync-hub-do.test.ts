// KhalaSyncHubDO tests (KS-4.2, #8295).
//
// Drives the REAL Durable Object class over a `node:sqlite` database whose
// cursor reproduces Cloudflare's `SqlStorageCursor` semantics, plus fake
// hibernation sockets — the same real-storage idiom as
// `inference/durable-inference-real-do.test.ts`. The happy-path WebSocket
// upgrade (`WebSocketPair` + a 101 Response) is Workers-runtime-only (Node's
// Response rejects status 101), so socket behavior is tested through the
// REAL `attachSocket` accept/catch-up path and the pre-upgrade HTTP error
// paths of `/connect`.

import { describe, expect, test } from 'vitest'

import {
  KHALA_SYNC_HUB_PING_TEXT,
  KhalaSyncHubDO,
  handleKhalaSyncHubInternalRoute,
} from './khala-sync-hub-do'
// Real-SQLite DO state + fake hibernation sockets: shared with the KS-4.4
// stitch-seam suite (src/test/khala-sync-hub-do-harness.ts).
import {
  FakeWebSocket,
  makeHub,
  reopenHub,
} from './test/khala-sync-hub-do-harness'

const SCOPE = 'scope.thread.hub-test'
const OTHER_SCOPE = 'scope.thread.other'

const entryInput = (
  version: number,
  overrides: Partial<{
    entityId: string
    entityType: string
    op: string
    postImageJson: string
    scope: string
  }> = {},
) => ({
  committedAt: '2026-07-04T00:00:00.000Z',
  entityId: overrides.entityId ?? `entity-${version}`,
  entityType: overrides.entityType ?? 'thread',
  op: overrides.op ?? 'upsert',
  postImageJson:
    overrides.postImageJson ?? `{"id":"entity-${version}","v":${version}}`,
  scope: overrides.scope ?? SCOPE,
  version,
})

const appendRequest = (entries: Array<unknown>, scope = SCOPE) =>
  new Request('https://hub.internal/append', {
    body: JSON.stringify({ entries, scope }),
    method: 'POST',
  })

const append = async (
  hub: KhalaSyncHubDO,
  entries: Array<unknown>,
  scope = SCOPE,
) => {
  const response = await hub.fetch(appendRequest(entries, scope))
  return {
    body: (await response.json()) as Record<string, unknown>,
    status: response.status,
  }
}

const logRequest = (input: { cursor?: number; limit?: number; scope?: string }) => {
  const params = new URLSearchParams({ scope: input.scope ?? SCOPE })
  if (input.cursor !== undefined) params.set('cursor', String(input.cursor))
  if (input.limit !== undefined) params.set('limit', String(input.limit))
  return new Request(`https://hub.internal/log?${params.toString()}`)
}

const getLog = async (
  hub: KhalaSyncHubDO,
  input: { cursor?: number; limit?: number; scope?: string } = {},
) => {
  const response = await hub.fetch(logRequest(input))
  return {
    body: (await response.json()) as Record<string, unknown>,
    status: response.status,
  }
}

const scopeOf = (value: string) => value as never

// --------------------------------------------------------------------------
// /append
// --------------------------------------------------------------------------

describe('KhalaSyncHubDO /append', () => {
  test('appends a batch, then ignores a full replay (idempotent, at-least-once safe)', async () => {
    const { hub } = makeHub()
    const batch = [entryInput(1), entryInput(2), entryInput(3)]

    const first = await append(hub, batch)
    expect(first.status).toBe(200)
    expect(first.body).toMatchObject({
      appended: 3,
      duplicates: 0,
      lastVersion: 3,
      ok: true,
      windowStartVersion: 1,
    })

    const replay = await append(hub, batch)
    expect(replay.status).toBe(200)
    expect(replay.body).toMatchObject({
      appended: 0,
      duplicates: 3,
      lastVersion: 3,
      ok: true,
    })

    // Partial overlap: only the genuinely new suffix lands.
    const overlap = await append(hub, [entryInput(3), entryInput(4)])
    expect(overlap.body).toMatchObject({ appended: 1, duplicates: 1, lastVersion: 4 })
  })

  test('one version group may hold several entities (changelog PK shape)', async () => {
    const { hub } = makeHub()
    const result = await append(hub, [
      entryInput(1, { entityId: 'a' }),
      entryInput(1, { entityId: 'b' }),
      entryInput(2, { entityId: 'a' }),
    ])
    expect(result.status).toBe(200)
    expect(result.body).toMatchObject({ appended: 3, lastVersion: 2 })
  })

  test('rejects descending versions within a batch', async () => {
    const { hub } = makeHub()
    const result = await append(hub, [entryInput(2), entryInput(1)])
    expect(result.status).toBe(400)
    expect(result.body.error).toBe('khala_sync_hub_append_invalid')
  })

  test('rejects a gapped append against a non-empty window (dense-version invariant)', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])

    const gapped = await append(hub, [entryInput(4)])
    expect(gapped.status).toBe(409)
    expect(gapped.body).toMatchObject({
      error: 'khala_sync_hub_version_gap',
      expectedFirstVersion: 3,
      receivedFirstVersion: 4,
    })

    const internallyGapped = await append(hub, [entryInput(3), entryInput(5)])
    expect(internallyGapped.status).toBe(409)
    expect(internallyGapped.body.error).toBe('khala_sync_hub_version_gap')
  })

  test('rejects a scope mismatch (batch scope, entry scope, and pinned scope)', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1)])

    // Entry scope differs from the batch scope.
    const mixed = await append(hub, [entryInput(2, { scope: OTHER_SCOPE })])
    expect(mixed.status).toBe(409)
    expect(mixed.body.error).toBe('khala_sync_hub_scope_mismatch')

    // Batch scope differs from the pinned scope.
    const wrongHub = await append(
      hub,
      [entryInput(2, { scope: OTHER_SCOPE })],
      OTHER_SCOPE,
    )
    expect(wrongHub.status).toBe(409)
    expect(wrongHub.body.error).toBe('khala_sync_hub_scope_mismatch')
  })

  test('rejects undecodable bodies and empty batches', async () => {
    const { hub } = makeHub()
    expect((await append(hub, [])).status).toBe(400)
    expect(
      (await append(hub, [{ nonsense: true }])).status,
    ).toBe(400)
    const noScope = await hub.fetch(
      new Request('https://hub.internal/append', {
        body: JSON.stringify({ entries: [entryInput(1)] }),
        method: 'POST',
      }),
    )
    expect(noScope.status).toBe(400)
  })
})

// --------------------------------------------------------------------------
// Eviction
// --------------------------------------------------------------------------

describe('KhalaSyncHubDO window eviction', () => {
  test('evicts oldest version groups past the entry bound and advances window_start_version', async () => {
    const { hub } = makeHub({ KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES: '4' })

    for (let v = 1; v <= 6; v++) {
      const result = await append(hub, [entryInput(v)])
      expect(result.status).toBe(200)
    }

    const final = await append(hub, [entryInput(7)])
    expect(final.body).toMatchObject({
      lastVersion: 7,
      windowStartVersion: 4,
    })

    // Behind the window → typed behind-window error; inside → served.
    expect((await getLog(hub, { cursor: 0 })).status).toBe(410)
    expect((await getLog(hub, { cursor: 2 })).status).toBe(410)
    const inWindow = await getLog(hub, { cursor: 3 })
    expect(inWindow.status).toBe(200)
    expect(
      (inWindow.body.entries as Array<{ version: number }>).map(e => e.version),
    ).toEqual([4, 5, 6, 7])
  })

  test('evicts whole version groups, never splitting one', async () => {
    const { hub } = makeHub({ KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES: '3' })
    await append(hub, [
      entryInput(1, { entityId: 'a' }),
      entryInput(1, { entityId: 'b' }),
      entryInput(2, { entityId: 'a' }),
      entryInput(2, { entityId: 'b' }),
    ])
    // 4 entries > bound 3 → the ENTIRE v1 group goes; v2 stays whole.
    const page = await getLog(hub, { cursor: 1 })
    expect(page.status).toBe(200)
    expect(
      (page.body.entries as Array<{ version: number }>).map(e => e.version),
    ).toEqual([2, 2])
    expect((await getLog(hub, { cursor: 0 })).status).toBe(410)
  })

  test('byte bound evicts too, but the newest version group is always retained', async () => {
    const big = 'x'.repeat(512)
    const { hub } = makeHub({ KHALA_SYNC_HUB_WINDOW_MAX_BYTES: '600' })
    await append(hub, [entryInput(1, { postImageJson: `{"blob":"${big}"}` })])
    const second = await append(hub, [
      entryInput(2, { postImageJson: `{"blob":"${big}"}` }),
    ])
    // Both groups exceed 600 bytes together → v1 evicted; v2 alone exceeds
    // the bound but is the newest group, so it is retained.
    expect(second.body).toMatchObject({ lastVersion: 2, windowStartVersion: 2 })
    const page = await getLog(hub, { cursor: 1 })
    expect(page.status).toBe(200)
    expect((page.body.entries as Array<unknown>).length).toBe(1)
  })
})

// --------------------------------------------------------------------------
// /log
// --------------------------------------------------------------------------

describe('KhalaSyncHubDO /log', () => {
  test('serves a LogPage from the window with nextCursor and upToDate', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1), entryInput(2), entryInput(3)])

    const page = await getLog(hub, { cursor: 1 })
    expect(page.status).toBe(200)
    expect(page.body).toMatchObject({
      nextCursor: 3,
      protocolVersion: 1,
      scope: SCOPE,
      upToDate: true,
    })
    const entries = page.body.entries as Array<Record<string, unknown>>
    expect(entries.map(e => e.version)).toEqual([2, 3])
    expect(entries[0]).toMatchObject({
      entityId: 'entity-2',
      entityType: 'thread',
      op: 'upsert',
      scope: SCOPE,
    })
  })

  test('limit pages by whole version groups; a paged read walks to upToDate', async () => {
    const { hub } = makeHub()
    await append(hub, [
      entryInput(1, { entityId: 'a' }),
      entryInput(1, { entityId: 'b' }),
      entryInput(2, { entityId: 'a' }),
      entryInput(2, { entityId: 'b' }),
      entryInput(3, { entityId: 'a' }),
    ])

    // limit 3 admits v1 (2 entries) but NOT a split v2 → page ends at v1.
    const first = await getLog(hub, { cursor: 0, limit: 3 })
    expect(first.status).toBe(200)
    expect(
      (first.body.entries as Array<{ version: number }>).map(e => e.version),
    ).toEqual([1, 1])
    expect(first.body).toMatchObject({ nextCursor: 1, upToDate: false })

    const second = await getLog(hub, { cursor: 1, limit: 3 })
    expect(
      (second.body.entries as Array<{ version: number }>).map(e => e.version),
    ).toEqual([2, 2, 3])
    expect(second.body).toMatchObject({ nextCursor: 3, upToDate: true })

    // A single version group larger than the limit is served whole.
    const giant = await getLog(hub, { cursor: 0, limit: 1 })
    expect(
      (giant.body.entries as Array<{ version: number }>).map(e => e.version),
    ).toEqual([1, 1])
  })

  test('cursor at the edge returns an empty up-to-date page', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])
    const page = await getLog(hub, { cursor: 2 })
    expect(page.status).toBe(200)
    expect(page.body).toMatchObject({ nextCursor: 2, upToDate: true })
    expect(page.body.entries).toEqual([])
  })

  test('empty window returns the typed behind-window SyncError for ANY cursor (route falls through to Postgres)', async () => {
    const { hub } = makeHub()
    for (const cursor of [0, 1, 50]) {
      const result = await getLog(hub, { cursor })
      expect(result.status).toBe(410)
      expect(result.body).toMatchObject({
        _tag: 'SyncError',
        code: 'cursor_behind_retained_window',
        retryable: false,
      })
    }
  })

  test('cursor ahead of the window returns the retryable storage_unavailable SyncError (409)', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1)])
    const result = await getLog(hub, { cursor: 9 })
    expect(result.status).toBe(409)
    expect(result.body).toMatchObject({
      _tag: 'SyncError',
      code: 'storage_unavailable',
      retryable: true,
    })
  })

  test('rejects invalid scope/cursor/limit and mismatched scope', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1)])
    expect(
      (await hub.fetch(new Request('https://hub.internal/log?scope=nonsense')))
        .status,
    ).toBe(400)
    expect((await getLog(hub, { limit: 0 })).status).toBe(400)
    expect((await getLog(hub, { scope: OTHER_SCOPE })).status).toBe(409)
  })
})

// --------------------------------------------------------------------------
// Rehydrate after reset: the hub is cache/fan-out only
// --------------------------------------------------------------------------

describe('KhalaSyncHubDO rehydrate-after-reset', () => {
  test('a fresh DO starts empty and rebuilds from mid-stream appends (no business writes originate here)', async () => {
    // Fresh hub (as after a DO reset): /log serves nothing → route falls
    // through to Postgres.
    const { hub } = makeHub()
    expect((await getLog(hub, { cursor: 6 })).status).toBe(410)

    // Capture resumes from its checkpoint mid-stream (version 7): accepted
    // as the new window start.
    const result = await append(hub, [entryInput(7), entryInput(8)])
    expect(result.status).toBe(200)
    expect(result.body).toMatchObject({ lastVersion: 8, windowStartVersion: 7 })

    expect((await getLog(hub, { cursor: 3 })).status).toBe(410)
    const page = await getLog(hub, { cursor: 6 })
    expect(page.status).toBe(200)
    expect(
      (page.body.entries as Array<{ version: number }>).map(e => e.version),
    ).toEqual([7, 8])
  })

  test('window state survives an isolate restart (same storage, new instance)', async () => {
    const first = makeHub()
    await append(first.hub, [entryInput(1), entryInput(2)])

    const { hub: reopened } = reopenHub(first.db)
    const page = await getLog(reopened, { cursor: 0 })
    expect(page.status).toBe(200)
    expect(page.body).toMatchObject({ nextCursor: 2, upToDate: true })

    const appended = await append(reopened, [entryInput(3)])
    expect(appended.body).toMatchObject({ lastVersion: 3 })
  })
})

// --------------------------------------------------------------------------
// WebSocket connect + catch-up (via the real attachSocket path)
// --------------------------------------------------------------------------

describe('KhalaSyncHubDO connect catch-up', () => {
  test('a behind-in-window socket is caught up with one DeltaFrame per version group and its cursor advances', async () => {
    const { hub, sockets } = makeHub()
    await append(hub, [
      entryInput(1),
      entryInput(2, { entityId: 'a' }),
      entryInput(2, { entityId: 'b' }),
      entryInput(3),
    ])

    const ws = new FakeWebSocket()
    hub.attachSocket(ws, scopeOf(SCOPE), 1)

    expect(sockets).toContain(ws)
    const frames = ws.frames()
    expect(frames.map(f => f._tag)).toEqual(['DeltaFrame', 'DeltaFrame'])
    expect(frames[0]).toMatchObject({ cursor: 2, scope: SCOPE })
    expect((frames[0]!.entries as Array<unknown>).length).toBe(2)
    expect(frames[1]).toMatchObject({ cursor: 3 })
    expect(ws.cursor()).toBe(3)
  })

  test('an at-edge socket gets no frames on connect', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])
    const ws = new FakeWebSocket()
    hub.attachSocket(ws, scopeOf(SCOPE), 2)
    expect(ws.sent).toEqual([])
    expect(ws.cursor()).toBe(2)
    expect(ws.closed).toBeUndefined()
  })

  test('a behind-window socket gets MustRefetch(cursor_behind_retained_window) and is closed', async () => {
    const { hub } = makeHub({ KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES: '2' })
    for (let v = 1; v <= 5; v++) await append(hub, [entryInput(v)])

    const ws = new FakeWebSocket()
    hub.attachSocket(ws, scopeOf(SCOPE), 1)
    expect(ws.frames()).toEqual([
      {
        _tag: 'MustRefetchFrame',
        reason: 'cursor_behind_retained_window',
        scope: SCOPE,
      },
    ])
    expect(ws.closed).toBeDefined()
  })

  test('connecting to an EMPTY hub keeps the socket open and lets the first append decide', async () => {
    const { hub } = makeHub()
    const atEdge = new FakeWebSocket()
    const behind = new FakeWebSocket()
    hub.attachSocket(atEdge, scopeOf(SCOPE), 6)
    hub.attachSocket(behind, scopeOf(SCOPE), 2)
    expect(atEdge.sent).toEqual([])
    expect(behind.sent).toEqual([])

    // Capture resumes at version 7: the socket at cursor 6 is at the new
    // window edge; the socket at cursor 2 is behind the retained window.
    await append(hub, [entryInput(7)])

    expect(atEdge.frames().map(f => f._tag)).toEqual(['DeltaFrame'])
    expect(atEdge.cursor()).toBe(7)
    expect(behind.frames().map(f => f._tag)).toEqual(['MustRefetchFrame'])
    expect(behind.closed).toBeDefined()
  })

  test('/connect over HTTP rejects non-upgrade requests and invalid params before upgrading', async () => {
    const { hub } = makeHub()
    const plain = await hub.fetch(
      new Request(`https://hub.internal/connect?scope=${SCOPE}&cursor=0`),
    )
    expect(plain.status).toBe(426)

    const upgrade = (params: string) =>
      hub.fetch(
        new Request(`https://hub.internal/connect?${params}`, {
          headers: { Upgrade: 'websocket' },
        }),
      )
    expect((await upgrade('scope=nonsense&cursor=0')).status).toBe(400)
    expect((await upgrade(`scope=${SCOPE}&cursor=-2`)).status).toBe(400)
    expect((await upgrade(`scope=${SCOPE}&cursor=abc`)).status).toBe(400)

    await append(hub, [entryInput(1)])
    expect((await upgrade(`scope=${OTHER_SCOPE}&cursor=0`)).status).toBe(409)
  })
})

// --------------------------------------------------------------------------
// Append fan-out
// --------------------------------------------------------------------------

describe('KhalaSyncHubDO append fan-out', () => {
  test('at-edge sockets get exactly ONE DeltaFrame with the appended batch; cursors advance', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])

    const ws = new FakeWebSocket()
    hub.attachSocket(ws, scopeOf(SCOPE), 2)
    ws.sent.length = 0

    await append(hub, [
      entryInput(3, { entityId: 'a' }),
      entryInput(3, { entityId: 'b' }),
    ])

    const frames = ws.frames()
    expect(frames.map(f => f._tag)).toEqual(['DeltaFrame'])
    expect(frames[0]).toMatchObject({ cursor: 3, scope: SCOPE })
    expect((frames[0]!.entries as Array<unknown>).length).toBe(2)
    expect(ws.cursor()).toBe(3)

    // The NEXT append fans out again from the advanced cursor.
    await append(hub, [entryInput(4)])
    expect(ws.frames().map(f => f._tag)).toEqual(['DeltaFrame', 'DeltaFrame'])
    expect(ws.cursor()).toBe(4)
  })

  test('a socket behind the edge but inside the window is caught up in order', async () => {
    const { hub } = makeHub()
    await append(hub, [entryInput(1), entryInput(2), entryInput(3)])

    const ws = new FakeWebSocket()
    hub.attachSocket(ws, scopeOf(SCOPE), 3)
    ws.sent.length = 0
    // Simulate a socket that missed fan-outs (e.g. attachment from an older
    // edge): rewind its cursor, then append.
    ws.serializeAttachment({ cursor: 1 })

    await append(hub, [entryInput(4)])

    const frames = ws.frames()
    expect(frames.map(f => f._tag)).toEqual([
      'DeltaFrame',
      'DeltaFrame',
      'DeltaFrame',
    ])
    expect(frames.map(f => f.cursor)).toEqual([2, 3, 4])
    expect(ws.cursor()).toBe(4)
  })

  test('a socket pushed out of the window by eviction gets MustRefetch and is closed', async () => {
    const { hub } = makeHub({ KHALA_SYNC_HUB_WINDOW_MAX_ENTRIES: '2' })
    await append(hub, [entryInput(1), entryInput(2)])

    const ws = new FakeWebSocket()
    hub.attachSocket(ws, scopeOf(SCOPE), 2)
    ws.sent.length = 0
    ws.serializeAttachment({ cursor: 1 })

    // v3+v4 evict v1 and v2: cursor 1 now needs v2 which is gone.
    await append(hub, [entryInput(3), entryInput(4)])

    expect(ws.frames().map(f => f._tag)).toEqual(['MustRefetchFrame'])
    expect(ws.frames()[0]).toMatchObject({
      reason: 'cursor_behind_retained_window',
    })
    expect(ws.closed).toBeDefined()
  })

  test('a dead socket never poisons fan-out to healthy sockets', async () => {
    const { hub, sockets } = makeHub()
    await append(hub, [entryInput(1)])

    const dead = new FakeWebSocket()
    const healthy = new FakeWebSocket()
    hub.attachSocket(dead, scopeOf(SCOPE), 1)
    hub.attachSocket(healthy, scopeOf(SCOPE), 1)
    // Kill the socket underneath the hub (remote close not yet observed):
    // getWebSockets still returns it, but sends throw.
    dead.send = () => {
      throw new Error('broken pipe')
    }
    expect(sockets).toContain(dead)

    await append(hub, [entryInput(2)])

    expect(healthy.frames().map(f => f._tag)).toEqual(['DeltaFrame'])
    expect(healthy.cursor()).toBe(2)
  })
})

// --------------------------------------------------------------------------
// Hibernation keepalive
// --------------------------------------------------------------------------

describe('KhalaSyncHubDO ping/pong', () => {
  test('configures the hibernation auto-response ping pair when the runtime supports it', () => {
    class FakePair {
      constructor(
        readonly request: string,
        readonly response: string,
      ) {}
    }
    const globals = globalThis as { WebSocketRequestResponsePair?: unknown }
    const previous = globals.WebSocketRequestResponsePair
    globals.WebSocketRequestResponsePair = FakePair
    try {
      const { autoResponses } = makeHub()
      expect(autoResponses).toHaveLength(1)
      expect(autoResponses[0]).toMatchObject({
        request: KHALA_SYNC_HUB_PING_TEXT,
        response: KHALA_SYNC_HUB_PING_TEXT,
      })
    } finally {
      globals.WebSocketRequestResponsePair = previous
    }
  })

  test('webSocketMessage answers PingFrame and ignores everything else', () => {
    const { hub } = makeHub()
    const ws = new FakeWebSocket()
    hub.webSocketMessage(ws, JSON.stringify({ _tag: 'PingFrame' }))
    expect(ws.sent).toEqual([KHALA_SYNC_HUB_PING_TEXT])

    hub.webSocketMessage(ws, 'not json at all')
    hub.webSocketMessage(ws, JSON.stringify({ _tag: 'SomethingElse' }))
    hub.webSocketMessage(
      ws,
      JSON.stringify({
        _tag: 'MutationAckFrame',
        clientId: 'c1',
        lastMutationId: 1,
      }),
    )
    expect(ws.sent).toEqual([KHALA_SYNC_HUB_PING_TEXT])
  })
})

// --------------------------------------------------------------------------
// Internal worker route proxy (admin bearer guard, KS-0.2 style)
// --------------------------------------------------------------------------

describe('handleKhalaSyncHubInternalRoute', () => {
  const makeNamespace = () => {
    const fetched: Array<Request> = []
    const ids: Array<string> = []
    return {
      fetched,
      ids,
      namespace: {
        get: (_id: unknown) => ({
          fetch: (request: Request) => {
            fetched.push(request)
            return Promise.resolve(Response.json({ ok: true }))
          },
        }),
        idFromName: (name: string) => {
          ids.push(name)
          return { name }
        },
      },
    }
  }

  const routeRequest = (path: '/append' | '/connect' | '/log', scope = SCOPE) =>
    new Request(
      `https://openagents.com/api/internal/khala-sync/hub${path}?scope=${scope}&cursor=0`,
      path === '/append'
        ? { body: JSON.stringify({ entries: [entryInput(1)], scope }), method: 'POST' }
        : undefined,
    )

  test('requires the operator bearer', async () => {
    const { namespace } = makeNamespace()
    const response = await handleKhalaSyncHubInternalRoute(routeRequest('/log'), {
      doPath: '/log',
      namespace,
      requireOperator: () => Promise.resolve(false),
    })
    expect(response.status).toBe(401)
  })

  test('enforces the method per path', async () => {
    const { namespace } = makeNamespace()
    const wrongMethod = await handleKhalaSyncHubInternalRoute(
      routeRequest('/log'),
      {
        doPath: '/append',
        namespace,
        requireOperator: () => Promise.resolve(true),
      },
    )
    expect(wrongMethod.status).toBe(405)
  })

  test('rejects an invalid scope before touching the namespace', async () => {
    const { namespace, ids } = makeNamespace()
    const response = await handleKhalaSyncHubInternalRoute(
      new Request(
        'https://openagents.com/api/internal/khala-sync/hub/log?scope=not-a-scope',
      ),
      {
        doPath: '/log',
        namespace,
        requireOperator: () => Promise.resolve(true),
      },
    )
    expect(response.status).toBe(400)
    expect(ids).toEqual([])
  })

  test('reports an absent binding honestly (503) instead of an opaque crash', async () => {
    const response = await handleKhalaSyncHubInternalRoute(routeRequest('/log'), {
      doPath: '/log',
      namespace: undefined,
      requireOperator: () => Promise.resolve(true),
    })
    expect(response.status).toBe(503)
    const body = (await response.json()) as Record<string, unknown>
    expect(body.error).toBe('khala_sync_hub_binding_missing')
  })

  test('routes to idFromName(scope) and forwards method, query, and body to the DO path', async () => {
    const { namespace, fetched, ids } = makeNamespace()
    const response = await handleKhalaSyncHubInternalRoute(
      routeRequest('/append'),
      {
        doPath: '/append',
        namespace,
        requireOperator: () => Promise.resolve(true),
      },
    )
    expect(response.status).toBe(200)
    expect(ids).toEqual([SCOPE])
    expect(fetched).toHaveLength(1)
    const forwarded = fetched[0]!
    const url = new URL(forwarded.url)
    expect(url.pathname).toBe('/append')
    expect(url.searchParams.get('scope')).toBe(SCOPE)
    expect(forwarded.method).toBe('POST')
    const body = (await forwarded.json()) as Record<string, unknown>
    expect(body.scope).toBe(SCOPE)
  })
})

// --------------------------------------------------------------------------
// End-to-end through the REAL DO behind the proxy
// --------------------------------------------------------------------------

describe('internal route proxy driving the real DO', () => {
  test('append → log round-trips through per-scope namespace instances', async () => {
    const hubs = new Map<string, KhalaSyncHubDO>()
    const hubFor = (name: string): KhalaSyncHubDO => {
      let hub = hubs.get(name)
      if (hub === undefined) {
        hub = makeHub().hub
        hubs.set(name, hub)
      }
      return hub
    }
    const namespace = {
      get: (id: unknown) => ({
        fetch: (request: Request) =>
          hubFor((id as { name: string }).name).fetch(request),
      }),
      idFromName: (name: string) => ({ name }),
    }
    const deps = (doPath: '/append' | '/connect' | '/log') => ({
      doPath,
      namespace,
      requireOperator: () => Promise.resolve(true),
    })

    const appendResponse = await handleKhalaSyncHubInternalRoute(
      new Request(
        `https://openagents.com/api/internal/khala-sync/hub/append?scope=${SCOPE}`,
        {
          body: JSON.stringify({
            entries: [entryInput(1), entryInput(2)],
            scope: SCOPE,
          }),
          method: 'POST',
        },
      ),
      deps('/append'),
    )
    expect(appendResponse.status).toBe(200)

    const logResponse = await handleKhalaSyncHubInternalRoute(
      new Request(
        `https://openagents.com/api/internal/khala-sync/hub/log?scope=${SCOPE}&cursor=0`,
      ),
      deps('/log'),
    )
    expect(logResponse.status).toBe(200)
    const page = (await logResponse.json()) as Record<string, unknown>
    expect(page).toMatchObject({ nextCursor: 2, upToDate: true })
    expect((page.entries as Array<unknown>).length).toBe(2)

    // A different scope routes to a DIFFERENT (empty) hub instance.
    const otherLog = await handleKhalaSyncHubInternalRoute(
      new Request(
        `https://openagents.com/api/internal/khala-sync/hub/log?scope=${OTHER_SCOPE}&cursor=0`,
      ),
      deps('/log'),
    )
    expect(otherLog.status).toBe(410)
  })
})
