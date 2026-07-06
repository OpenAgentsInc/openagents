// khala-live-hub server E2E tests (CFG-5, #8520): the REAL Bun.serve
// surface — shared-bearer auth (header AND ?token=, the b45071b9b6
// query-token channel), append→log round trips, real WebSocket connects
// with catch-up + fan-out + access-changed broadcast, and the pre-upgrade
// HTTP error contract. These are the seam-tier transport assertions (epic
// #8506) against the LiveHub instead of the DO.

import { afterAll, beforeAll, describe, expect, test } from "bun:test"

import {
  LIVE_HUB_PING_TEXT,
  startLiveHubServer,
  type LiveHubServer,
} from "./index.js"

const TOKEN = "test-live-hub-token"
const SCOPE = "scope.thread.server-test"

let running: LiveHubServer
let base: string
let wsBase: string

beforeAll(() => {
  running = startLiveHubServer({ token: TOKEN, port: 0 })
  base = `http://127.0.0.1:${running.port}`
  wsBase = `ws://127.0.0.1:${running.port}`
})

afterAll(async () => {
  await running.stop()
})

const entryInput = (
  version: number,
  overrides: Partial<{ entityId: string; scope: string }> = {},
) => ({
  committedAt: "2026-07-06T00:00:00.000Z",
  entityId: overrides.entityId ?? `entity-${version}`,
  entityType: "thread",
  op: "upsert",
  postImageJson: `{"id":"entity-${version}","v":${version}}`,
  scope: overrides.scope ?? SCOPE,
  version,
})

const appendHttp = async (
  entries: Array<unknown>,
  scope = SCOPE,
  token = TOKEN,
) =>
  fetch(`${base}/append`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ entries, scope }),
  })

type OpenSocket = Readonly<{
  ws: WebSocket
  frames: Array<Record<string, unknown>>
  closed: Promise<{ code: number }>
  waitForFrames: (count: number, ms?: number) => Promise<void>
}>

const openSocket = async (
  query: string,
  timeoutMs = 5_000,
): Promise<OpenSocket> => {
  const ws = new WebSocket(`${wsBase}/connect?${query}`)
  const frames: Array<Record<string, unknown>> = []
  let closeResolve: (value: { code: number }) => void = () => {}
  const closed = new Promise<{ code: number }>((resolve) => {
    closeResolve = resolve
  })
  ws.addEventListener("message", (event) => {
    frames.push(JSON.parse(String(event.data)) as Record<string, unknown>)
  })
  ws.addEventListener("close", (event) => closeResolve({ code: event.code }))
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("websocket open timed out")),
      timeoutMs,
    )
    ws.addEventListener("open", () => {
      clearTimeout(timer)
      resolve()
    })
    ws.addEventListener("error", () => {
      clearTimeout(timer)
      reject(new Error("websocket errored before opening"))
    })
  })
  const waitForFrames = async (count: number, ms = 5_000): Promise<void> => {
    const deadline = Date.now() + ms
    while (frames.length < count) {
      if (Date.now() > deadline) {
        throw new Error(
          `expected ${count} frames, saw ${frames.length}: ${JSON.stringify(frames)}`,
        )
      }
      await new Promise((resolve) => setTimeout(resolve, 10))
    }
  }
  return { ws, frames, closed, waitForFrames }
}

describe("auth", () => {
  test("healthz needs no token", async () => {
    const response = await fetch(`${base}/healthz`)
    expect(response.status).toBe(200)
    expect((await response.json()) as Record<string, unknown>).toMatchObject({
      ok: true,
    })
  })

  test("every hub route 401s without the shared bearer", async () => {
    for (const [path, init] of [
      ["/append", { method: "POST" }],
      [`/log?scope=${SCOPE}`, {}],
      ["/access-changed", { method: "POST" }],
      [`/connect?scope=${SCOPE}`, {}],
    ] as const) {
      const response = await fetch(`${base}${path}`, init)
      expect(response.status).toBe(401)
    }
    const wrong = await appendHttp([entryInput(1)], SCOPE, "wrong-token")
    expect(wrong.status).toBe(401)
  })

  test("the bearer is accepted via the ?token= query parameter too", async () => {
    const response = await fetch(
      `${base}/log?scope=${SCOPE}&cursor=0&token=${TOKEN}`,
    )
    // Authenticated; empty window → the typed 410 behind-window SyncError.
    expect(response.status).toBe(410)
    expect((await response.json()) as Record<string, unknown>).toMatchObject({
      _tag: "SyncError",
      code: "cursor_behind_retained_window",
    })
  })
})

