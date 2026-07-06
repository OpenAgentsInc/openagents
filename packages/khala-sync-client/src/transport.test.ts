import {
  BootstrapEntity,
  BootstrapRequest,
  BootstrapResponse,
  ClientGroupId,
  ClientId,
  CvrPullRequest,
  CvrPullResponse,
  CvrVersion,
  encodeLiveFrame,
  EntityId,
  EntityType,
  KHALA_SYNC_PROTOCOL_VERSION,
  LogPage,
  MutationEnvelope,
  MutationId,
  MutationResult,
  MutatorName,
  PingFrame,
  PushRequest,
  PushResponse,
  SyncSchemaVersion,
  SyncScope,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createHttpKhalaSyncTransport,
  KHALA_SYNC_BOOTSTRAP_PATH,
  KHALA_SYNC_CONNECT_PATH,
  KHALA_SYNC_CVR_PULL_PATH,
  KHALA_SYNC_LOG_PATH,
  KHALA_SYNC_PUSH_PATH,
  type WebSocketLike,
} from "./transport.js"

const scope = SyncScope.make("scope.team.alpha")
const schemaVersion = SyncSchemaVersion.make(1)
const clientGroupId = ClientGroupId.make("cg-alpha")
const clientId = ClientId.make("client-mobile-1")

interface FetchCall {
  readonly url: string
  readonly init: RequestInit
}

const jsonResponse = (body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  })

const headersOf = (init: RequestInit): Headers => new Headers(init.headers)

const requestBodyOf = (init: RequestInit): Record<string, unknown> =>
  JSON.parse(String(init.body)) as Record<string, unknown>

