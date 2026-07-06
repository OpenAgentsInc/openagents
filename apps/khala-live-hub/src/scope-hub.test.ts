// ScopeHub tests (CFG-5, #8520) — the KhalaSyncHubDO unit suite
// (workers/api/src/khala-sync-hub-do.test.ts) ported to the LiveHub core so
// the contract-parity claim is TEST-PROVEN: append idempotency/density,
// whole-version-group eviction, log paging + the 410/409 window errors,
// mid-stream rehydrate, attach catch-up, fan-out policy, and
// access-changed broadcast are byte-level ports of the DO cases.

import { describe, expect, test } from "bun:test"

import {
  LIVE_HUB_PING_TEXT,
  ScopeHub,
  type HubSocketLike,
  type ScopeHubBounds,
} from "./scope-hub.js"

const SCOPE = "scope.thread.hub-test"
const OTHER_SCOPE = "scope.thread.other"

const scopeOf = (value: string) => value as never

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
  committedAt: "2026-07-04T00:00:00.000Z",
  entityId: overrides.entityId ?? `entity-${version}`,
  entityType: overrides.entityType ?? "thread",
  op: overrides.op ?? "upsert",
  postImageJson:
    overrides.postImageJson ?? `{"id":"entity-${version}","v":${version}}`,
  scope: overrides.scope ?? SCOPE,
  version,
})

const makeHub = (bounds: ScopeHubBounds = {}, scope: string = SCOPE) =>
  new ScopeHub(scopeOf(scope), bounds)

const append = async (
  hub: ScopeHub,
  entries: Array<unknown>,
  scope = SCOPE,
) => {
  const response = hub.append({ entries, scope })
  return {
    body: (await response.json()) as Record<string, unknown>,
    status: response.status,
  }
}

const getLog = async (
  hub: ScopeHub,
  input: { cursor?: number; limit?: number; scope?: string } = {},
) => {
  const params = new URLSearchParams({ scope: input.scope ?? SCOPE })
  if (input.cursor !== undefined) params.set("cursor", String(input.cursor))
  if (input.limit !== undefined) params.set("limit", String(input.limit))
  const response = hub.log(params)
  return {
    body: (await response.json()) as Record<string, unknown>,
    status: response.status,
  }
}

class FakeSocket implements HubSocketLike {
  readonly sent: Array<string> = []
  closed:
    | { code?: number | undefined; reason?: string | undefined }
    | undefined

  send(message: string): void {
    if (this.closed !== undefined) throw new Error("socket closed")
    this.sent.push(message)
  }

  close(code?: number, reason?: string): void {
    this.closed = { code, reason }
  }

  frames(): Array<Record<string, unknown>> {
    return this.sent.map((text) => JSON.parse(text) as Record<string, unknown>)
  }
}

// --------------------------------------------------------------------------
// append
// --------------------------------------------------------------------------

