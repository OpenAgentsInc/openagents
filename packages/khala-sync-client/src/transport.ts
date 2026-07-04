import {
  type BootstrapRequest,
  type BootstrapResponse,
  decodeBootstrapResponse,
  decodeLiveFrame,
  decodeLogPage,
  decodePushResponse,
  decodeSyncError,
  encodeBootstrapRequest,
  encodePushRequest,
  type LiveFrame,
  type LogPage,
  type PushRequest,
  type PushResponse,
  type SyncError,
  type SyncScope,
  type SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

/**
 * Transport seam for the Khala Sync client engine (KS-5.3; SPEC §3).
 *
 * The session state machine (session.ts) talks to the server exclusively
 * through {@link KhalaSyncTransport}. The production implementation
 * ({@link createHttpKhalaSyncTransport}) speaks the SPEC §3 routes over
 * fetch + WebSocket; tests inject a deterministic fake. Every boundary
 * value round-trips through the `@openagentsinc/khala-sync` codecs — raw
 * JSON never crosses this seam undecoded.
 */

// ---------------------------------------------------------------------------
// Routes (SPEC §3)
// ---------------------------------------------------------------------------

export const KHALA_SYNC_PUSH_PATH = "/api/sync/push"
export const KHALA_SYNC_BOOTSTRAP_PATH = "/api/sync/bootstrap"
export const KHALA_SYNC_LOG_PATH = "/api/sync/log"
export const KHALA_SYNC_CONNECT_PATH = "/api/sync/connect"

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export type KhalaSyncTransportErrorReason =
  /** The request never produced an HTTP response (fetch threw / socket refused). */
  | "network"
  /** Non-2xx HTTP status without a decodable in-band `SyncError` body. */
  | "http_status"
  /** The response body / frame failed to decode through the wire codecs. */
  | "decode_failure"
  /** The server answered with a typed in-band {@link SyncError}. */
  | "sync_error"

/**
 * Typed transport error. `retryable` drives the session's backoff-vs-fail
 * decision: network faults and 5xx/429/408 statuses retry; decode failures
 * never do; in-band `SyncError`s carry their own `retryable` bit.
 */
export class KhalaSyncTransportError extends Error {
  readonly _tag = "KhalaSyncTransportError"
  override readonly name = "KhalaSyncTransportError"
  constructor(
    readonly reason: KhalaSyncTransportErrorReason,
    readonly retryable: boolean,
    message: string,
    readonly details?: {
      readonly status?: number
      readonly syncError?: SyncError
      readonly cause?: unknown
    },
  ) {
    super(message, { cause: details?.cause })
  }
}

/** The session treats `cursor_behind_retained_window` as a refetch signal. */
export const isRefetchSignal = (error: unknown): boolean =>
  error instanceof KhalaSyncTransportError &&
  error.reason === "sync_error" &&
  error.details?.syncError?.code === "cursor_behind_retained_window"

export const isRetryableTransportError = (error: unknown): boolean =>
  error instanceof KhalaSyncTransportError && error.retryable

// ---------------------------------------------------------------------------
// Transport interface
// ---------------------------------------------------------------------------

export interface LiveSocketHandlers {
  /** One decoded {@link LiveFrame} per server message, in arrival order. */
  readonly onFrame: (frame: LiveFrame) => void
  /**
   * Fired exactly once when the connection ends for any reason after a
   * successful connect — server close, network fault, or an undecodable
   * frame (the transport closes the socket rather than skip frames).
   * NOT fired for a locally-initiated `close()`.
   */
  readonly onClose: (cause: {
    readonly error?: KhalaSyncTransportError
  }) => void
}

export interface LiveSocket {
  /** Close locally; suppresses the handlers' `onClose`. Idempotent. */
  readonly close: () => void
}

export interface KhalaSyncTransport {
  /** `POST /api/sync/bootstrap` — one snapshot page (SPEC §3). */
  readonly bootstrap: (
    request: BootstrapRequest,
  ) => Effect.Effect<BootstrapResponse, KhalaSyncTransportError>
  /** `GET /api/sync/log?scope&cursor&limit` — offset-resumable catch-up. */
  readonly logPage: (
    scope: SyncScope,
    cursor: SyncVersionWatermark,
    limit: number,
  ) => Effect.Effect<LogPage, KhalaSyncTransportError>
  /** `POST /api/sync/push` — durable mutation batch. */
  readonly push: (
    request: PushRequest,
  ) => Effect.Effect<PushResponse, KhalaSyncTransportError>
  /**
   * `WS /api/sync/connect?scope&cursor` — live tail. Resolves once the
   * socket is open; frames/closure flow through `handlers`.
   */
  readonly connectLive: (
    scope: SyncScope,
    cursor: SyncVersionWatermark,
    handlers: LiveSocketHandlers,
  ) => Effect.Effect<LiveSocket, KhalaSyncTransportError>
}

// ---------------------------------------------------------------------------
// HTTP + WebSocket implementation
// ---------------------------------------------------------------------------

/** Minimal WebSocket surface (browser / Bun compatible), injectable. */
export interface WebSocketLike {
  onopen: ((event: unknown) => void) | null
  onmessage: ((event: { readonly data: unknown }) => void) | null
  onerror: ((event: unknown) => void) | null
  onclose:
    | ((event: { readonly code?: number; readonly reason?: string }) => void)
    | null
  close(code?: number, reason?: string): void
}

export interface HttpTransportConfig {
  /** e.g. `https://openagents.com` — the SPEC §3 paths are appended. */
  readonly baseUrl: string
  /** Bearer for the `Authorization` header (and WS `token` query param). */
  readonly authToken: () => string
}

export interface HttpTransportDeps {
  readonly fetch?: typeof globalThis.fetch
  readonly webSocket?: new (url: string) => WebSocketLike
}

const retryableStatus = (status: number): boolean =>
  status >= 500 || status === 429 || status === 408

const toNetworkError = (cause: unknown): KhalaSyncTransportError =>
  cause instanceof KhalaSyncTransportError
    ? cause
    : new KhalaSyncTransportError(
        "network",
        true,
        `khala-sync request failed before a response arrived: ${
          cause instanceof Error ? cause.message : String(cause)
        }`,
        { cause },
      )

/**
 * Production transport: fetch + WebSocket against the SPEC §3 routes, all
 * payloads round-tripped through the khala-sync codecs, bearer auth from
 * `config.authToken()` (re-read per request, so token rotation is picked
 * up without a new transport). The WebSocket upgrade carries the token as
 * a `token` query parameter because browser WebSocket clients cannot set
 * an `Authorization` header.
 */
export const createHttpKhalaSyncTransport = (
  config: HttpTransportConfig,
  deps: HttpTransportDeps = {},
): KhalaSyncTransport => {
  const fetchImpl =
    deps.fetch ?? (globalThis.fetch.bind(globalThis) as typeof fetch)
  const WebSocketImpl =
    deps.webSocket ??
    (globalThis.WebSocket as unknown as new (url: string) => WebSocketLike)
  const base = config.baseUrl.replace(/\/+$/, "")

  /** Read the body once; surface an in-band SyncError when the server sent one. */
  const readJson = async (response: Response): Promise<unknown> => {
    const text = await response.text()
    let body: unknown = undefined
    try {
      body = text.length > 0 ? (JSON.parse(text) as unknown) : undefined
    } catch {
      body = undefined
    }
    if (response.ok) {
      if (body === undefined) {
        throw new KhalaSyncTransportError(
          "decode_failure",
          false,
          "khala-sync response body is not valid JSON",
        )
      }
      return body
    }
    if (body !== undefined) {
      try {
        const syncError = decodeSyncError(body)
        throw new KhalaSyncTransportError(
          "sync_error",
          syncError.retryable,
          `khala-sync server error ${syncError.code}: ${syncError.messageSafe}`,
          { status: response.status, syncError },
        )
      } catch (error) {
        if (error instanceof KhalaSyncTransportError) throw error
        // body was JSON but not a SyncError — fall through to http_status
      }
    }
    throw new KhalaSyncTransportError(
      "http_status",
      retryableStatus(response.status),
      `khala-sync request failed with HTTP ${response.status}`,
      { status: response.status },
    )
  }

  const decodeOr = <A>(decode: (input: unknown) => A, input: unknown): A => {
    try {
      return decode(input)
    } catch (cause) {
      throw new KhalaSyncTransportError(
        "decode_failure",
        false,
        "khala-sync response failed wire-codec decode",
        { cause },
      )
    }
  }

  const postJson = async <A>(
    path: string,
    body: unknown,
    decode: (input: unknown) => A,
  ): Promise<A> => {
    const response = await fetchImpl(`${base}${path}`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${config.authToken()}`,
      },
      body: JSON.stringify(body),
    })
    return decodeOr(decode, await readJson(response))
  }

  const run = <A>(
    request: () => Promise<A>,
  ): Effect.Effect<A, KhalaSyncTransportError> =>
    Effect.tryPromise({ try: request, catch: toNetworkError })

  return {
    bootstrap: (request) =>
      run(() =>
        postJson(
          KHALA_SYNC_BOOTSTRAP_PATH,
          encodeBootstrapRequest(request),
          decodeBootstrapResponse,
        ),
      ),
    push: (request) =>
      run(() =>
        postJson(
          KHALA_SYNC_PUSH_PATH,
          encodePushRequest(request),
          decodePushResponse,
        ),
      ),
    logPage: (scope, cursor, limit) =>
      run(async () => {
        const query = new URLSearchParams({
          scope,
          cursor: String(cursor),
          limit: String(limit),
        })
        const response = await fetchImpl(
          `${base}${KHALA_SYNC_LOG_PATH}?${query.toString()}`,
          {
            method: "GET",
            headers: { authorization: `Bearer ${config.authToken()}` },
          },
        )
        return decodeOr(decodeLogPage, await readJson(response))
      }),
    connectLive: (scope, cursor, handlers) =>
      run(
        () =>
          new Promise<LiveSocket>((resolve, reject) => {
            const wsBase = base.replace(/^http/, "ws")
            const query = new URLSearchParams({
              scope,
              cursor: String(cursor),
              token: config.authToken(),
            })
            let socket: WebSocketLike
            try {
              socket = new WebSocketImpl(
                `${wsBase}${KHALA_SYNC_CONNECT_PATH}?${query.toString()}`,
              )
            } catch (cause) {
              reject(toNetworkError(cause))
              return
            }
            let opened = false
            let done = false
            const finish = (error?: KhalaSyncTransportError): void => {
              if (done) return
              done = true
              if (!opened) {
                reject(
                  error ??
                    new KhalaSyncTransportError(
                      "network",
                      true,
                      "khala-sync live socket closed before opening",
                    ),
                )
                return
              }
              handlers.onClose(error === undefined ? {} : { error })
            }
            socket.onopen = () => {
              opened = true
              resolve({
                close: () => {
                  done = true // local close: suppress onClose
                  socket.close(1000, "khala_sync_client_close")
                },
              })
            }
            socket.onmessage = (event) => {
              if (done) return
              try {
                const frame = decodeOr(
                  decodeLiveFrame,
                  JSON.parse(String(event.data)) as unknown,
                )
                handlers.onFrame(frame)
              } catch (cause) {
                // Undecodable frame = protocol violation: never skip frames
                // silently — drop the connection and let the session resume
                // from the durable cursor.
                const error =
                  cause instanceof KhalaSyncTransportError
                    ? cause
                    : new KhalaSyncTransportError(
                        "decode_failure",
                        false,
                        "khala-sync live frame failed wire-codec decode",
                        { cause },
                      )
                finish(error)
                socket.close(1002, "khala_sync_frame_decode_failure")
              }
            }
            socket.onerror = () => {
              finish(
                new KhalaSyncTransportError(
                  "network",
                  true,
                  "khala-sync live socket errored",
                ),
              )
            }
            socket.onclose = () => {
              finish()
            }
          }),
      ),
  }
}
