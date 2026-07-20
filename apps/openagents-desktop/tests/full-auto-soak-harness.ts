import path from "node:path"

import type { DesktopMessage } from "../src/chat-contract.ts"
import { settleFullAutoRunLiveness } from "../src/full-auto-liveness.ts"
import {
  classifyFullAutoDispatchFailure,
  FULL_AUTO_MAX_CONTINUATIONS,
  reconcileFullAutoThreads,
} from "../src/full-auto-reconcile.ts"
import {
  openFullAutoRegistry,
  type FullAutoRegistry,
  type FullAutoRotationRecord,
  type FullAutoRoutingCandidate,
} from "../src/full-auto-registry.ts"
import { validateFullAutoRoutingPolicy } from "../src/full-auto-routing.ts"
import {
  isFullAutoRunTerminal,
  openFullAutoRunRegistry,
  type FullAutoRunActor,
  type FullAutoRunRegistry,
  type FullAutoRunState,
} from "../src/full-auto-run-registry.ts"
import {
  openFullAutoRunReportStore,
  type FullAutoRunReport,
  type FullAutoRunReportStore,
} from "../src/full-auto-run-report.ts"
import { openLocalTurnJournal, type LocalTurnJournal } from "../src/local-turn-journal.ts"
import { makeThreadStore } from "../src/thread-store.ts"

/**
 * FA-SOAK-01 (#8992): the long-window Full Auto soak harness and the SM-10
 * typed-termination measurement, shared by tests/full-auto-soak.e2e.test.ts
 * (CI, compressed clock) and scripts/full-auto-soak.ts (`--smoke` real-time
 * fixture run, `--collect` live-report measurement).
 *
 * What is REAL here (the exact production seams, same discipline as
 * full-auto-acceptance-driver.ts and tests/full-auto-thread-pressure.e2e.test.ts):
 *
 *  - continuation scheduling: `reconcileFullAutoThreads` over the durable
 *    `openFullAutoRegistry` lease/cap/backoff/rotation machinery (#8987);
 *  - run lifecycle: `openFullAutoRunRegistry` + `settleFullAutoRunLiveness`
 *    (FA-RUN-01/03) -- every terminal state in this harness is produced by
 *    the production transition graph and attribution vocabulary, never
 *    hand-written;
 *  - turn records: `openLocalTurnJournal`; threads: `makeThreadStore` (the
 *    real 5-slot bounded cache, so the cache-pressure scenario applies REAL
 *    eviction pressure);
 *  - the report: `openFullAutoRunReportStore.sync` (#8988) with the
 *    thread-record and liveness-projection inputs the control server passes;
 *  - routing-policy admission for the rotation scenario:
 *    `validateFullAutoRoutingPolicy` (#8987) over a fixture lane gate.
 *
 * What is FIXTURE: only the provider. The dispatch wrapper mirrors main.ts's
 * `runFullAutoReconciliation` wiring (in-flight guard, thread fail-closed
 * check, `classifyFullAutoDispatchFailure` -> `failureClass` mapping) around
 * a scripted per-lane fault queue instead of a live child process. No
 * Keychain access, no live provider calls.
 *
 * Long windows are compressed by injecting the harness clock as `now` into
 * every durable store and into reconciliation, so FA-H5 backoff windows
 * (minutes) elapse instantly in CI. The real-time smoke variant uses the
 * actual wall clock and therefore only runs the scenarios that do not need
 * backoff jumps (`requiresCompressedClock: false`).
 *
 * SM-10 (desktop surface spec rev 3): share of soak runs terminating by a
 * TYPED reason -- owner stop, cap, objective complete, guardrail/policy
 * block, FA-H5 disable -- versus unhandled/unclassified error. The >= 99%
 * target is recorded in the summary as the gate VALUE; meeting it on this
 * synthetic fixture population is deliberately NOT asserted as a product
 * claim (SM-11 owner-AFK dogfood evidence is the live measurement).
 */

// -----------------------------------------------------------------------
// Clock.
// -----------------------------------------------------------------------

export type SoakClockMode = "compressed" | "realtime"

