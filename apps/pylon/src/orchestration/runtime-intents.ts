import {
  decodeRuntimeControlIntentRow,
  type RuntimeControlIntentRow,
} from "@openagentsinc/khala-sync"

/**
 * Khala Sync runtime control-intent poller (#8388) — the Pylon-side typed
 * reader for the durable `runtime.*` control intents the runtime mutators
 * (`runtime.startTurn` / `appendUserMessage` / `interruptTurn` /
 * `continueTurn` / `retryTurn` / `closeTurn`) record in
 * `khala_sync_runtime_control_intents`. Mirrors `./fleet-intents.ts`
 * precisely.
 *
 * Pragmatic v1 transport: the admin-guarded Worker route
 * `GET /api/internal/khala-sync/runtime-intents?ownerUserId=&after=&limit=`,
 * polled with the supervisor's `OPENAGENTS_ADMIN_API_TOKEN` bearer against
 * `OPENAGENTS_BASE_URL`. The route pages oldest-first by `seq` and returns
 * a `nextAfter` watermark; callers persist that watermark (see
 * `PylonOrchestrationStore.getRuntimeIntentWatermark`/
 * `setRuntimeIntentWatermark` in `./store.ts`) and resume from it.
 */

export const RUNTIME_INTENTS_ROUTE_PATH = "/api/internal/khala-sync/runtime-intents"
export const CHAT_MESSAGE_READ_ROUTE_PATH = "/api/internal/khala-sync/chat-message"

export type { RuntimeControlIntentRow } from "@openagentsinc/khala-sync"

export interface ReadPendingRuntimeIntentsOptions {
  /** e.g. `https://openagents.com` (`OPENAGENTS_BASE_URL`). */
  readonly baseUrl: string
  /** Admin bearer (`OPENAGENTS_ADMIN_API_TOKEN`). */
  readonly adminToken: string
  /** Resume watermark: only intents with `seq > after`. Default 0. */
  readonly after?: number
  /** Restrict to one owner's intents. */
  readonly ownerUserId?: string
  /** Page size (the route clamps to its own maximum). */
  readonly limit?: number
  /** Test seam. Default `globalThis.fetch`. */
  readonly fetchImpl?: typeof globalThis.fetch
}

export type ReadPendingRuntimeIntentsResult =
  | Readonly<{
      ok: true
      intents: ReadonlyArray<RuntimeControlIntentRow>
      /** Persist this and pass it back as `after` on the next poll. */
      nextAfter: number
      /** True when the page was not truncated — the caller is caught up. */
      upToDate: boolean
    }>
  | Readonly<{
      ok: false
      /** Bounded, token-free failure classification. */
      error:
        | "unauthorized"
        | "invalid_request"
        | "storage_unavailable"
        | "not_enabled"
        | "bad_response"
        | "network_failed"
      status: number | null
      /** Public-safe detail (never echoes tokens or connection strings). */
      reason: string | null
    }>

const boundedReason = (value: unknown): string | null => {
  if (typeof value !== "string" || value.length === 0) return null
  return value.slice(0, 300)
}

/**
 * Poll the Worker's runtime-intents route once. Never throws: transport and
 * contract failures come back as typed `ok: false` results so a dispatch
 * consumer loop can log and retry on its own cadence.
 */