describe("ScopeHub append", () => {
  test("appends a batch, then ignores a full replay (idempotent, at-least-once safe)", async () => {
    const hub = makeHub()
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
    expect(overlap.body).toMatchObject({
      appended: 1,
      duplicates: 1,
      lastVersion: 4,
    })
  })

  test("one version group may hold several entities (changelog PK shape)", async () => {
    const hub = makeHub()
    const result = await append(hub, [
      entryInput(1, { entityId: "a" }),
      entryInput(1, { entityId: "b" }),
      entryInput(2, { entityId: "a" }),
    ])
    expect(result.status).toBe(200)
    expect(result.body).toMatchObject({ appended: 3, lastVersion: 2 })
  })

  test("rejects descending versions within a batch", async () => {
    const hub = makeHub()
    const result = await append(hub, [entryInput(2), entryInput(1)])
    expect(result.status).toBe(400)
    expect(result.body["error"]).toBe("khala_sync_hub_append_invalid")
  })

  test("rejects a gapped append against a non-empty window (dense-version invariant)", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])

    const gapped = await append(hub, [entryInput(4)])
    expect(gapped.status).toBe(409)
    expect(gapped.body).toMatchObject({
      error: "khala_sync_hub_version_gap",
      expectedFirstVersion: 3,
      receivedFirstVersion: 4,
    })

    const internallyGapped = await append(hub, [entryInput(3), entryInput(5)])
    expect(internallyGapped.status).toBe(409)
    expect(internallyGapped.body["error"]).toBe("khala_sync_hub_version_gap")
  })

  test("rejects a scope mismatch (batch scope, entry scope, and hub scope)", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1)])

    // Entry scope differs from the batch scope.
    const mixed = await append(hub, [entryInput(2, { scope: OTHER_SCOPE })])
    expect(mixed.status).toBe(409)
    expect(mixed.body["error"]).toBe("khala_sync_hub_scope_mismatch")

    // Batch scope differs from the hub's own scope.
    const wrongHub = await append(
      hub,
      [entryInput(2, { scope: OTHER_SCOPE })],
      OTHER_SCOPE,
    )
    expect(wrongHub.status).toBe(409)
    expect(wrongHub.body["error"]).toBe("khala_sync_hub_scope_mismatch")
  })

  test("rejects undecodable bodies and empty batches", async () => {
    const hub = makeHub()
    expect((await append(hub, [])).status).toBe(400)
    expect((await append(hub, [{ nonsense: true }])).status).toBe(400)
    const noScope = hub.append({ entries: [entryInput(1)] })
    expect(noScope.status).toBe(400)
  })
})

// --------------------------------------------------------------------------
// Eviction
// --------------------------------------------------------------------------

describe("ScopeHub window eviction", () => {
  test("evicts oldest version groups past the entry bound and advances window_start_version", async () => {
    const hub = makeHub({ maxEntries: 4 })

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
      (inWindow.body["entries"] as Array<{ version: number }>).map(
        (e) => e.version,
      ),
    ).toEqual([4, 5, 6, 7])
  })

  test("evicts whole version groups, never splitting one", async () => {
    const hub = makeHub({ maxEntries: 3 })
    await append(hub, [
      entryInput(1, { entityId: "a" }),
      entryInput(1, { entityId: "b" }),
      entryInput(2, { entityId: "a" }),
      entryInput(2, { entityId: "b" }),
    ])
    // 4 entries > bound 3 → the ENTIRE v1 group goes; v2 stays whole.
    const page = await getLog(hub, { cursor: 1 })
    expect(page.status).toBe(200)
    expect(
      (page.body["entries"] as Array<{ version: number }>).map(
        (e) => e.version,
      ),
    ).toEqual([2, 2])
    expect((await getLog(hub, { cursor: 0 })).status).toBe(410)
  })

  test("byte bound evicts too, but the newest version group is always retained", async () => {
    const big = "x".repeat(512)
    const hub = makeHub({ maxBytes: 600 })
    await append(hub, [entryInput(1, { postImageJson: `{"blob":"${big}"}` })])
    const second = await append(hub, [
      entryInput(2, { postImageJson: `{"blob":"${big}"}` }),
    ])
    // Both groups exceed 600 bytes together → v1 evicted; v2 alone exceeds
    // the bound but is the newest group, so it is retained.
    expect(second.body).toMatchObject({
      lastVersion: 2,
      windowStartVersion: 2,
    })
    const page = await getLog(hub, { cursor: 1 })
    expect(page.status).toBe(200)
    expect((page.body["entries"] as Array<unknown>).length).toBe(1)
  })
})

// --------------------------------------------------------------------------
// log
// --------------------------------------------------------------------------