export type SoakClock = Readonly<{
  mode: SoakClockMode
  now: () => Date
  /** Advance logical time by `ms`. Compressed: instant. Realtime: a real,
   * bounded wait (the wall clock is the clock; waiting the full duration
   * would defeat the smoke budget, and no realtime scenario needs it). */
  advance: (ms: number) => Promise<void>
}>

export const makeCompressedSoakClock = (startMs: number = Date.now()): SoakClock => {
  let currentMs = startMs
  return {
    mode: "compressed",
    now: () => new Date(currentMs),
    advance: ms => {
      currentMs += ms
      return Promise.resolve()
    },
  }
}

/** Bound on any single realtime wait -- keeps the `--smoke` run interactive. */
export const REALTIME_SOAK_MAX_WAIT_MS = 50

export const makeRealtimeSoakClock = (): SoakClock => ({
  mode: "realtime",
  now: () => new Date(),
  advance: ms =>
    new Promise(resolve => setTimeout(resolve, Math.max(2, Math.min(ms, REALTIME_SOAK_MAX_WAIT_MS)))),
})

// -----------------------------------------------------------------------
// Fault injection: a per-lane FIFO of scripted dispatch outcomes. An empty
// queue means the fixture provider completes the turn (the healthy default).
// -----------------------------------------------------------------------

export type SoakDispatchOutcome =
  | Readonly<{ kind: "complete"; text?: string }>
  /** A typed lane failure: `reason` is the lane's terminal failure reason
   * (the vocabulary `classifyFullAutoDispatchFailure` reads), `detail` the
   * bounded public-safe detail string main.ts forwards as the durable
   * blockedReason. */
  | Readonly<{ kind: "lane_failure"; reason: string; detail?: string }>

export type SoakFaultQueue = Readonly<{
  push: (lane: string, ...outcomes: ReadonlyArray<SoakDispatchOutcome>) => void
  next: (lane: string) => SoakDispatchOutcome
}>

export const makeSoakFaultQueue = (): SoakFaultQueue => {
  const queues = new Map<string, Array<SoakDispatchOutcome>>()
  return {
    push: (lane, ...outcomes) => {
      queues.set(lane, [...(queues.get(lane) ?? []), ...outcomes])
    },
    next: lane => queues.get(lane)?.shift() ?? { kind: "complete" },
  }
}

// -----------------------------------------------------------------------
// Scenario-scoped event log (survives an in-scenario app restart, which
// reopens the runtime but keeps the same scenario-level sink).
// -----------------------------------------------------------------------

export type SoakEvents = {
  dispatchFailures: Array<Readonly<{
    threadRef: string
    reason: string
    consecutiveFailures: number
    disabled: boolean
  }>>
  rotations: Array<FullAutoRotationRecord>
  capStops: Array<string>
  workspaceBlocks: Array<Readonly<{ threadRef: string; reason: string }>>
  restarts: number
}

const makeSoakEvents = (): SoakEvents => ({
  dispatchFailures: [],
  rotations: [],
  capStops: [],
  workspaceBlocks: [],
  restarts: 0,
})

// -----------------------------------------------------------------------
// Runtime: one "desktop process" over durable files under `root`. Opening a
// second runtime over the same root is a cold app restart, exactly like the
// existing restart e2e proofs.
// -----------------------------------------------------------------------

export const SOAK_WORKSPACE_REF = "/granted/soak-workspace"
export const SOAK_DEFAULT_LANE = "codex-local"
export const SOAK_ALTERNATE_LANE = "claude-local"

type SoakRuntime = Readonly<{
  store: ReturnType<typeof makeThreadStore>
  journal: LocalTurnJournal
  registry: FullAutoRegistry
  runRegistry: FullAutoRunRegistry
  reportStore: FullAutoRunReportStore
  reconcile: (options?: Readonly<{ startup?: boolean }>) => Promise<ReadonlyArray<string>>
  settleAndSync: (runRef: string) => Readonly<{ report: FullAutoRunReport }>
}>

