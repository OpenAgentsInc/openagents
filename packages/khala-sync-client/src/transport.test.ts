import {
  BootstrapRequest,
  type LiveFrame,
  ClientGroupId,
  ClientId,
  CvrPullRequest,
  encodeBootstrapRequest,
  encodeCvrPullRequest,
  encodeLiveFrame,
  encodePushRequest,
  DeltaFrame,
  PushRequest,
  SyncError,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createHttpKhalaSyncTransport,
  isAccessDeniedSignal,
  isRefetchSignal,
  isRetryableTransportError,
  KHALA_SYNC_BOOTSTRAP_PATH,
  KHALA_SYNC_CONNECT_PATH,
  KHALA_SYNC_CVR_PULL_PATH,
  KHALA_SYNC_LOG_PATH,
  KHALA_SYNC_PUSH_PATH,
  KhalaSyncTransportError,
  type LiveSocketHandlers,
  type WebSocketLike,
} from "./transport.js"

/**
 * ST-2 (#8508): direct unit coverage for the PRODUCTION transport —
 * `createHttpKhalaSyncTransport` — against a RECORDING fetch and a
 * RECORDING WebSocket, injected through the `HttpTransportDeps` seam the
 * transport already exposes. No fakes of the transport itself: the real
 * URL/query/header construction code runs.
 *
 * The two load-bearing assertions are the mirror image of the server-side
 * `withBearerFromQueryToken` tests in
 * `apps/openagents.com/workers/api/src/khala-sync-connect-routes.test.ts`:
 *
 *   1. `connectLive` carries the bearer in the `token` QUERY PARAM of the
 *      ws:// URL (WebSocket clients cannot set an `Authorization` header).
 *   2. Every HTTP call (`bootstrap`/`push`/`logPage`/`cvrPull`) carries the
 *      bearer in the `Authorization: Bearer` HEADER.
 *
 * Together with the server tests, these make the 2026-07-06 incident class
 * ("client puts the bearer in the query, server reads only the header")
 * impossible to reintroduce silently on either side. Incident audit:
 * docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md.
 */

// ---------------------------------------------------------------------------
// Recording fakes
// ---------------------------------------------------------------------------

interface RecordedFetchCall {
  readonly url: string
  readonly method: string
  readonly headers: Record<string, string>
  readonly body: unknown
}

const makeRecordingFetch = (
  responder: (url: string) => unknown,
): { readonly calls: Array<RecordedFetchCall>; readonly fetch: typeof fetch } => {
  const calls: Array<RecordedFetchCall> = []
  const fetchImpl = (async (input: unknown, init?: RequestInit) => {
    const url = String(input)
    calls.push({
      url,
      method: init?.method ?? "GET",
      headers: Object.fromEntries(
        Object.entries((init?.headers ?? {}) as Record<string, string>).map(
          ([key, value]) => [key.toLowerCase(), value],
        ),
      ),
      body:
        typeof init?.body === "string"
          ? (JSON.parse(init.body) as unknown)
          : undefined,
    })
    return new Response(JSON.stringify(responder(url)), {
      status: 200,
      headers: { "content-type": "application/json" },
    })
  }) as typeof fetch
  return { calls, fetch: fetchImpl }
}

class RecordingWebSocket implements WebSocketLike {
  static instances: Array<RecordingWebSocket> = []
  static reset(): void {
    RecordingWebSocket.instances = []
  }
  onopen: ((event: unknown) => void) | null = null
  onmessage: ((event: { readonly data: unknown }) => void) | null = null
  onerror: ((event: unknown) => void) | null = null
  onclose:
    | ((event: { readonly code?: number; readonly reason?: string }) => void)
    | null = null
  readonly closeCalls: Array<{ code: number | undefined; reason: string | undefined }> = []
  constructor(readonly url: string) {
    RecordingWebSocket.instances.push(this)
  }
  close(code?: number, reason?: string): void {
    this.closeCalls.push({ code, reason })
  }
}

const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const SCOPE = SyncScope.make("scope.user.user_test_1")
const CLIENT_GROUP = ClientGroupId.make("cg-transport-test")
const CLIENT = ClientId.make("client-transport-test")
const SCHEMA = SyncSchemaVersion.make(1)
const watermark = SyncVersionWatermark.make