describe("ScopeHub log", () => {
  test("serves a LogPage from the window with nextCursor and upToDate", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1), entryInput(2), entryInput(3)])

    const page = await getLog(hub, { cursor: 1 })
    expect(page.status).toBe(200)
    expect(page.body).toMatchObject({
      nextCursor: 3,
      protocolVersion: 1,
      scope: SCOPE,
      upToDate: true,
    })
    const entries = page.body["entries"] as Array<Record<string, unknown>>
    expect(entries.map((e) => e["version"])).toEqual([2, 3])
    expect(entries[0]).toMatchObject({
      entityId: "entity-2",
      entityType: "thread",
      op: "upsert",
      scope: SCOPE,
    })
  })

  test("limit pages by whole version groups; a paged read walks to upToDate", async () => {
    const hub = makeHub()
    await append(hub, [
      entryInput(1, { entityId: "a" }),
      entryInput(1, { entityId: "b" }),
      entryInput(2, { entityId: "a" }),
      entryInput(2, { entityId: "b" }),
      entryInput(3, { entityId: "a" }),
    ])

    // limit 3 admits v1 (2 entries) but NOT a split v2 → page ends at v1.
    const first = await getLog(hub, { cursor: 0, limit: 3 })
    expect(first.status).toBe(200)
    expect(
      (first.body["entries"] as Array<{ version: number }>).map(
        (e) => e.version,
      ),
    ).toEqual([1, 1])
    expect(first.body).toMatchObject({ nextCursor: 1, upToDate: false })

    const second = await getLog(hub, { cursor: 1, limit: 3 })
    expect(
      (second.body["entries"] as Array<{ version: number }>).map(
        (e) => e.version,
      ),
    ).toEqual([2, 2, 3])
    expect(second.body).toMatchObject({ nextCursor: 3, upToDate: true })

    // A single version group larger than the limit is served whole.
    const giant = await getLog(hub, { cursor: 0, limit: 1 })
    expect(
      (giant.body["entries"] as Array<{ version: number }>).map(
        (e) => e.version,
      ),
    ).toEqual([1, 1])
  })

  test("cursor at the edge returns an empty up-to-date page", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])
    const page = await getLog(hub, { cursor: 2 })
    expect(page.status).toBe(200)
    expect(page.body).toMatchObject({ nextCursor: 2, upToDate: true })
    expect(page.body["entries"]).toEqual([])
  })

  test("empty window returns the typed behind-window SyncError for ANY cursor (route falls through to Postgres)", async () => {
    const hub = makeHub()
    for (const cursor of [0, 1, 50]) {
      const result = await getLog(hub, { cursor })
      expect(result.status).toBe(410)
      expect(result.body).toMatchObject({
        _tag: "SyncError",
        code: "cursor_behind_retained_window",
        retryable: false,
      })
    }
  })

  test("cursor ahead of the window returns the retryable storage_unavailable SyncError (409)", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1)])
    const result = await getLog(hub, { cursor: 9 })
    expect(result.status).toBe(409)
    expect(result.body).toMatchObject({
      _tag: "SyncError",
      code: "storage_unavailable",
      retryable: true,
    })
  })

  test("rejects invalid scope/cursor/limit and mismatched scope", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1)])
    expect(
      hub.log(new URLSearchParams({ scope: "nonsense" })).status,
    ).toBe(400)
    expect((await getLog(hub, { limit: 0 })).status).toBe(400)
    expect((await getLog(hub, { scope: OTHER_SCOPE })).status).toBe(409)
  })
})

// --------------------------------------------------------------------------
// Rehydrate after reset: the hub is cache/fan-out only
// --------------------------------------------------------------------------

describe("ScopeHub rehydrate-after-reset", () => {
  test("a fresh hub starts empty and rebuilds from mid-stream appends (no business writes originate here)", async () => {
    const hub = makeHub()
    expect((await getLog(hub, { cursor: 6 })).status).toBe(410)

    // Capture resumes from its checkpoint mid-stream (version 7): accepted
    // as the new window start.
    const result = await append(hub, [entryInput(7), entryInput(8)])
    expect(result.status).toBe(200)
    expect(result.body).toMatchObject({
      lastVersion: 8,
      windowStartVersion: 7,
    })

    expect((await getLog(hub, { cursor: 3 })).status).toBe(410)
    const page = await getLog(hub, { cursor: 6 })
    expect(page.status).toBe(200)
    expect(
      (page.body["entries"] as Array<{ version: number }>).map(
        (e) => e.version,
      ),
    ).toEqual([7, 8])
  })
})

// --------------------------------------------------------------------------
// Socket attach + catch-up
// --------------------------------------------------------------------------