const openSoakRuntime = (input: Readonly<{
  root: string
  clock: SoakClock
  faults: SoakFaultQueue
  events: SoakEvents
  resolveWorkspaceRef: () => string
}>): SoakRuntime => {
  const { clock, faults, events } = input
  const now = (): Date => clock.now()
  const store = makeThreadStore(path.join(input.root, "threads.json"))
  const journal = openLocalTurnJournal(path.join(input.root, "local-turns.json"), now)
  const registry = openFullAutoRegistry(path.join(input.root, "full-auto", "registry.json"), now)
  const runRegistry = openFullAutoRunRegistry(path.join(input.root, "full-auto", "runs.json"), now)
  const reportStore = openFullAutoRunReportStore(
    path.join(input.root, "full-auto", "run-reports.json"),
    now,
  )

  /** main.ts's `runFullAutoReconciliation` dispatch wiring, minus the live
   * lane child process: the same in-flight guard, the same thread
   * fail-closed check (the exact incident string), and the same
   * `classifyFullAutoDispatchFailure` -> `failureClass` mapping #8987's
   * rotation reads. The scripted fault queue stands in for the provider. */
  const dispatch = async (dispatchInput: Readonly<{
    threadRef: string
    turnRef: string
    message: string
    profile?: Readonly<{ lane?: string }>
  }>): Promise<Readonly<{ ok: boolean; reason?: string; failureClass?: FullAutoRotationRecord["reason"] }>> => {
    if (journal.nonterminal().some(record => record.threadRef === dispatchInput.threadRef)) {
      return { ok: false, reason: "turn_already_in_flight" }
    }
    const lane = dispatchInput.profile?.lane ?? SOAK_DEFAULT_LANE
    const thread = store.open(dispatchInput.threadRef)
    if (thread === null) return { ok: false, reason: "That conversation no longer exists." }
    const outcome = faults.next(lane)
    if (outcome.kind === "lane_failure") {
      const failureClass = classifyFullAutoDispatchFailure(outcome.reason, outcome.detail)
      return {
        ok: false,
        reason: outcome.detail ?? outcome.reason,
        ...(failureClass === null ? {} : { failureClass }),
      }
    }
    const accepted = journal.accept({
      threadRef: dispatchInput.threadRef,
      turnRef: dispatchInput.turnRef,
      lane,
      userMessageKey: `${dispatchInput.turnRef}-user`,
      assistantMessageKey: `${dispatchInput.turnRef}-assistant`,
    })
    if (!accepted.accepted) return { ok: false, reason: "turn not accepted by the journal" }
    const text = outcome.text ?? "Autonomous packet complete."
    const key = { threadRef: dispatchInput.threadRef, turnRef: dispatchInput.turnRef, lane }
    journal.setAssistantText(key, text)
    journal.terminal(key, "completed", "completed")
    store.append(dispatchInput.threadRef, {
      key: `${dispatchInput.turnRef}-assistant`,
      role: "assistant",
      text,
      timestamp: now().toISOString(),
      meta: { lane, turnRef: dispatchInput.turnRef },
    })
    return { ok: true }
  }

  const reconcile: SoakRuntime["reconcile"] = options =>
    reconcileFullAutoThreads({
      registry,
      nonterminalThreadRefs: () => new Set(journal.nonterminal().map(record => record.threadRef)),
      resolveWorkspaceRef: input.resolveWorkspaceRef,
      journalHasNonterminalTurn: turnRef =>
        journal.nonterminal().some(record => record.turnRef === turnRef),
      now,
      ...(options?.startup === true ? { clearStaleLeases: true } : {}),
      dispatch,
      onDispatchFailed: (threadRef, failure) => {
        events.dispatchFailures.push({
          threadRef,
          reason: failure.reason,
          consecutiveFailures: failure.consecutiveFailures,
          disabled: failure.disabled,
        })
      },
      onRotated: (_threadRef, rotation) => {
        events.rotations.push(rotation)
      },
      onCapReached: threadRef => {
        events.capStops.push(threadRef)
      },
      onWorkspaceBlocked: (threadRef, block) => {
        events.workspaceBlocks.push({ threadRef, reason: block.reason })
      },
    })

  /** The control server's settle-and-sync sequence: thread-state sync +
   * liveness classification through `settleFullAutoRunLiveness`, then a
   * report sync fed the same thread record and liveness projection. */
  const settleAndSync: SoakRuntime["settleAndSync"] = runRef => {
    const run = runRegistry.get(runRef)
    if (run === null) throw new Error(`soak harness: run ${runRef} missing from the run registry`)
    const threadRecord = run.threadRef === undefined ? null : registry.record(run.threadRef)
    const settled = settleFullAutoRunLiveness(
      runRegistry,
      run,
      {
        threadRecord,
        turnRunning: run.threadRef !== undefined &&
          journal.nonterminal().some(record => record.threadRef === run.threadRef),
      },
      now,
    )
    const report = reportStore.sync({
      run: settled.run,
      turns: run.threadRef === undefined
        ? []
        : journal.list().filter(record => record.threadRef === run.threadRef),
      handoffs: [],
      livenessProjection: settled.projection,
      threadRecord,
      metricsEnabled: true,
    })
    return { report }
  }

  return { store, journal, registry, runRegistry, reportStore, reconcile, settleAndSync }
}