const bootstrapRequest = new BootstrapRequest({
  protocolVersion: 1,
  schemaVersion: SCHEMA,
  scope: SCOPE,
  clientGroupId: CLIENT_GROUP,
})

const pushRequest = new PushRequest({
  protocolVersion: 1,
  schemaVersion: SCHEMA,
  clientGroupId: CLIENT_GROUP,
  clientId: CLIENT,
  mutations: [],
})

const cvrPullRequest = new CvrPullRequest({
  protocolVersion: 1,
  schemaVersion: SCHEMA,
  scope: SCOPE,
  clientGroupId: CLIENT_GROUP,
})

const responseFor = (url: string): unknown => {
  if (url.includes(KHALA_SYNC_BOOTSTRAP_PATH)) {
    return { protocolVersion: 1, scope: SCOPE, entities: [], cursor: 0 }
  }
  if (url.includes(KHALA_SYNC_PUSH_PATH)) {
    return { protocolVersion: 1, results: [], lastMutationId: 0 }
  }
  if (url.includes(KHALA_SYNC_CVR_PULL_PATH)) {
    return {
      protocolVersion: 1,
      scope: SCOPE,
      mode: "reset",
      puts: [],
      dels: [],
      cvrVersion: 1,
      cursor: 0,
    }
  }
  if (url.includes(KHALA_SYNC_LOG_PATH)) {
    return {
      protocolVersion: 1,
      scope: SCOPE,
      entries: [],
      nextCursor: 7,
      upToDate: true,
    }
  }
  throw new Error(`unexpected fetch url in test: ${url}`)
}

const noopHandlers: LiveSocketHandlers = {
  onFrame: () => {},
  onClose: () => {},
}

const makeTransport = (input: {
  readonly baseUrl?: string
  readonly authToken?: () => string
}) => {
  RecordingWebSocket.reset()
  const recording = makeRecordingFetch(responseFor)
  const transport = createHttpKhalaSyncTransport(
    {
      baseUrl: input.baseUrl ?? "https://openagents.com",
      authToken: input.authToken ?? (() => "token-1"),
    },
    { fetch: recording.fetch, webSocket: RecordingWebSocket },
  )
  return { transport, calls: recording.calls }
}

/** Start connectLive, open the recorded socket, and resolve the LiveSocket. */
const openLiveSocket = async (
  transport: ReturnType<typeof makeTransport>["transport"],
  handlers: LiveSocketHandlers = noopHandlers,
) => {
  const pending = Effect.runPromise(
    transport.connectLive(SCOPE, watermark(42), handlers),
  )
  await tick()
  const socket = RecordingWebSocket.instances.at(-1)
  if (socket === undefined) throw new Error("connectLive created no WebSocket")
  socket.onopen?.({})
  const live = await pending
  return { live, socket }
}

// ---------------------------------------------------------------------------
// SPEC §3 route constants
// ---------------------------------------------------------------------------

describe("khala-sync transport route constants (SPEC §3)", () => {
  test("path constants match the server routes exactly", () => {
    expect(KHALA_SYNC_PUSH_PATH).toBe("/api/sync/push")
    expect(KHALA_SYNC_BOOTSTRAP_PATH).toBe("/api/sync/bootstrap")
    expect(KHALA_SYNC_LOG_PATH).toBe("/api/sync/log")
    expect(KHALA_SYNC_CONNECT_PATH).toBe("/api/sync/connect")
    expect(KHALA_SYNC_CVR_PULL_PATH).toBe("/api/sync/cvr-pull")
  })
})

// ---------------------------------------------------------------------------
// HTTP calls: bearer in the Authorization HEADER
// ---------------------------------------------------------------------------

