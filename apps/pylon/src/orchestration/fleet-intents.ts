import {
  decodeFleetIntentRow,
  type FleetIntentRow,
} from "@openagentsinc/khala-sync"

/**
 * Khala Sync fleet-intent poller (KS-3.2, #8292) — the Pylon-side typed
 * reader for the durable operator intents that the fleet cockpit mutators
 * (`fleet.setDesiredSlots` / `fleet.pauseRun` / `fleet.resumeRun` /
 * `fleet.pauseWorker` / `fleet.resumeWorker` / `fleet.acknowledgeInboxFlag`
 * / `fleet.stopRun`) record in `khala_sync_fleet_intents`.
 *
 * Pragmatic v1 transport: the admin-guarded Worker route
 * `GET /api/internal/khala-sync/fleet-intents?scope=&after=&limit=`,
 * polled with the supervisor's `OPENAGENTS_ADMIN_API_TOKEN` bearer against
 * `OPENAGENTS_BASE_URL`. The route pages oldest-first and returns a
 * `nextAfter` watermark; callers persist that watermark (e.g. next to the
 * orchestration store's fleet-run row) and resume from it.
 *
 * HONEST V1 SCOPE: this module is the OBSERVATION half of intent
 * enforcement. Wiring the supervisor/orchestration loop to poll it and
 * actually change dispatch behavior (pause/resume runs and workers, stop
 * runs, clear acknowledged inbox flags) is the follow-up enforcement lane
 * tracked on epic #8282 — until that lands, an applied fleet mutation
 * remains a durable recorded request, not proof of changed dispatch.
 */

export const FLEET_INTENTS_ROUTE_PATH = "/api/internal/khala-sync/fleet-intents"

export type { FleetIntentRow } from "@openagentsinc/khala-sync"

export interface ReadPendingFleetIntentsOptions {
  /** e.g. `https://openagents.com` (`OPENAGENTS_BASE_URL`). */
  readonly baseUrl: string
  /** Admin bearer (`OPENAGENTS_ADMIN_API_TOKEN`). */
  readonly adminToken: string
  /** Resume watermark: only intents with `id > after`. Default 0. */
  readonly after?: number
  /** Restrict to one fleet scope (`scope.fleet_run.<runId>`). */
  readonly scope?: string
  /** Page size (the route clamps to its own maximum). */
  readonly limit?: number
  /** Test seam. Default `globalThis.fetch`. */
  readonly fetchImpl?: typeof globalThis.fetch
}

export type ReadPendingFleetIntentsResult =
  | Readonly<{
      ok: true
      intents: ReadonlyArray<FleetIntentRow>
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
 * Poll the Worker's fleet-intents route once. Never throws: transport and
 * contract failures come back as typed `ok: false` results so a supervisor
 * loop can log and retry on its own cadence.
 */
export const readPendingFleetIntents = async (
  options: ReadPendingFleetIntentsOptions,
): Promise<ReadPendingFleetIntentsResult> => {
  const url = new URL(FLEET_INTENTS_ROUTE_PATH, options.baseUrl)
  if (options.scope !== undefined) url.searchParams.set("scope", options.scope)
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
    return {
      error: "storage_unavailable",
      ok: false,
      reason: null,
      status: 503,
    }
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
    return {
      error: "bad_response",
      ok: false,
      reason: "response body was not JSON",
      status: 200,
    }
  }

  const record = body as {
    ok?: unknown
    reason?: unknown
    intents?: unknown
    nextAfter?: unknown
    upToDate?: unknown
  }
  if (record.ok !== true) {
    // The route's honest enablement gap (KHALA_SYNC_DB binding absent) is
    // an HTTP 200 with ok:false + reason, mirroring the db-smoke convention.
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
      reason: "response shape did not match the fleet-intents contract",
      status: 200,
    }
  }
  let intents: Array<FleetIntentRow>
  try {
    intents = record.intents.map((row) => decodeFleetIntentRow(row))
  } catch {
    return {
      error: "bad_response",
      ok: false,
      reason: "an intent row failed to decode against FleetIntentRow",
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