describe("append → log over HTTP", () => {
  test("round-trips a batch through the window with the DO response contract", async () => {
    const first = await appendHttp([entryInput(1), entryInput(2)])
    expect(first.status).toBe(200)
    expect((await first.json()) as Record<string, unknown>).toMatchObject({
      appended: 2,
      duplicates: 0,
      lastVersion: 2,
      ok: true,
      windowStartVersion: 1,
    })

    const page = await fetch(
      `${base}/log?scope=${SCOPE}&cursor=0&limit=100`,
      { headers: { authorization: `Bearer ${TOKEN}` } },
    )
    expect(page.status).toBe(200)
    const body = (await page.json()) as Record<string, unknown>
    expect(body).toMatchObject({
      nextCursor: 2,
      scope: SCOPE,
      upToDate: true,
    })
    expect(
      (body["entries"] as Array<{ version: number }>).map((e) => e.version),
    ).toEqual([1, 2])

    // Gap contract for capture: 409 + expectedFirstVersion.
    const gapped = await appendHttp([entryInput(9)])
    expect(gapped.status).toBe(409)
    expect((await gapped.json()) as Record<string, unknown>).toMatchObject({
      error: "khala_sync_hub_version_gap",
      expectedFirstVersion: 3,
    })
  })
})

describe("live websocket", () => {
  test("connect via ?token= catches up from the cursor and receives append fan-out", async () => {
    const scope = "scope.thread.server-ws"
    await appendHttp([entryInput(1, { scope }), entryInput(2, { scope })], scope)

    const socket = await openSocket(
      `scope=${scope}&cursor=1&token=${TOKEN}`,
    )
    await socket.waitForFrames(1)
    expect(socket.frames[0]).toMatchObject({
      _tag: "DeltaFrame",
      cursor: 2,
      scope,
    })

    // Fan-out on the next append.
    await appendHttp([entryInput(3, { scope })], scope)
    await socket.waitForFrames(2)
    expect(socket.frames[1]).toMatchObject({ _tag: "DeltaFrame", cursor: 3 })

    // PingFrame is answered in-band.
    socket.ws.send(JSON.stringify({ _tag: "PingFrame" }))
    await socket.waitForFrames(3)
    expect(JSON.stringify(socket.frames[2])).toBe(LIVE_HUB_PING_TEXT)

    socket.ws.close(1000)
  })

  test("access-changed broadcasts MustRefetch(access_changed) and closes the socket", async () => {
    const scope = "scope.thread.server-revoke"
    await appendHttp([entryInput(1, { scope })], scope)
    const socket = await openSocket(`scope=${scope}&cursor=1&token=${TOKEN}`)

    const response = await fetch(`${base}/access-changed`, {
      method: "POST",
      headers: {
        authorization: `Bearer ${TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({ scope }),
    })
    expect(response.status).toBe(200)
    expect((await response.json()) as Record<string, unknown>).toMatchObject({
      notified: 1,
      ok: true,
      scope,
    })

    await socket.waitForFrames(1)
    expect(socket.frames[0]).toMatchObject({
      _tag: "MustRefetchFrame",
      reason: "access_changed",
      scope,
    })
    const closed = await socket.closed
    expect(closed.code).toBe(1000)
  })

  test("a behind-window cursor gets MustRefetch(cursor_behind_retained_window) then close", async () => {
    const scope = "scope.thread.server-behind"
    // Mid-stream window start (fresh hub accepts any starting version).
    await appendHttp([entryInput(7, { scope }), entryInput(8, { scope })], scope)

    const socket = await openSocket(`scope=${scope}&cursor=2&token=${TOKEN}`)
    await socket.waitForFrames(1)
    expect(socket.frames[0]).toMatchObject({
      _tag: "MustRefetchFrame",
      reason: "cursor_behind_retained_window",
    })
    await socket.closed
  })

  test("pre-upgrade HTTP errors: 426 without upgrade, 400 on bad params", async () => {
    const plain = await fetch(`${base}/connect?scope=${SCOPE}&cursor=0`, {
      headers: { authorization: `Bearer ${TOKEN}` },
    })
    expect(plain.status).toBe(426)

    const badScope = await fetch(`${base}/connect?scope=nonsense&token=${TOKEN}`, {
      headers: { upgrade: "websocket" },
    })
    expect(badScope.status).toBe(400)

    const badCursor = await fetch(
      `${base}/connect?scope=${SCOPE}&cursor=abc&token=${TOKEN}`,
      { headers: { upgrade: "websocket" } },
    )
    expect(badCursor.status).toBe(400)
  })
})