describe("createHttpKhalaSyncTransport HTTP calls", () => {
  test("bootstrap POSTs the encoded request with an Authorization: Bearer header", async () => {
    const { transport, calls } = makeTransport({})
    await Effect.runPromise(transport.bootstrap(bootstrapRequest))
    expect(calls).toHaveLength(1)
    const call = calls[0]!
    expect(call.url).toBe(`https://openagents.com${KHALA_SYNC_BOOTSTRAP_PATH}`)
    expect(call.method).toBe("POST")
    expect(call.headers["authorization"]).toBe("Bearer token-1")
    expect(call.headers["content-type"]).toBe("application/json")
    expect(call.body).toEqual(encodeBootstrapRequest(bootstrapRequest))
    // The bearer must NOT leak into the HTTP URL.
    expect(call.url).not.toContain("token-1")
  })

  test("push POSTs the encoded request with an Authorization: Bearer header", async () => {
    const { transport, calls } = makeTransport({})
    await Effect.runPromise(transport.push(pushRequest))
    const call = calls[0]!
    expect(call.url).toBe(`https://openagents.com${KHALA_SYNC_PUSH_PATH}`)
    expect(call.method).toBe("POST")
    expect(call.headers["authorization"]).toBe("Bearer token-1")
    expect(call.body).toEqual(encodePushRequest(pushRequest))
  })

  test("cvrPull POSTs the encoded request with an Authorization: Bearer header", async () => {
    const { transport, calls } = makeTransport({})
    await Effect.runPromise(transport.cvrPull!(cvrPullRequest))
    const call = calls[0]!
    expect(call.url).toBe(`https://openagents.com${KHALA_SYNC_CVR_PULL_PATH}`)
    expect(call.method).toBe("POST")
    expect(call.headers["authorization"]).toBe("Bearer token-1")
    expect(call.body).toEqual(encodeCvrPullRequest(cvrPullRequest))
  })

  test("logPage GETs with scope/cursor/limit query and an Authorization: Bearer header", async () => {
    const { transport, calls } = makeTransport({})
    const page = await Effect.runPromise(transport.logPage(SCOPE, watermark(5), 200))
    expect(page.nextCursor).toBe(watermark(7))
    const call = calls[0]!
    const url = new URL(call.url)
    expect(url.origin).toBe("https://openagents.com")
    expect(url.pathname).toBe(KHALA_SYNC_LOG_PATH)
    expect(url.searchParams.get("scope")).toBe(SCOPE)
    expect(url.searchParams.get("cursor")).toBe("5")
    expect(url.searchParams.get("limit")).toBe("200")
    expect(call.method).toBe("GET")
    expect(call.headers["authorization"]).toBe("Bearer token-1")
    // The bearer never rides the HTTP query string.
    expect(url.searchParams.get("token")).toBeNull()
  })

  test("authToken() is re-read per request so rotation is picked up", async () => {
    const tokens = ["first-token", "second-token"]
    const { transport, calls } = makeTransport({
      authToken: () => tokens.shift() ?? "exhausted",
    })
    await Effect.runPromise(transport.bootstrap(bootstrapRequest))
    await Effect.runPromise(transport.bootstrap(bootstrapRequest))
    expect(calls[0]!.headers["authorization"]).toBe("Bearer first-token")
    expect(calls[1]!.headers["authorization"]).toBe("Bearer second-token")
  })

  test("trailing slashes on baseUrl never double up in request URLs", async () => {
    const { transport, calls } = makeTransport({
      baseUrl: "https://openagents.com///",
    })
    await Effect.runPromise(transport.bootstrap(bootstrapRequest))
    expect(calls[0]!.url).toBe(
      `https://openagents.com${KHALA_SYNC_BOOTSTRAP_PATH}`,
    )
  })
})

// ---------------------------------------------------------------------------
// connectLive: bearer in the `token` QUERY PARAM of the ws(s):// URL
// ---------------------------------------------------------------------------