describe("ScopeHub attach catch-up", () => {
  test("a behind-in-window socket is caught up with one DeltaFrame per version group and its cursor advances", async () => {
    const hub = makeHub()
    await append(hub, [
      entryInput(1),
      entryInput(2, { entityId: "a" }),
      entryInput(2, { entityId: "b" }),
      entryInput(3),
    ])

    const ws = new FakeSocket()
    hub.attachSocket(ws, 1)

    expect(hub.socketCount()).toBe(1)
    const frames = ws.frames()
    expect(frames.map((f) => f["_tag"])).toEqual(["DeltaFrame", "DeltaFrame"])
    expect(frames[0]).toMatchObject({ cursor: 2, scope: SCOPE })
    expect((frames[0]!["entries"] as Array<unknown>).length).toBe(2)
    expect(frames[1]).toMatchObject({ cursor: 3 })
    expect(hub.socketCursor(ws)).toBe(3)
  })

  test("an at-edge socket gets no frames on attach", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])
    const ws = new FakeSocket()
    hub.attachSocket(ws, 2)
    expect(ws.sent).toEqual([])
    expect(hub.socketCursor(ws)).toBe(2)
    expect(ws.closed).toBeUndefined()
  })

  test("a behind-window socket gets MustRefetch(cursor_behind_retained_window) and is closed", async () => {
    const hub = makeHub({ maxEntries: 2 })
    for (let v = 1; v <= 5; v++) await append(hub, [entryInput(v)])

    const ws = new FakeSocket()
    hub.attachSocket(ws, 1)
    expect(ws.frames()).toEqual([
      {
        _tag: "MustRefetchFrame",
        reason: "cursor_behind_retained_window",
        scope: SCOPE,
      },
    ])
    expect(ws.closed).toBeDefined()
    expect(hub.socketCount()).toBe(0)
  })

  test("attaching to an EMPTY hub keeps the socket open and lets the first append decide", async () => {
    const hub = makeHub()
    const atEdge = new FakeSocket()
    const behind = new FakeSocket()
    hub.attachSocket(atEdge, 6)
    hub.attachSocket(behind, 2)
    expect(atEdge.sent).toEqual([])
    expect(behind.sent).toEqual([])

    // Capture resumes at version 7: the socket at cursor 6 is at the new
    // window edge; the socket at cursor 2 is behind the retained window.
    await append(hub, [entryInput(7)])

    expect(atEdge.frames().map((f) => f["_tag"])).toEqual(["DeltaFrame"])
    expect(hub.socketCursor(atEdge)).toBe(7)
    expect(behind.frames().map((f) => f["_tag"])).toEqual(["MustRefetchFrame"])
    expect(behind.closed).toBeDefined()
  })
})

// --------------------------------------------------------------------------
// Append fan-out
// --------------------------------------------------------------------------