// -----------------------------------------------------------------------
// SM-10 termination classification -- pure, over the run report's own typed
// terminal state + stop attribution (#8988). Anything that is not terminal,
// or terminal without an attributed stopper, or terminal under an actor this
// vocabulary does not recognize for that state, is honestly `untyped`.
// -----------------------------------------------------------------------

export type Sm10TerminationClass =
  | "owner_stop"
  | "cap"
  | "objective_complete"
  | "guardrail_policy_block"
  | "fa_h5_disable"
  | "untyped"

const OWNER_STOP_ACTORS: ReadonlySet<FullAutoRunActor> = new Set([
  "owner_ui",
  "control_api",
  "cli",
  "mcp",
])

export const classifySm10Termination = (input: Readonly<{
  state: FullAutoRunState
  stopAttribution: FullAutoRunActor | null | undefined
}>): Sm10TerminationClass => {
  if (!isFullAutoRunTerminal(input.state)) return "untyped"
  const actor = input.stopAttribution ?? null
  if (actor === null) return "untyped"
  switch (input.state) {
    case "completed":
      return "objective_complete"
    case "cap_reached":
      return actor === "continuation_cap" ? "cap" : "untyped"
    case "stopped":
      return OWNER_STOP_ACTORS.has(actor) ? "owner_stop" : "untyped"
    case "failed":
      if (actor === "workspace_guard") return "guardrail_policy_block"
      if (actor === "dispatch_failure_limit") return "fa_h5_disable"
      return "untyped"
    default:
      return "untyped"
  }
}

// -----------------------------------------------------------------------
// The machine-readable SM-10 summary.
// -----------------------------------------------------------------------

export const SM10_SUMMARY_SCHEMA = "openagents.desktop.full_auto_soak_sm10_summary.v1" as const
export const SM10_GATE_TARGET = 0.99

/** The minimal per-run row the summary needs -- soak results conform
 * structurally, and `--collect` builds rows from live run reports. */
export type Sm10PopulationEntry = Readonly<{
  scenario: string
  runRef: string
  state: FullAutoRunState
  stopAttribution: FullAutoRunActor | null
  classification: Sm10TerminationClass
}>

export type FullAutoSoakSm10Summary = Readonly<{
  schema: typeof SM10_SUMMARY_SCHEMA
  generatedAt: string
  clockMode: string
  population: number
  runs: ReadonlyArray<Sm10PopulationEntry>
  classCounts: Readonly<Record<Sm10TerminationClass, number>>
  sm10: Readonly<{
    typedTerminations: number
    untypedTerminations: number
    /** null only for an empty population -- never fabricated. */
    typedTerminationRate: number | null
    gate: Readonly<{
      target: number
      comparator: ">="
      source: string
      note: string
    }>
  }>
}>