describe("createHttpKhalaSyncTransport connectLive", () => {
  test("puts the bearer in the token QUERY param, plus scope and cursor (the incident invariant)", async () => {
    const { transport } = makeTransport({})
    const { socket } = await openLiveSocket(transport)
    const url = new URL(socket.url)
    // The server MUST read this via withBearerFromQueryToken — WebSocket
    // clients cannot set an Authorization header on the upgrade request.
    expect(url.searchParams.get("token")).toBe("token-1")
    expect(url.searchParams.get("scope")).toBe(SCOPE)
    expect(url.searchParams.get("cursor")).toBe("42")
    expect(url.pathname).toBe(KHALA_SYNC_CONNECT_PATH)
  })

  test("rewrites https:// base to wss:// (and http:// to ws://)", async () => {
    const secure = makeTransport({ baseUrl: "https://openagents.com" })
    const { socket: secureSocket } = await openLiveSocket(secure.transport)
    expect(secureSocket.url.startsWith("wss://openagents.com/")).toBe(true)

    const local = makeTransport({ baseUrl: "http://127.0.0.1:8787" })
    const { socket: localSocket } = await openLiveSocket(local.transport)
    expect(localSocket.url.startsWith("ws://127.0.0.1:8787/")).toBe(true)
  })

  test("re-reads authToken() per connect so a rotated token reaches the query", async () => {
    const tokens = ["ws-token-a", "ws-token-b"]
    const { transport } = makeTransport({
      authToken: () => tokens.shift() ?? "exhausted",
    })
    const first = await openLiveSocket(transport)
    const second = await openLiveSocket(transport)
    expect(new URL(first.socket.url).searchParams.get("token")).toBe("ws-token-a")
    expect(new URL(second.socket.url).searchParams.get("token")).toBe("ws-token-b")
  })

  test("delivers decoded LiveFrames and suppresses onClose for a local close", async () => {
    const frames: Array<LiveFrame> = []
    let closes = 0
    const { transport } = makeTransport({})
    const { live, socket } = await openLiveSocket(transport, {
      onFrame: (frame) => frames.push(frame),
      onClose: () => {
        closes += 1
      },
    })
    const frame = new DeltaFrame({
      scope: SCOPE,
      entries: [],
      cursor: SyncVersion.make(43),
    })
    socket.onmessage?.({ data: JSON.stringify(encodeLiveFrame(frame)) })
    expect(frames).toHaveLength(1)
    expect(frames[0]!._tag).toBe("DeltaFrame")

    live.close()
    expect(socket.closeCalls).toEqual([
      { code: 1000, reason: "khala_sync_client_close" },
    ])
    socket.onclose?.({ code: 1000 })
    expect(closes).toBe(0)
  })

  test("classification predicates route the session's retry/refetch/park decisions", () => {
    const syncError = (code: "cursor_behind_retained_window" | "unauthorized_scope") =>
      new KhalaSyncTransportError("sync_error", false, `server error ${code}`, {
        status: code === "unauthorized_scope" ? 403 : 409,
        syncError: new SyncError({
          code,
          messageSafe: "test",
          retryable: false,
        }),
      })
    expect(isRefetchSignal(syncError("cursor_behind_retained_window"))).toBe(true)
    expect(isRefetchSignal(syncError("unauthorized_scope"))).toBe(false)
    expect(isAccessDeniedSignal(syncError("unauthorized_scope"))).toBe(true)
    expect(
      isAccessDeniedSignal(
        new KhalaSyncTransportError("http_status", false, "HTTP 403", {
          status: 403,
        }),
      ),
    ).toBe(true)
    // A 401 stays retryable (token rotation can heal it) — it must NOT park.
    expect(
      isAccessDeniedSignal(
        new KhalaSyncTransportError("http_status", false, "HTTP 401", {
          status: 401,
        }),
      ),
    ).toBe(false)
    expect(
      isRetryableTransportError(
        new KhalaSyncTransportError("network", true, "network fault"),
      ),
    ).toBe(true)
    expect(isRetryableTransportError(syncError("unauthorized_scope"))).toBe(false)
  })

  test("a socket that closes before opening rejects with a retryable network error", async () => {
    const { transport } = makeTransport({})
    const pending = Effect.runPromise(
      transport.connectLive(SCOPE, watermark(0), noopHandlers),
    )
    await tick()
    const socket = RecordingWebSocket.instances.at(-1)!
    socket.onclose?.({ code: 1006 })
    expect(pending).rejects.toThrow(KhalaSyncTransportError)
    await pending.catch((error: unknown) => {
      expect(error).toBeInstanceOf(KhalaSyncTransportError)
      expect((error as KhalaSyncTransportError).reason).toBe("network")
      expect((error as KhalaSyncTransportError).retryable).toBe(true)
    })
  })
})
