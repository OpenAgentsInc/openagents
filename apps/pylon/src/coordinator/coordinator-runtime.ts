// CL-36 coordinator runtime — closes the self-driving loop. Polls the intent
// queue: a "received" intent is planned (planIntent fans a checklist body into
// multiple parts = multiple agents), each part is spawned as a control session
// in its own worktree, and the intent advances received -> planning ->
// fanning_out -> shipping -> shipped/failed as its sessions finish. The spawned
// sessions appear in session.list with live timelines, so the phone + desktop
// show exactly what every fan-out agent is doing.

import { Effect } from "effect"

import type { SubmittedWorkIntent, IntentQueue, IntentStatus } from "../node/intent-intake.js"
import { planIntent } from "./planner.js"
import { classifyShipModeFromFingerprint } from "./ship-mode-classify.js"
import { decideShipEligibility } from "./ship-eligibility.js"
import { buildShipReceipt, type ShipReceipt } from "./ship-receipt.js"

// CL-37/CL-41: at the ship step, classify OTA-vs-rebuild from the Expo runtime
// fingerprint and gate the autonomous ship on the spend decision, recording a
// ship receipt. The runtime stays side-effect-free: a node supplies the
// fingerprint + spend context, and the actual publish/build is an opt-in
// callback (recordShip) so this can be unit-tested and never auto-ships unless
// the node explicitly wires execution.
export type ShipContext = {
  previousRuntimeFingerprint: string
  nextRuntimeFingerprint: string
  changedPaths: string[]
  spendGate: { decision: "allow" | "deny" }
}
export type CoordinatorShipDecision = ShipReceipt & { eligible: boolean; reason: string }

type SpawnInput = { adapter: "codex" | "claude_agent"; objective: string; verify: string[]; worktreePath: string }
type TerminalState = "completed" | "failed" | "cancelled"

export type CoordinatorRuntimeDeps = {
  intentQueue: Pick<IntentQueue, "list" | "getIntent" | "advanceStatus">
  // Spawn one coding session; returns its sessionRef.
  spawnSession: (input: SpawnInput) => Promise<{ sessionRef: string }>
  // Current state of a session (or null if unknown), for completion tracking.
  sessionState: (sessionRef: string) => Promise<string | null>
  // Materialize a fresh isolated workspace for one fan-out part.
  createWorktree: (intentId: string, index: number) => Promise<string>
  // CL-37/CL-41: at the ship step, the node supplies the Expo runtime
  // fingerprint diff + spend-gate decision; the runtime classifies OTA-vs-rebuild
  // and gates the autonomous ship, recording a receipt via recordShip. Both are
  // optional — without them the loop still advances shipped/failed as before.
  shipContext?: (intentId: string) => Promise<ShipContext | null>
  recordShip?: (intentId: string, decision: CoordinatorShipDecision) => void
  log?: (message: string) => void
  maxFanout?: number
}

export type CoordinatorIntentView = {
  intentId: string
  status: IntentStatus
  sessionRefs: string[]
}

export type CoordinatorRuntime = {
  tick: () => Promise<void>
  start: (intervalMs: number) => void
  stop: () => void
  view: () => CoordinatorIntentView[]
  // CL-17 (rescoped): pause/resume the AUTONOMOUS work loop. Pausing holds new
  // fan-out (no new intents dispatched) while letting in-flight sessions finish
  // and reconcile; resuming dispatches again. (Per-session pause/resume isn't
  // possible — the agent CLIs run to completion; use cancel to stop a session.)
  pause: () => void
  resume: () => void
  isPaused: () => boolean
}

const TERMINAL: ReadonlySet<string> = new Set<TerminalState>(["completed", "failed", "cancelled"])

// `Effect.tryPromise`'s bare-function form wraps a rejection in
// `Cause.UnknownError`, whose own `.message` is a generic
// "An error occurred in Effect.tryPromise" — the original rejection is
// preserved on `.cause`. Unwrap it so logs keep the real underlying message.
function underlyingErrorMessage(error: unknown): string {
  const cause = error instanceof Error ? error.cause : undefined
  const underlying = cause instanceof Error ? cause : error
  return underlying instanceof Error ? underlying.message : String(underlying)
}