export const buildFullAutoSoakSm10Summary = (
  entries: ReadonlyArray<Sm10PopulationEntry>,
  input: Readonly<{ clockMode: string; generatedAt: string }>,
): FullAutoSoakSm10Summary => {
  const classCounts: Record<Sm10TerminationClass, number> = {
    owner_stop: 0,
    cap: 0,
    objective_complete: 0,
    guardrail_policy_block: 0,
    fa_h5_disable: 0,
    untyped: 0,
  }
  for (const entry of entries) classCounts[entry.classification] += 1
  const typed = entries.length - classCounts.untyped
  return {
    schema: SM10_SUMMARY_SCHEMA,
    generatedAt: input.generatedAt,
    clockMode: input.clockMode,
    population: entries.length,
    runs: entries.map(entry => ({
      scenario: entry.scenario,
      runRef: entry.runRef,
      state: entry.state,
      stopAttribution: entry.stopAttribution,
      classification: entry.classification,
    })),
    classCounts,
    sm10: {
      typedTerminations: typed,
      untypedTerminations: classCounts.untyped,
      typedTerminationRate: entries.length === 0 ? null : typed / entries.length,
      gate: {
        target: SM10_GATE_TARGET,
        comparator: ">=",
        source: "desktop surface spec rev 3 SM-10 (FA-SOAK-01 #8992)",
        note: "The target is recorded here as the gate VALUE. Meeting it on a synthetic fixture population is not a product claim; SM-11 owner-AFK dogfood evidence is the live measurement.",
      },
    },
  }
}

// -----------------------------------------------------------------------
// The fault matrix.
// -----------------------------------------------------------------------

export type SoakScenarioId =
  | "clean_objective_completion"
  | "owner_stop_mid_run"
  | "cap_exhaustion"
  | "account_exhausted_rotation"
  | "account_exhausted_no_alternate"
  | "provider_error_fa_h5_disable"
  | "provider_error_transient_recovery"
  | "app_restart_mid_run"
  | "cache_pressure"
  | "workspace_drift_block"

export type SoakScenarioDefinition = Readonly<{
  id: SoakScenarioId
  description: string
  /** True when the scenario must jump FA-H5 backoff windows (minutes) and
   * therefore cannot run under the real wall clock within a smoke budget. */
  requiresCompressedClock: boolean
  expected: Sm10TerminationClass
}>

export const FULL_AUTO_SOAK_SCENARIOS: ReadonlyArray<SoakScenarioDefinition> = [
  {
    id: "clean_objective_completion",
    description: "three clean continuations, then the owner marks the objective complete",
    requiresCompressedClock: false,
    expected: "objective_complete",
  },
  {
    id: "owner_stop_mid_run",
    description: "two clean continuations, then an owner Stop through the control-API attribution",
    requiresCompressedClock: false,
    expected: "owner_stop",
  },
  {
    id: "cap_exhaustion",
    description: `${FULL_AUTO_MAX_CONTINUATIONS} clean continuations, then the continuation cap disables the loop`,
    requiresCompressedClock: false,
    expected: "cap",
  },
  {
    id: "account_exhausted_rotation",
    description: "account exhaustion mid-run on the bound lane rotates to the next admitted candidate (#8987) and the run still completes",
    requiresCompressedClock: false,
    expected: "objective_complete",
  },
  {
    id: "account_exhausted_no_alternate",
    description: "account exhaustion with no routing policy consumes FA-H5 budget until the failure limit disables the loop",
    requiresCompressedClock: true,
    expected: "fa_h5_disable",
  },
  {
    id: "provider_error_fa_h5_disable",
    description: "persistent provider errors through backoff until the FA-H5 failure limit disables the loop",
    requiresCompressedClock: true,
    expected: "fa_h5_disable",
  },
  {
    id: "provider_error_transient_recovery",
    description: "two provider errors enter backoff/Retrying, the provider recovers, and the run completes",
    requiresCompressedClock: true,
    expected: "objective_complete",
  },
  {
    id: "app_restart_mid_run",
    description: "cold app restart mid-run (every durable store reopened), startup reconciliation resumes, run completes",
    requiresCompressedClock: false,
    expected: "objective_complete",
  },
  {
    id: "cache_pressure",
    description: "real 5-slot thread-cache eviction pressure between continuations (#8989 shape); run stays addressable and completes",
    requiresCompressedClock: false,
    expected: "objective_complete",
  },
  {
    id: "workspace_drift_block",
    description: "the resolved workspace drifts mid-run; FA-H2 blocks typed (workspace_guard), never redirects",
    requiresCompressedClock: false,
    expected: "guardrail_policy_block",
  },
]

