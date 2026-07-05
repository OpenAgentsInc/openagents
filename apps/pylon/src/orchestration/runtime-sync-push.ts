/**
 * Minimal Khala Sync push client (#8388) — posts one named mutation to
 * `POST /api/sync/push`, the same wire shape every other Khala Sync client
 * in this repo uses (`packages/khala-sync/src/index.ts`'s `PushRequest` /
 * `MutationEnvelope`), without pulling in the full client session/overlay/
 * local-store machinery (`@openagentsinc/khala-sync`'s `createKhalaSyncSession`)
 * that desktop/mobile use for optimistic local state. The Pylon runtime
 * dispatch consumer only ever PUSHES `runtime.recordEvent` — it has no
 * local UI state to keep in sync — so a bare fetch wrapper is the right
 * amount of machinery here.
 *
 * IDEMPOTENCY / ORDERING (SPEC §2.4 invariant 5): the push engine requires
 * `mutationId` to be exactly `lastMutationId + 1` for a given
 * `(clientGroupId, clientId)` pair, densely, or the mutation comes back
 * `out_of_order` and acks nothing. Rather than track that watermark across
 * process restarts, callers mint a FRESH random `clientId` per turn
 * (`runtimeSyncClientIdForTurn`) and increment a local counter starting at
 * 1 for that turn's event stream — since nothing else ever writes under
 * that synthetic clientId, `lastMutationId` is always 0 the first time, so
 * this is correct by construction and trivially safe across restarts (a
 * restart mid-turn just mints a new clientId and starts over at 1).
 */

export const RUNTIME_RECORD_EVENT_MUTATOR_NAME = "runtime.recordEvent"
export const KHALA_SYNC_PUSH_ROUTE_PATH = "/api/sync/push"
const KHALA_SYNC_PROTOCOL_VERSION = 1
const KHALA_SYNC_SCHEMA_VERSION = 1

export type PushMutationResult = {
  readonly mutationId: number
  readonly status: "applied" | "rejected" | "duplicate"
  readonly errorCode?: string
  readonly errorMessageSafe?: string
}

export type PushKhalaSyncMutationOptions = {
  readonly baseUrl: string
  readonly agentToken: string
  readonly clientGroupId: string
  readonly clientId: string
  readonly mutationId: number
  readonly name: string
  readonly args: unknown
  readonly fetchImpl?: typeof globalThis.fetch
}

export type PushKhalaSyncMutationResult =
  | Readonly<{ ok: true; result: PushMutationResult }>
  | Readonly<{
      ok: false
      error: "unauthorized" | "invalid_request" | "bad_response" | "network_failed"
      status: number | null
      reason: string | null
    }>

const boundedReason = (value: unknown): string | null => {
  if (typeof value !== "string" || value.length === 0) return null
  return value.slice(0, 300)
}

const canonicalJson = (value: unknown): string => JSON.stringify(value)

/**
 * Push ONE mutation envelope. Never throws: transport and contract
 * failures come back as typed `ok: false` results so the dispatch consumer
 * can log and decide whether to retry (safe to retry — the same
 * `mutationId` against the same fresh clientId is idempotent per the push
 * engine's ledger).
 */
export const pushKhalaSyncMutation = async (
  options: PushKhalaSyncMutationOptions,
): Promise<PushKhalaSyncMutationResult> => {
  const url = new URL(KHALA_SYNC_PUSH_ROUTE_PATH, options.baseUrl)
  const fetchImpl = options.fetchImpl ?? globalThis.fetch
  const body = {
    clientGroupId: options.clientGroupId,
    clientId: options.clientId,
    mutations: [
      {
        argsJson: canonicalJson(options.args),
        mutationId: options.mutationId,
        name: options.name,
      },
    ],
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    schemaVersion: KHALA_SYNC_SCHEMA_VERSION,
  }

  let response: Response
  try {
    response = await fetchImpl(url.toString(), {
      body: JSON.stringify(body),
      headers: {
        Authorization: `Bearer ${options.agentToken}`,
        "Content-Type": "application/json",
      },
      method: "POST",
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
  if (response.status === 400) {
    let reason: string | null = null
    try {
      const errorBody = (await response.json()) as { reason?: unknown }
      reason = boundedReason(errorBody.reason)
    } catch {
      // best-effort; a 400 with no readable body still surfaces as invalid_request
    }
    return { error: "invalid_request", ok: false, reason, status: 400 }
  }
  if (response.status !== 200) {
    return { error: "bad_response", ok: false, reason: `unexpected status ${response.status}`, status: response.status }
  }

  let responseBody: unknown
  try {
    responseBody = await response.json()
  } catch {
    return { error: "bad_response", ok: false, reason: "response body was not JSON", status: 200 }
  }
  const record = responseBody as { results?: unknown }
  if (!Array.isArray(record.results) || record.results.length === 0) {
    return { error: "bad_response", ok: false, reason: "push response carried no results", status: 200 }
  }
  const result = record.results[0] as PushMutationResult
  return { ok: true, result }
}

/**
 * A fresh synthetic (clientGroupId, clientId) pair for one runtime turn's
 * event stream, so the push engine's dense-ordering ledger always starts
 * clean at `mutationId = 1` for that turn (see module doc above).
 */
export const runtimeSyncClientForTurn = (input: {
  readonly pylonRef: string
  readonly turnId: string
  readonly random?: string
}): { readonly clientGroupId: string; readonly clientId: string } => ({
  clientGroupId: `khala-pylon-runtime.${input.pylonRef}`,
  clientId: `runtime-turn.${input.turnId}.${input.random ?? crypto.randomUUID()}`,
})
