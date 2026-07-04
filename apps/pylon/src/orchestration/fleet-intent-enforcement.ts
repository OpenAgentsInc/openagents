import {
  readPendingFleetIntents,
  type FleetIntentRow,
  type ReadPendingFleetIntentsResult,
} from "./fleet-intents.js"
import type {
  FleetIntentOutcomeStatus,
  PylonOrchestrationStore,
} from "./store.js"

/**
 * Fleet-intent ENFORCEMENT (KS-3.2 #8332) — the supervisor-side consumer
 * that turns durable operator intents (`khala_sync_fleet_intents`, written
 * by the fleet cockpit mutators) into actual dispatch-behavior changes in
 * the Pylon orchestration store:
 *
 * - `set_desired_slots` — durable operator slots cap
 *   (`store.setFleetRunDesiredSlotsCap`) + `targetConcurrency` when >= 1;
 *   the supervisor loop reads `store.effectiveFleetRunDesiredSlots`
 * - `pause` / `resume` — `store.updateFleetRunState` (operator provenance)
 * - `pause_worker` / `resume_worker` — `store.setDispatchContextPaused`
 *   (`workerId` is the projected dispatch-context id); paused contexts are
 *   refused by `dispatchEligibility` with `worker_paused`
 * - `stop` — terminal `stopped` state + release of the run's live claims
 * - `acknowledge_inbox_flag` — recorded no-op today: the acknowledged
 *   `fleet_inbox_flag` post-image is already durable server-side, and the
 *   orchestration store has no local attention-item rows yet (flag
 *   producers are a follow-up projection lane on epic #8282)
 *
 * EXACTLY-ONCE: consumption is watermarked (`store.getFleetIntentWatermark`
 * persists `nextAfter` across restarts) AND deduped per intent id — every
 * application records a `FleetIntentOutcomeRecord` keyed by the intent's
 * monotonic id, and a redelivered intent that already has an outcome is
 * returned as `deduped` without touching state again.
 *
 * FAILURE ISOLATION: one bad intent NEVER wedges the loop. Application
 * errors are caught per intent, recorded as a `failed` outcome with a
 * bounded public-safe detail, and the loop moves on; the watermark still
 * advances so the poison intent is consumed, inspectable, and not retried
 * forever. Transport/contract failures from the poller leave the watermark
 * untouched and come back as a typed `ok: false` result.
 */

const boundedDetail = (value: string | null | undefined): string | null => {
  if (typeof value !== "string" || value.length === 0) return null
  return value.slice(0, 300)
}

const TERMINAL_RUN_STATES: ReadonlySet<string> = new Set(["stopped", "completed"])

export type FleetIntentApplication = {
  outcome: FleetIntentOutcomeStatus
  detail: string | null
}

const applied = (detail: string | null = null): FleetIntentApplication => ({
  detail,
  outcome: "applied",
})

const skippedStale = (detail: string): FleetIntentApplication => ({
  detail,
  outcome: "skipped_stale",
})

const failed = (detail: string): FleetIntentApplication => ({
  detail: boundedDetail(detail),
  outcome: "failed",
})

/**
 * Apply ONE decoded intent to the orchestration store's control primitives.
 * Idempotent per intent kind (re-applying a pause to a paused run is
 * `applied`); intents that no longer make sense are `skipped_stale` with an
 * honest reason. Throws only on store-level errors — callers convert those
 * to `failed` outcomes.
 */
