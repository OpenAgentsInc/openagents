// CL-36 coordinator runtime — closes the self-driving loop. Polls the intent
// queue: a "received" intent is planned (planIntent fans a checklist body into
// multiple parts = multiple agents), each part is spawned as a control session
// in its own worktree, and the intent advances received -> planning ->
// fanning_out -> shipping -> shipped/failed as its sessions finish. The spawned
// sessions appear in session.list with live timelines, so the phone + desktop
// show exactly what every fan-out agent is doing.

import type { SubmittedWorkIntent, IntentQueue, IntentStatus } from "../node/intent-intake"
import { planIntent } from "./planner"

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
}

const TERMINAL: ReadonlySet<string> = new Set<TerminalState>(["completed", "failed", "cancelled"])

export function createCoordinatorRuntime(deps: CoordinatorRuntimeDeps): CoordinatorRuntime {
  const log = deps.log ?? (() => {})
  const maxFanout = deps.maxFanout ?? 4
  // intentId -> spawned session refs (in-memory; the loop runs while the node is up).
  const intentSessions = new Map<string, string[]>()
  let timer: ReturnType<typeof setInterval> | null = null
  let running = false

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

  const reconcile = async (intentId: string): Promise<void> => {
    const refs = intentSessions.get(intentId) ?? []
    if (refs.length === 0) return
    const states = await Promise.all(refs.map((ref) => deps.sessionState(ref)))
    const known = states.filter((s): s is string => s !== null)
    if (known.length < refs.length) return // some sessions not yet observable
    if (!known.every((s) => TERMINAL.has(s))) return // still running
    const allPassed = known.every((s) => s === "completed")
    deps.intentQueue.advanceStatus(intentId, "shipping")
    deps.intentQueue.advanceStatus(intentId, allPassed ? "shipped" : "failed")
    log(`[coordinator] intent ${intentId} ${allPassed ? "shipped" : "failed"} (${refs.length} agents)`)
  }

  const tick = async (): Promise<void> => {
    if (running) return
    running = true
    try {
      for (const projection of deps.intentQueue.list()) {
        if (projection.status === "received") {
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