describe("createHttpKhalaSyncTransport wire shape", () => {
  test("constructs the SPEC HTTP routes with method, bearer, query params, and encoded bodies", async () => {
    const calls: Array<FetchCall> = []
    let token = "token-bootstrap"
    const fetchImpl = (async (
      input: string | URL | Request,
      init?: RequestInit,
    ) => {
      const url = String(input)
      calls.push({ url, init: init ?? {} })
      const pathname = new URL(url).pathname
      if (pathname === KHALA_SYNC_BOOTSTRAP_PATH) {
        return jsonResponse(
          new BootstrapResponse({
            protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
            scope,
            entities: [
              new BootstrapEntity({
                entityType: EntityType.make("thread"),
                entityId: EntityId.make("thread-1"),
                postImageJson: JSON.stringify({ title: "First thread" }),
              }),
            ],
            cursor: SyncVersionWatermark.make(11),
          }),
        )
      }
      if (pathname === KHALA_SYNC_PUSH_PATH) {
        return jsonResponse(
          new PushResponse({
            protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
            results: [
              new MutationResult({
                mutationId: MutationId.make(4),
                status: "applied",
              }),
            ],
            lastMutationId: 4,
          }),
        )
      }
      if (pathname === KHALA_SYNC_CVR_PULL_PATH) {
        return jsonResponse(
          new CvrPullResponse({
            protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
            scope,
            mode: "diff",
            puts: [],
            dels: [],
            cvrVersion: CvrVersion.make(3),
            cursor: SyncVersionWatermark.make(12),
          }),
        )
      }
      if (pathname === KHALA_SYNC_LOG_PATH) {
        return jsonResponse(
          new LogPage({
            protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
            scope,
            entries: [],
            nextCursor: SyncVersionWatermark.make(12),
            upToDate: true,
          }),
        )
      }
      throw new Error(`unexpected fetch path ${pathname}`)
    }) as typeof fetch

    const transport = createHttpKhalaSyncTransport(
      {
        baseUrl: "https://api.example.test/",
        authToken: () => token,
      },
      { fetch: fetchImpl },
    )

    await Effect.runPromise(
      transport.bootstrap(
        new BootstrapRequest({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          schemaVersion,
          scope,
          clientGroupId,
          pageSize: 25,
          pageToken: "page-2",
        }),
      ),
    )

    token = "token-push"
    await Effect.runPromise(
      transport.push(
        new PushRequest({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          schemaVersion,
          clientGroupId,
          clientId,
          mutations: [
            new MutationEnvelope({
              mutationId: MutationId.make(4),
              name: MutatorName.make("thread.rename"),
              argsJson: JSON.stringify({ threadId: "thread-1" }),
            }),
          ],
        }),
      ),
    )

    token = "token-cvr"
    await Effect.runPromise(
      transport.cvrPull!(
        new CvrPullRequest({
          protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
          schemaVersion,
          scope,
          clientGroupId,
          cvrVersion: CvrVersion.make(2),
          drift: [],
        }),
      ),
    )

    token = "token-log"
    await Effect.runPromise(
      transport.logPage(scope, SyncVersionWatermark.make(12), 50),
    )

    expect(calls).toHaveLength(4)
    expect(calls.map((call) => new URL(call.url).pathname)).toEqual([
      KHALA_SYNC_BOOTSTRAP_PATH,
      KHALA_SYNC_PUSH_PATH,
      KHALA_SYNC_CVR_PULL_PATH,
      KHALA_SYNC_LOG_PATH,
    ])

    const [bootstrap, push, cvrPull, logPage] = calls
    expect(bootstrap?.init.method).toBe("POST")
    expect(headersOf(bootstrap!.init).get("content-type")).toBe(
      "application/json",
    )
    expect(headersOf(bootstrap!.init).get("authorization")).toBe(
      "Bearer token-bootstrap",
    )
    expect(requestBodyOf(bootstrap!.init)).toMatchObject({
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      schemaVersion: 1,
      scope,
      clientGroupId,
      pageSize: 25,
      pageToken: "page-2",
    })

    expect(push?.init.method).toBe("POST")
    expect(headersOf(push!.init).get("authorization")).toBe("Bearer token-push")
    expect(requestBodyOf(push!.init)).toMatchObject({
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      schemaVersion: 1,
      clientGroupId,
      clientId,
    })
    expect(requestBodyOf(push!.init).mutations).toEqual([
      {
        mutationId: 4,
        name: "thread.rename",
        argsJson: JSON.stringify({ threadId: "thread-1" }),
      },
    ])

    expect(cvrPull?.init.method).toBe("POST")
    expect(headersOf(cvrPull!.init).get("authorization")).toBe(
      "Bearer token-cvr",
    )
    expect(requestBodyOf(cvrPull!.init)).toMatchObject({
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      schemaVersion: 1,
      scope,
      clientGroupId,
      cvrVersion: 2,
      drift: [],
    })

    const logUrl = new URL(logPage!.url)
    expect(logPage?.init.method).toBe("GET")
    expect(headersOf(logPage!.init).get("authorization")).toBe(
      "Bearer token-log",
    )
    expect(logUrl.searchParams.get("scope")).toBe(scope)
    expect(logUrl.searchParams.get("cursor")).toBe("12")
    expect(logUrl.searchParams.get("limit")).toBe("50")
  })

  test("connectLive carries cookie-less bearer auth in the WebSocket token query param", async () => {
    const sockets: Array<RecordingWebSocket> = []
    class RecordingWebSocket implements WebSocketLike {
      onopen: ((event: unknown) => void) | null = null
      onmessage: ((event: { readonly data: unknown }) => void) | null = null
      onerror: ((event: unknown) => void) | null = null
      onclose:
        | ((event: { readonly code?: number; readonly reason?: string }) => void)
        | null = null
      closeCode: number | undefined
      closeReason: string | undefined

      constructor(readonly url: string) {
        sockets.push(this)
      }

      close(code?: number, reason?: string): void {
        this.closeCode = code
        this.closeReason = reason
      }
    }

    const frames: Array<unknown> = []
    const closes: Array<unknown> = []
    const transport = createHttpKhalaSyncTransport(
      {
        baseUrl: "https://api.example.test",
        authToken: () => "mobile-bearer",
      },
      { webSocket: RecordingWebSocket },
    )

    const livePromise = Effect.runPromise(
      transport.connectLive(scope, SyncVersionWatermark.make(9), {
        onFrame: (frame) => frames.push(frame),
        onClose: (cause) => closes.push(cause),
      }),
    )

    expect(sockets).toHaveLength(1)
    const socket = sockets[0]!
    const url = new URL(socket.url)
    expect(url.protocol).toBe("wss:")
    expect(url.pathname).toBe(KHALA_SYNC_CONNECT_PATH)
    expect(url.searchParams.get("scope")).toBe(scope)
    expect(url.searchParams.get("cursor")).toBe("9")
    expect(url.searchParams.get("token")).toBe("mobile-bearer")

    socket.onopen?.({})
    const live = await livePromise
    socket.onmessage?.({
      data: JSON.stringify(encodeLiveFrame(new PingFrame({}))),
    })
    expect(frames).toMatchObject([{ _tag: "PingFrame" }])

    live.close()
    socket.onclose?.({ code: 1000, reason: "client close" })
    expect(socket.closeCode).toBe(1000)
    expect(socket.closeReason).toBe("khala_sync_client_close")
    expect(closes).toEqual([])
  })
})