export const applyFleetIntentToStore = (
  store: PylonOrchestrationStore,
  intent: FleetIntentRow,
  now: Date = new Date(),
): FleetIntentApplication => {
  const runRef = intent.runId

  switch (intent.intent) {
    case "set_desired_slots": {
      const desiredSlots = intent.desiredSlots
      if (desiredSlots === null || !Number.isInteger(desiredSlots) || desiredSlots < 0) {
        return failed("intent carried no usable desiredSlots")
      }
      const run = store.getFleetRun(runRef)
      if (run === null) return skippedStale("unknown_run")
      if (TERMINAL_RUN_STATES.has(run.state)) return skippedStale(`run_terminal:${run.state}`)
      store.setFleetRunDesiredSlotsCap(runRef, desiredSlots)
      if (desiredSlots >= 1) {
        store.upsertFleetRun({ ...run, targetConcurrency: desiredSlots }, now)
      }
      return applied(`desired slots cap=${desiredSlots}; effective=${store.effectiveFleetRunDesiredSlots(runRef)}`)
    }
    case "pause": {
      const run = store.getFleetRun(runRef)
      if (run === null) return skippedStale("unknown_run")
      if (run.state === "paused") return applied("already paused")
      if (run.state !== "running") return skippedStale(`pause_not_applicable_from:${run.state}`)
      store.updateFleetRunState(runRef, "paused", now, "operator")
      return applied("run paused")
    }
    case "resume": {
      const run = store.getFleetRun(runRef)
      if (run === null) return skippedStale("unknown_run")
      if (run.state === "running") return applied("already running")
      if (run.state !== "paused") return skippedStale(`resume_not_applicable_from:${run.state}`)
      store.updateFleetRunState(runRef, "running", now, "operator")
      return applied("run resumed")
    }
    case "stop": {
      const run = store.getFleetRun(runRef)
      if (run === null) return skippedStale("unknown_run")
      if (TERMINAL_RUN_STATES.has(run.state)) return applied(`already terminal:${run.state}`)
      store.updateFleetRunState(runRef, "stopped", now, "operator")
      const liveClaims = store
        .listLiveWorkClaims(now)
        .filter((claim) => claim.runRef === runRef)
      for (const claim of liveClaims) {
        store.releaseWorkClaim(claim.claimRef, now)
      }
      return applied(`run stopped; released ${liveClaims.length} live claim(s)`)
    }
    case "pause_worker": {
      if (intent.workerId === null) return failed("intent carried no workerId")
      const context = store.getDispatchContext(intent.workerId)
      if (context === null) return skippedStale("unknown_worker")
      if (context.paused) return applied("worker already paused")
      store.setDispatchContextPaused(intent.workerId, true, now)
      return applied("worker paused")
    }
    case "resume_worker": {
      if (intent.workerId === null) return failed("intent carried no workerId")
      const context = store.getDispatchContext(intent.workerId)
      if (context === null) return skippedStale("unknown_worker")
      if (!context.paused) return applied("worker already resumed")
      store.setDispatchContextPaused(intent.workerId, false, now)
      return applied("worker resumed")
    }
    case "acknowledge_inbox_flag": {
      // HONEST no-op: the ack is already durable server-side (acknowledged
      // fleet_inbox_flag post-image, written in the mutator transaction) and
      // there are no pylon-local attention-item rows to clear yet — flag
      // producers are a follow-up projection lane (epic #8282).
      return applied("acknowledged server-side; no pylon-local attention items to clear")
    }
  }
}

export type EnforcedFleetIntentOutcome = {
  intentId: number
  runRef: string
  intent: FleetIntentRow["intent"]
  outcome: FleetIntentOutcomeStatus
  detail: string | null
  /** True when the intent already had a recorded outcome (redelivery). */
  deduped: boolean
}

export type ReadPendingFleetIntentsLike = (options: {
  baseUrl: string
  adminToken: string
  after?: number
  scope?: string
  limit?: number
  fetchImpl?: typeof globalThis.fetch
}) => Promise<ReadPendingFleetIntentsResult>

export interface EnforceFleetIntentsOptions {
  /** e.g. `https://openagents.com` (`OPENAGENTS_BASE_URL`). */
  readonly baseUrl: string
  /** Admin bearer (`OPENAGENTS_ADMIN_API_TOKEN`); never echoed. */
  readonly adminToken: string
  /** Restrict the poll to one fleet scope (`scope.fleet_run.<runId>`). */
  readonly scope?: string
  /** Page size (the route clamps to its own maximum). */
  readonly limit?: number
  readonly now?: Date
  /** Test seam for the HTTP transport. */
  readonly fetchImpl?: typeof globalThis.fetch
  /** Test seam for the whole poller. Default `readPendingFleetIntents`. */
  readonly readImpl?: ReadPendingFleetIntentsLike
  /**
   * Public-safe per-intent log line (intent id/kind, public run ref,
   * outcome — never tokens). Default `console.error` so supervisor logs
   * capture it without polluting stdout JSON.
   */
  readonly log?: (line: string) => void
}