export function createCoordinatorRuntime(deps: CoordinatorRuntimeDeps): CoordinatorRuntime {
  const log = deps.log ?? (() => {})
  const maxFanout = deps.maxFanout ?? 4
  // intentId -> spawned session refs (in-memory; the loop runs while the node is up).
  const intentSessions = new Map<string, string[]>()
  let timer: ReturnType<typeof setInterval> | null = null
  let running = false
  let paused = false

  const dispatch = async (intent: SubmittedWorkIntent): Promise<void> => {
    deps.intentQueue.advanceStatus(intent.intentId, "planning")
    const entries = planIntent(
      {
        intentId: intent.intentId,
        title: intent.title,
        body: intent.body,
        ...(intent.scopeHint === undefined ? {} : { scopeHint: intent.scopeHint }),
      },
      // Workspace is overridden per-entry below with a fresh worktree; this is a
      // placeholder so the planner's single-selector guard is satisfied.
      { worktreePath: "__per_entry__", availableAccounts: [] },
    ).slice(0, maxFanout)

    const refs: string[] = []
    for (let index = 0; index < entries.length; index++) {
      const entry = entries[index]
      try {
        const worktreePath = await deps.createWorktree(intent.intentId, index)
        const spawned = await deps.spawnSession({
          adapter: (entry.adapter as SpawnInput["adapter"]) ?? "codex",
          objective: entry.objective,
          verify: entry.verify ?? ["true"],
          worktreePath,
        })
        refs.push(spawned.sessionRef)
        log(`[coordinator] intent ${intent.intentId} -> session ${spawned.sessionRef} (part ${index + 1}/${entries.length})`)
      } catch (error) {
        log(`[coordinator] spawn failed for ${intent.intentId} part ${index + 1}: ${error instanceof Error ? error.message : String(error)}`)
      }
    }

    intentSessions.set(intent.intentId, refs)
    if (refs.length === 0) {
      deps.intentQueue.advanceStatus(intent.intentId, "failed")
      return
    }
    deps.intentQueue.advanceStatus(intent.intentId, "fanning_out")
  }

  // Each session's state read is independent of its siblings. Isolate them
  // with Effect structured concurrency (`Effect.forEach` + `Effect.result`)
  // instead of a bare `Promise.all`: one flaky/failed `sessionState` read
  // must not abort `reconcile` for this intent's OTHER sessions, and — since
  // `tick()` calls `reconcile` in a `for...of` loop over every queued intent
  // each cycle — must not abort dispatch/reconcile for every OTHER unrelated
  // intent still queued in the same tick either. A failed read is logged and
  // treated as "not yet observable" (`null`), matching the existing
  // null-handling contract below (`known.length < refs.length` defers this
  // intent to the next tick rather than crashing the whole cycle).
  const reconcile = async (intentId: string): Promise<void> => {
    const refs = intentSessions.get(intentId) ?? []
    if (refs.length === 0) return
    const stateOutcomes = await Effect.runPromise(
      Effect.forEach(
        refs,
        (ref) =>
          Effect.result(Effect.tryPromise(() => deps.sessionState(ref))).pipe(
            Effect.map((outcome) => ({ ref, outcome })),
          ),
        { concurrency: "unbounded" },
      ),
    )
    const states = stateOutcomes.map(({ ref, outcome }) => {
      if (outcome._tag === "Success") return outcome.success
      log(
        `[coordinator] session state read failed for ${ref} (intent ${intentId}): ${underlyingErrorMessage(outcome.failure)}`,
      )
      return null
    })
    const known = states.filter((s): s is string => s !== null)
    if (known.length < refs.length) return // some sessions not yet observable
    if (!known.every((s) => TERMINAL.has(s))) return // still running
    const allPassed = known.every((s) => s === "completed")
    if (!allPassed) {
      deps.intentQueue.advanceStatus(intentId, "failed")
      log(`[coordinator] intent ${intentId} failed (${refs.length} agents)`)
      return
    }
    deps.intentQueue.advanceStatus(intentId, "shipping")
    // CL-37/CL-41 ship step: classify OTA-vs-rebuild from the fingerprint and
    // gate the autonomous ship on spend, recording a receipt. Side-effect-free:
    // the actual publish/build is the node's job via recordShip.
    if (deps.shipContext !== undefined) {
      try {
        const ctx = await deps.shipContext(intentId)
        if (ctx !== null) {
          const mode = classifyShipModeFromFingerprint({
            previousRuntimeFingerprint: ctx.previousRuntimeFingerprint,
            nextRuntimeFingerprint: ctx.nextRuntimeFingerprint,
            changedPaths: ctx.changedPaths,
          })
          const eligibility = decideShipEligibility({ mode: mode.mode, spendGate: ctx.spendGate })
          const receipt = buildShipReceipt({
            intentId,
            shipMode: mode.mode,
            decision: eligibility.eligible ? "auto" : "escalate",
            summary: `${mode.mode}: ${mode.reason} | ${eligibility.reason}`,
          })
          deps.recordShip?.(intentId, { ...receipt, eligible: eligibility.eligible, reason: eligibility.reason })
          log(
            `[coordinator] intent ${intentId} ship decision: mode=${mode.mode} eligible=${eligibility.eligible} (${eligibility.reason})`,
          )
        }
      } catch (error) {
        log(`[coordinator] ship decision failed for ${intentId}: ${error instanceof Error ? error.message : String(error)}`)
      }
    }
    deps.intentQueue.advanceStatus(intentId, "shipped")
    log(`[coordinator] intent ${intentId} shipped (${refs.length} agents)`)
  }

  const tick = async (): Promise<void> => {
    if (running) return
    running = true
    try {
      for (const projection of deps.intentQueue.list()) {
        if (projection.status === "received") {
          // Paused: hold new fan-out, but keep reconciling in-flight work below.
          if (paused) continue
          const intent = deps.intentQueue.getIntent(projection.intentId)
          if (intent !== null) await dispatch(intent)
        } else if (projection.status === "fanning_out") {
          await reconcile(projection.intentId)
        }
      }
    } finally {
      running = false
    }
  }

  return {
    tick,
    pause() {
      paused = true
      log("[coordinator] paused — holding new fan-out")
    },
    resume() {
      paused = false
      log("[coordinator] resumed")
    },
    isPaused: () => paused,
    start(intervalMs) {
      if (timer !== null) return
      void tick()
      timer = setInterval(() => void tick(), intervalMs)
      timer.unref?.()
    },
    stop() {
      if (timer !== null) clearInterval(timer)
      timer = null
    },
    view() {
      return [...intentSessions.entries()].map(([intentId, sessionRefs]) => {
        const projection = deps.intentQueue.list().find((p) => p.intentId === intentId)
        return { intentId, status: projection?.status ?? "received", sessionRefs: [...sessionRefs] }
      })
    },
  }
}