/** Well above every FA-H5 backoff step reachable before the failure limit
 * (2^4 * 30s = 8min) -- one compressed jump per retry pass. */
const BACKOFF_JUMP_MS = 10 * 60_000
const CYCLE_GAP_MS = 1_000

const ACCOUNT_EXHAUSTED_FAULT: SoakDispatchOutcome = {
  kind: "lane_failure",
  reason: "budget_exceeded",
  detail: "Codex account usage limit reached for this window.",
}
const PROVIDER_ERROR_FAULT: SoakDispatchOutcome = {
  kind: "lane_failure",
  reason: "session_failed",
  detail: "Provider stream crashed mid-turn (soak synthetic provider incident).",
}

// -----------------------------------------------------------------------
// Per-scenario execution.
// -----------------------------------------------------------------------

export type FullAutoSoakRunResult = Readonly<{
  scenario: SoakScenarioId
  expected: Sm10TerminationClass
  clockMode: SoakClockMode
  runRef: string
  threadRef: string
  state: FullAutoRunState
  stopAttribution: FullAutoRunActor | null
  classification: Sm10TerminationClass
  /** Successful continuation dispatches recorded on the run. */
  continuations: number
  dispatchFailures: number
  rotations: number
  restarts: number
  reportRevision: number
  report: FullAutoRunReport
  events: SoakEvents
}>

/** Real 2ms wall-clock tick: the thread store keeps its own real-time
 * recency ordering regardless of the harness clock (same rationale as
 * FA-RUN-02 / FA-PRESS-01). */
const tickReal = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 2))

const pressureNote = (label: string, at: string): DesktopMessage => ({
  key: `pressure-${label}`,
  role: "user",
  text: `pressure follow-up ${label}`,
  timestamp: at,
})