export type EnforceFleetIntentsResult =
  | Readonly<{
      ok: true
      outcomes: ReadonlyArray<EnforcedFleetIntentOutcome>
      /** Watermark persisted to the store after this page. */
      nextAfter: number
      upToDate: boolean
    }>
  | Readonly<{
      ok: false
      error: Extract<ReadPendingFleetIntentsResult, { ok: false }>["error"]
      status: number | null
      reason: string | null
      /** Watermark left untouched so the page is re-polled next tick. */
      watermark: number
    }>

/**
 * One enforcement tick: poll the Worker's fleet-intents route from the
 * persisted watermark, apply every new intent to the store, record
 * per-intent outcomes, and advance the watermark. Never throws.
 */
export const enforcePendingFleetIntents = async (
  store: PylonOrchestrationStore,
  options: EnforceFleetIntentsOptions,
): Promise<EnforceFleetIntentsResult> => {
  const now = options.now ?? new Date()
  const log = options.log ?? ((line: string) => console.error(line))
  const read = options.readImpl ?? readPendingFleetIntents
  const watermark = store.getFleetIntentWatermark(options.scope)

  const page = await read({
    adminToken: options.adminToken,
    after: watermark,
    baseUrl: options.baseUrl,
    ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
    ...(options.limit === undefined ? {} : { limit: options.limit }),
    ...(options.scope === undefined ? {} : { scope: options.scope }),
  })
  if (!page.ok) {
    log(`fleet-intents poll failed error=${page.error} status=${page.status ?? "none"} watermark=${watermark}`)
    return {
      error: page.error,
      ok: false,
      reason: page.reason,
      status: page.status,
      watermark,
    }
  }

  const outcomes: EnforcedFleetIntentOutcome[] = []
  for (const intent of page.intents) {
    const existing = store.getFleetIntentOutcome(intent.id)
    if (existing !== null) {
      outcomes.push({
        deduped: true,
        detail: existing.detail,
        intent: intent.intent,
        intentId: intent.id,
        outcome: existing.outcome,
        runRef: intent.runId,
      })
      continue
    }
    let application: FleetIntentApplication
    try {
      application = applyFleetIntentToStore(store, intent, now)
    } catch (error) {
      application = failed(error instanceof Error ? error.message : "intent application threw")
    }
    let recordedOutcome = application
    try {
      const { outcome } = store.recordFleetIntentOutcome({
        detail: application.detail,
        intent: intent.intent,
        intentId: intent.id,
        mutationRef: intent.mutationRef,
        now,
        outcome: application.outcome,
        runRef: intent.runId,
        scope: intent.scope,
      })
      recordedOutcome = { detail: outcome.detail, outcome: outcome.outcome }
    } catch {
      // Outcome persistence failing must not wedge the loop either. The
      // watermark still advances (the state change already landed), so a
      // lost outcome row is a bookkeeping gap, not a correctness gap —
      // every application above is idempotent per intent kind.
    }
    outcomes.push({
      deduped: false,
      detail: recordedOutcome.detail,
      intent: intent.intent,
      intentId: intent.id,
      outcome: recordedOutcome.outcome,
      runRef: intent.runId,
    })
    log(
      `fleet-intent id=${intent.id} intent=${intent.intent} run=${intent.runId} -> ${recordedOutcome.outcome}` +
        (recordedOutcome.detail === null ? "" : ` (${recordedOutcome.detail})`),
    )
  }

  if (page.nextAfter > watermark) {
    store.setFleetIntentWatermark(page.nextAfter, options.scope)
  }

  return {
    nextAfter: Math.max(page.nextAfter, watermark),
    ok: true,
    outcomes,
    upToDate: page.upToDate,
  }
}