export const readPendingRuntimeIntents = async (
  options: ReadPendingRuntimeIntentsOptions,
): Promise<ReadPendingRuntimeIntentsResult> => {
  const url = new URL(RUNTIME_INTENTS_ROUTE_PATH, options.baseUrl)
  if (options.ownerUserId !== undefined) url.searchParams.set("ownerUserId", options.ownerUserId)
  if (options.after !== undefined) {
    url.searchParams.set("after", String(Math.max(0, Math.floor(options.after))))
  }
  if (options.limit !== undefined) {
    url.searchParams.set("limit", String(Math.max(1, Math.floor(options.limit))))
  }

  const fetchImpl = options.fetchImpl ?? globalThis.fetch
  let response: Response
  try {
    response = await fetchImpl(url.toString(), {
      headers: { Authorization: `Bearer ${options.adminToken}` },
      method: "GET",
    })
  } catch (error) {
    return {
      error: "network_failed",
      ok: false,
      reason: boundedReason(error instanceof Error ? error.message : null),
      status: null,
    }
  }

  if (response.status === 401) {
    return { error: "unauthorized", ok: false, reason: null, status: 401 }
  }
  if (response.status === 400) {
    return { error: "invalid_request", ok: false, reason: null, status: 400 }
  }
  if (response.status === 503) {
    return { error: "storage_unavailable", ok: false, reason: null, status: 503 }
  }
  if (response.status !== 200) {
    return {
      error: "bad_response",
      ok: false,
      reason: `unexpected status ${response.status}`,
      status: response.status,
    }
  }

  let body: unknown
  try {
    body = await response.json()
  } catch {
    return { error: "bad_response", ok: false, reason: "response body was not JSON", status: 200 }
  }

  const record = body as {
    ok?: unknown
    reason?: unknown
    intents?: unknown
    nextAfter?: unknown
    upToDate?: unknown
  }
  if (record.ok !== true) {
    return {
      error: "not_enabled",
      ok: false,
      reason: boundedReason(record.reason),
      status: 200,
    }
  }
  if (!Array.isArray(record.intents) || typeof record.nextAfter !== "number") {
    return {
      error: "bad_response",
      ok: false,
      reason: "response shape did not match the runtime-intents contract",
      status: 200,
    }
  }
  let intents: Array<RuntimeControlIntentRow>
  try {
    intents = record.intents.map((row) => decodeRuntimeControlIntentRow(row))
  } catch {
    return {
      error: "bad_response",
      ok: false,
      reason: "an intent row failed to decode against RuntimeControlIntentRow",
      status: 200,
    }
  }
  return {
    intents,
    nextAfter: record.nextAfter,
    ok: true,
    upToDate: record.upToDate === true,
  }
}

export type FetchChatMessageOptions = {
  readonly baseUrl: string
  readonly adminToken: string
  readonly threadId: string
  readonly messageId: string
  readonly fetchImpl?: typeof globalThis.fetch
}

export type ChatMessageBody = {
  readonly messageId: string
  readonly threadId: string
  readonly authorUserId: string
  readonly body: string
  readonly createdAt: string
  readonly updatedAt: string
  readonly deletedAt: string | null
}

export type FetchChatMessageResult =
  | Readonly<{ ok: true; message: ChatMessageBody | null }>
  | Readonly<{
      ok: false
      error: "unauthorized" | "invalid_request" | "storage_unavailable" | "not_enabled" | "bad_response" | "network_failed"
      status: number | null
      reason: string | null
    }>

/**
 * Resolve a `turn.start` intent's `bodyRef` (`chat_message.<messageId>`
 * convention) into the real prompt text. Never throws.
 */
export const fetchChatMessage = async (
  options: FetchChatMessageOptions,
): Promise<FetchChatMessageResult> => {
  const url = new URL(CHAT_MESSAGE_READ_ROUTE_PATH, options.baseUrl)
  url.searchParams.set("threadId", options.threadId)
  url.searchParams.set("messageId", options.messageId)

  const fetchImpl = options.fetchImpl ?? globalThis.fetch
  let response: Response
  try {
    response = await fetchImpl(url.toString(), {
      headers: { Authorization: `Bearer ${options.adminToken}` },
      method: "GET",
    })
  } catch (error) {
    return {
      error: "network_failed",
      ok: false,
      reason: boundedReason(error instanceof Error ? error.message : null),
      status: null,
    }
  }

  if (response.status === 401) return { error: "unauthorized", ok: false, reason: null, status: 401 }
  if (response.status === 400) return { error: "invalid_request", ok: false, reason: null, status: 400 }
  if (response.status === 503) return { error: "storage_unavailable", ok: false, reason: null, status: 503 }
  if (response.status !== 200) {
    return { error: "bad_response", ok: false, reason: `unexpected status ${response.status}`, status: response.status }
  }

  let body: unknown
  try {
    body = await response.json()
  } catch {
    return { error: "bad_response", ok: false, reason: "response body was not JSON", status: 200 }
  }
  const record = body as { ok?: unknown; reason?: unknown; message?: unknown }
  if (record.ok !== true) {
    return { error: "not_enabled", ok: false, reason: boundedReason(record.reason), status: 200 }
  }
  if (record.message === null) {
    return { message: null, ok: true }
  }
  if (typeof record.message !== "object" || record.message === null) {
    return { error: "bad_response", ok: false, reason: "message shape was not an object", status: 200 }
  }
  return { message: record.message as ChatMessageBody, ok: true }
}