export const runFullAutoSoakScenario = async (
  scenario: SoakScenarioDefinition,
  context: Readonly<{ root: string; clock: SoakClock }>,
): Promise<FullAutoSoakRunResult> => {
  const { clock } = context
  if (scenario.requiresCompressedClock && clock.mode !== "compressed") {
    throw new Error(
      `soak scenario ${scenario.id} requires the compressed clock (FA-H5 backoff windows); refusing a realtime run`,
    )
  }
  const events = makeSoakEvents()
  const faults = makeSoakFaultQueue()
  let workspaceRef = SOAK_WORKSPACE_REF
  const open = (): SoakRuntime =>
    openSoakRuntime({
      root: context.root,
      clock,
      faults,
      events,
      resolveWorkspaceRef: () => workspaceRef,
    })
  let runtime = open()

  // The rotation scenario binds a #8987 routing policy validated through the
  // REAL admission gate (fixture lane gate: both built-in Full Auto lanes
  // admitted and fullAuto-capable).
  let routingPolicy: ReadonlyArray<FullAutoRoutingCandidate> | null = null
  if (scenario.id === "account_exhausted_rotation") {
    const validation = validateFullAutoRoutingPolicy(
      [{ lane: SOAK_DEFAULT_LANE }, { lane: SOAK_ALTERNATE_LANE }],
      laneRef =>
        laneRef === SOAK_DEFAULT_LANE || laneRef === SOAK_ALTERNATE_LANE
          ? { admitted: true, fullAuto: true }
          : null,
    )
    if (!validation.ok) {
      throw new Error(`soak harness: routing policy failed validation (${validation.reason})`)
    }
    routingPolicy = validation.policy
  }

  const thread = runtime.store.newThread(`Soak ${scenario.id}`)
  const started = runtime.runRegistry.startNew({
    title: `Soak ${scenario.id}`,
    objective: `FA-SOAK-01 ${scenario.id}: drive the synthetic fixture lane through the scripted fault matrix to a typed terminal state.`,
    doneCondition: `The run reaches the expected typed terminal class: ${scenario.expected}.`,
    objectiveSource: "control_caller",
    workspaceRef,
    profile: { lane: SOAK_DEFAULT_LANE },
    threadRef: thread.id,
    actor: "control_api",
    reason: `FA-SOAK-01 ${scenario.id}: soak population run started.`,
  })
  if (!started.ok) throw new Error(`soak harness: run could not start (${started.reason})`)
  const runRef = started.run.runRef
  runtime.registry.set(thread.id, true, {
    workspaceRef,
    profile: { lane: SOAK_DEFAULT_LANE },
    ...(routingPolicy === null ? {} : { routingPolicy }),
  })

  /** One continuation cycle: advance the clock, reconcile through the real
   * engine, mirror the attempt onto the run record (the acceptance driver's
   * convention), then settle liveness and sync the report. */
  const cycle = async (options?: Readonly<{ advanceMs?: number; startup?: boolean }>): Promise<void> => {
    await clock.advance(options?.advanceMs ?? CYCLE_GAP_MS)
    const failuresBefore = events.dispatchFailures.length
    const dispatched = await runtime.reconcile(
      options?.startup === true ? { startup: true } : undefined,
    )
    if (dispatched.includes(thread.id)) {
      runtime.runRegistry.recordAttempt(runRef, "success")
    } else if (events.dispatchFailures.length > failuresBefore) {
      const lastFailure = events.dispatchFailures.at(-1)!
      runtime.runRegistry.recordAttempt(runRef, "failure", { reason: lastFailure.reason })
    }
    runtime.settleAndSync(runRef)
  }

  /** Terminal owner ceremony: transition the RUN first (so the thread-state
   * sync never sees an unexplained external disable), then release the
   * thread-level gate with matching attribution. */
  const stopAsOwner = (
    to: "completed" | "stopped",
    actor: "owner_ui" | "control_api",
    reason: string,
  ): void => {
    const transitioned = runtime.runRegistry.transition(runRef, { to, actor, reason })
    if (!transitioned.ok) {
      throw new Error(`soak harness: owner ${to} transition refused (${transitioned.reason})`)
    }
    runtime.registry.set(thread.id, false, {
      disabledBy: actor === "owner_ui" ? "ui_toggle" : "control_api",
    })
    runtime.settleAndSync(runRef)
  }

  switch (scenario.id) {
    case "clean_objective_completion": {
      for (let index = 0; index < 3; index += 1) await cycle()
      stopAsOwner("completed", "owner_ui", "All three scripted packets complete; done condition met.")
      break
    }
    case "owner_stop_mid_run": {
      for (let index = 0; index < 2; index += 1) await cycle()
      stopAsOwner("stopped", "control_api", "Owner pressed Stop on the run control surface mid-run.")
      break
    }
    case "cap_exhaustion": {
      for (let index = 0; index < FULL_AUTO_MAX_CONTINUATIONS; index += 1) await cycle()
      // The pass after the cap: reconcile disables with continuation_cap and
      // the settle pass carries the run to cap_reached, both typed.
      await cycle()
      break
    }
    case "account_exhausted_rotation": {
      await cycle()
      faults.push(SOAK_DEFAULT_LANE, ACCOUNT_EXHAUSTED_FAULT)
      // This pass fails typed on the bound lane, rotates to the alternate
      // candidate IN THE SAME PASS (#8987), and succeeds there.
      await cycle()
      await cycle()
      stopAsOwner("completed", "owner_ui", "Rotation survived account exhaustion; done condition met.")
      break
    }
    case "account_exhausted_no_alternate": {
      await cycle()
      faults.push(
        SOAK_DEFAULT_LANE,
        ...Array.from({ length: 5 }, () => ACCOUNT_EXHAUSTED_FAULT),
      )
      for (let index = 0; index < 5; index += 1) await cycle({ advanceMs: BACKOFF_JUMP_MS })
      break
    }
    case "provider_error_fa_h5_disable": {
      await cycle()
      faults.push(
        SOAK_DEFAULT_LANE,
        ...Array.from({ length: 5 }, () => PROVIDER_ERROR_FAULT),
      )
      for (let index = 0; index < 5; index += 1) await cycle({ advanceMs: BACKOFF_JUMP_MS })
      break
    }
    case "provider_error_transient_recovery": {
      await cycle()
      faults.push(SOAK_DEFAULT_LANE, PROVIDER_ERROR_FAULT, PROVIDER_ERROR_FAULT)
      await cycle({ advanceMs: BACKOFF_JUMP_MS })
      await cycle({ advanceMs: BACKOFF_JUMP_MS })
      // Recovery: the queue is exhausted, so the next backoff-eligible pass
      // completes and clears FA-H5 state.
      await cycle({ advanceMs: BACKOFF_JUMP_MS })
      await cycle()
      stopAsOwner("completed", "owner_ui", "Provider recovered after transient errors; done condition met.")
      break
    }
    case "app_restart_mid_run": {
      await cycle()
      // Cold restart: every durable file reopened by a fresh runtime.
      runtime = open()
      events.restarts += 1
      const resumed = runtime.runRegistry.findByThreadRef(thread.id)
      if (resumed === null || resumed.runRef !== runRef) {
        throw new Error("soak harness: the restarted runtime did not resume the same durable run")
      }
      await cycle({ startup: true })
      await cycle()
      stopAsOwner("completed", "owner_ui", "All scripted packets complete across the restart; done condition met.")
      break
    }
    case "cache_pressure": {
      const allPressureIds: Array<string> = []
      const pressureBatch = async (labels: ReadonlyArray<string>): Promise<void> => {
        for (const label of labels) {
          const created = runtime.store.newThread(`Pressure ${label}`)
          allPressureIds.push(created.id)
          await tickReal()
          runtime.store.append(created.id, pressureNote(label, clock.now().toISOString()))
          await tickReal()
          for (const id of allPressureIds) void runtime.store.open(id)
          void runtime.store.list()
        }
      }
      await cycle()
      await tickReal()
      await pressureBatch(["p01", "p02", "p03", "p04"])
      await cycle()
      await tickReal()
      await pressureBatch(["p05", "p06", "p07", "p08"])
      await cycle()
      stopAsOwner("completed", "owner_ui", "All scripted packets complete under real cache pressure; done condition met.")
      break
    }
    case "workspace_drift_block": {
      await cycle()
      await cycle()
      // FA-H2: the resolved workspace drifts away from the granted one.
      workspaceRef = "/granted/other-workspace"
      await cycle()
      break
    }
  }

  const final = runtime.settleAndSync(runRef)
  const report = final.report
  const stopAttribution = report.stopAttribution ?? null
  return {
    scenario: scenario.id,
    expected: scenario.expected,
    clockMode: clock.mode,
    runRef,
    threadRef: thread.id,
    state: report.state,
    stopAttribution,
    classification: classifySm10Termination({ state: report.state, stopAttribution }),
    continuations: report.successfulAttempts,
    dispatchFailures: events.dispatchFailures.length,
    rotations: events.rotations.length,
    restarts: events.restarts,
    reportRevision: report.reportRevision,
    report,
    events,
  }
}

// -----------------------------------------------------------------------
// Matrix runner.
// -----------------------------------------------------------------------

export const runFullAutoSoakMatrix = async (input: Readonly<{
  scenarios?: ReadonlyArray<SoakScenarioDefinition>
  makeRoot: (scenarioId: SoakScenarioId) => string
  makeClock: (scenario: SoakScenarioDefinition) => SoakClock
  now?: () => Date
}>): Promise<Readonly<{
  results: ReadonlyArray<FullAutoSoakRunResult>
  summary: FullAutoSoakSm10Summary
}>> => {
  const scenarios = input.scenarios ?? FULL_AUTO_SOAK_SCENARIOS
  const results: Array<FullAutoSoakRunResult> = []
  for (const scenario of scenarios) {
    results.push(
      await runFullAutoSoakScenario(scenario, {
        root: input.makeRoot(scenario.id),
        clock: input.makeClock(scenario),
      }),
    )
  }
  const modes = new Set(results.map(result => result.clockMode))
  const summary = buildFullAutoSoakSm10Summary(results, {
    clockMode: modes.size === 1 ? [...modes][0]! : "mixed",
    generatedAt: (input.now ?? (() => new Date()))().toISOString(),
  })
  return { results, summary }
}