describe("ScopeHub append fan-out", () => {
  test("at-edge sockets get exactly ONE DeltaFrame with the appended batch; cursors advance", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])

    const ws = new FakeSocket()
    hub.attachSocket(ws, 2)
    ws.sent.length = 0

    await append(hub, [
      entryInput(3, { entityId: "a" }),
      entryInput(3, { entityId: "b" }),
    ])

    const frames = ws.frames()
    expect(frames.map((f) => f["_tag"])).toEqual(["DeltaFrame"])
    expect(frames[0]).toMatchObject({ cursor: 3, scope: SCOPE })
    expect((frames[0]!["entries"] as Array<unknown>).length).toBe(2)
    expect(hub.socketCursor(ws)).toBe(3)

    // The NEXT append fans out again from the advanced cursor.
    await append(hub, [entryInput(4)])
    expect(ws.frames().map((f) => f["_tag"])).toEqual([
      "DeltaFrame",
      "DeltaFrame",
    ])
    expect(hub.socketCursor(ws)).toBe(4)
  })

  test("a socket behind the edge but inside the window is caught up in order", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1), entryInput(2), entryInput(3)])

    const ws = new FakeSocket()
    hub.attachSocket(ws, 3)
    ws.sent.length = 0
    // Simulate a socket that missed fan-outs: rewind its cursor, then append.
    hub.setSocketCursor(ws, 1)

    await append(hub, [entryInput(4)])

    const frames = ws.frames()
    expect(frames.map((f) => f["_tag"])).toEqual([
      "DeltaFrame",
      "DeltaFrame",
      "DeltaFrame",
    ])
    expect(frames.map((f) => f["cursor"])).toEqual([2, 3, 4])
    expect(hub.socketCursor(ws)).toBe(4)
  })

  test("a socket pushed out of the window by eviction gets MustRefetch and is closed", async () => {
    const hub = makeHub({ maxEntries: 2 })
    await append(hub, [entryInput(1), entryInput(2)])

    const ws = new FakeSocket()
    hub.attachSocket(ws, 2)
    ws.sent.length = 0
    hub.setSocketCursor(ws, 1)

    // v3+v4 evict v1 and v2: cursor 1 now needs v2 which is gone.
    await append(hub, [entryInput(3), entryInput(4)])

    expect(ws.frames().map((f) => f["_tag"])).toEqual(["MustRefetchFrame"])
    expect(ws.frames()[0]).toMatchObject({
      reason: "cursor_behind_retained_window",
    })
    expect(ws.closed).toBeDefined()
  })

  test("a dead socket never poisons fan-out to healthy sockets", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1)])

    const dead = new FakeSocket()
    const healthy = new FakeSocket()
    hub.attachSocket(dead, 1)
    hub.attachSocket(healthy, 1)
    // Kill the socket underneath the hub (remote close not yet observed).
    dead.send = () => {
      throw new Error("broken pipe")
    }

    await append(hub, [entryInput(2)])

    expect(healthy.frames().map((f) => f["_tag"])).toEqual(["DeltaFrame"])
    expect(hub.socketCursor(healthy)).toBe(2)
    expect(hub.socketCount()).toBe(1)
  })
})

// --------------------------------------------------------------------------
// Ping + inbound frames
// --------------------------------------------------------------------------

describe("ScopeHub ping/pong", () => {
  test("onSocketMessage answers PingFrame and ignores everything else", () => {
    const hub = makeHub()
    const ws = new FakeSocket()
    hub.onSocketMessage(ws, JSON.stringify({ _tag: "PingFrame" }))
    expect(ws.sent).toEqual([LIVE_HUB_PING_TEXT])

    hub.onSocketMessage(ws, "not json at all")
    hub.onSocketMessage(ws, JSON.stringify({ _tag: "SomethingElse" }))
    hub.onSocketMessage(
      ws,
      JSON.stringify({
        _tag: "MutationAckFrame",
        clientId: "c1",
        lastMutationId: 1,
      }),
    )
    expect(ws.sent).toEqual([LIVE_HUB_PING_TEXT])
  })

  test("pingAll sends the keepalive to every attached socket and drops dead ones", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1)])
    const healthy = new FakeSocket()
    const dead = new FakeSocket()
    hub.attachSocket(healthy, 1)
    hub.attachSocket(dead, 1)
    dead.send = () => {
      throw new Error("broken pipe")
    }

    hub.pingAll()

    expect(healthy.sent).toEqual([LIVE_HUB_PING_TEXT])
    expect(hub.socketCount()).toBe(1)
  })
})

// --------------------------------------------------------------------------
// access-changed (KS-7.1)
// --------------------------------------------------------------------------

describe("ScopeHub access-changed", () => {
  test("broadcasts MustRefetch(access_changed) to EVERY socket and closes them all", async () => {
    const hub = makeHub()
    await append(hub, [entryInput(1), entryInput(2)])

    const a = new FakeSocket()
    const b = new FakeSocket()
    hub.attachSocket(a, 2)
    hub.attachSocket(b, 2)

    const notified = hub.accessChanged()
    expect(notified).toBe(2)
    for (const ws of [a, b]) {
      expect(ws.frames()).toEqual([
        { _tag: "MustRefetchFrame", reason: "access_changed", scope: SCOPE },
      ])
      expect(ws.closed).toBeDefined()
    }
    expect(hub.socketCount()).toBe(0)
  })

  test("an empty hub reports zero notified sockets", () => {
    const hub = makeHub()
    expect(hub.accessChanged()).toBe(0)
  })
})
