import path from "node:path"
import { randomUUID } from "node:crypto"

import {
  EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
  acceptanceTitleWithDisposition,
  evaluateFullAutoAcceptance,
  type FullAutoAcceptanceEvidence,
  type FullAutoAcceptanceIdentity,
  type FullAutoAcceptanceTestDefinition,
  type FullAutoAcceptanceVerdict,
} from "./full-auto-acceptance.ts"
import {
  buildProviderHandoffEnvelope,
  openProviderHandoffRegistry,
  providerHandoffDispositionForEnvelope,
  type ProviderHandoffEnvelope,
  type ProviderHandoffRegistry,
} from "./full-auto-provider-handoff.ts"
import { openFullAutoRegistry, type FullAutoRegistry } from "./full-auto-registry.ts"
import { reconcileFullAutoThreads } from "./full-auto-reconcile.ts"
import { analyzeFullAutoRunReport, type FullAutoRunAnalysis } from "./full-auto-run-analyzer.ts"
import {
  openFullAutoRunRegistry,
  type FullAutoRun,
  type FullAutoRunRegistry,
} from "./full-auto-run-registry.ts"
import {
  openFullAutoRunReportStore,
  type FullAutoRunReport,
  type FullAutoRunReportStore,
} from "./full-auto-run-report.ts"
import { openLocalTurnJournal, type LocalTurnJournal } from "./local-turn-journal.ts"
import { makeThreadStore } from "./thread-store.ts"
import {
  makeProviderLaneRegistry,
  type ProviderLaneRegistry,
  type ProviderLaneRegistryEntry,
} from "./provider-lane-registry.ts"
import type { ProviderLaneHistoryMessage } from "./provider-lane.ts"

/**
 * FA-QA-01 (#8976): the headless execution driver for the six typed
 * acceptance tests defined in full-auto-acceptance.ts.
 *
 * What is REAL here (the exact production seams, not stand-ins):
 * - thread creation/titles/notes: `makeThreadStore` (the real bounded
 *   mutable-composer cache, including its 5-slot eviction pressure);
 * - the lane switch: `makeProviderLaneRegistry().switchThread` -- the same
 *   host-owned bounded-history handoff seam the composer uses, including its
 *   admission/auth/capability refusals and the
 *   32-message/64,000-char projection;
 * - the handoff envelope: `buildProviderHandoffEnvelope` (FA-HO-01 #8975);
 * - durable transition receipts: `openProviderHandoffRegistry`;
 * - continuation scheduling: `reconcileFullAutoThreads` + the durable
 *   `openFullAutoRegistry` lease/cap/backoff machinery;
 * - run lifecycle: `openFullAutoRunRegistry`; turn records:
 *   `openLocalTurnJournal`; report + analyzer:
 *   `openFullAutoRunReportStore.sync` + `analyzeFullAutoRunReport`.
 *
 * What is FIXTURE here: only the PROVIDER (the model that would answer). The
 * `AcceptanceLaneExecutor` seam is where a later owner-armed run plugs the
 * real codex-local/claude-local lanes in; `makeFixtureLaneExecutor` is a
 * deterministic stand-in whose target-side behavior depends ONLY on the
 * history the real handoff seam handed it -- so a broken handoff genuinely
 * fails the marker-retention rule instead of being papered over.
 *
 * Restart boundaries (TEST 05) follow the exact convention of
 * tests/full-auto-restart.e2e.test.ts and local-turn-restart.e2e.test.ts:
 * "quit + relaunch" is a fresh re-open of the same durable files, because
 * that is precisely what a real relaunch does with them.
 */

// -----------------------------------------------------------------------
// Provider seam.
// -----------------------------------------------------------------------

export type AcceptanceLaneExecutorInput = Readonly<{
  laneRef: string
  prompt: string
  /** The ONLY context the executor may consult -- whatever the host-owned
   * handoff/projection seam supplied. Never the thread store directly. */
  history: ReadonlyArray<ProviderLaneHistoryMessage>
}>

export type AcceptanceLaneExecutorResult =
  | Readonly<{ ok: true; text: string }>
  | Readonly<{ ok: false; reason: string }>

export type AcceptanceLaneExecutor = (
  input: AcceptanceLaneExecutorInput,
) => Promise<AcceptanceLaneExecutorResult>

const MARKER_PATTERN = /\b([A-Z]+-\d+)\b/
const STEP_ONE_PATTERN = /STEP-ONE-RESULT\(([A-Z]+-\d+)\)/

/**
 * Deterministic fixture provider. Its marker behavior is intentionally
 * memory-free: when asked to state the marker it can ONLY find it in the
 * history it was handed, which is exactly what makes the marker-retention
 * pass rule bite -- an environment that drops the handoff history produces a
 * truthful "I do not have the marker" and the test FAILS.
 */
export const makeFixtureLaneExecutor = (): AcceptanceLaneExecutor => async input => {
  const establish = /establish marker ([A-Z]+-\d+)/i.exec(input.prompt)
  if (establish !== null) {
    const marker = establish[1]!
    return {
      ok: true,
      text:
        `Marker ${marker} acknowledged. STEP-ONE-RESULT(${marker}): completed step one of the bounded two-step task.`,
    }
  }
  if (/state the marker/i.test(input.prompt)) {
    const joined = input.history.map(message => message.text).join("\n")
    const priorResult = STEP_ONE_PATTERN.exec(joined)
    const marker = priorResult?.[1] ?? MARKER_PATTERN.exec(joined)?.[1] ?? null
    if (marker === null || priorResult === null) {
      return {
        ok: true,
        text: "I do not have the marker; this conversation contains no prior marker or step-one result.",
      }
    }
    return {
      ok: true,
      text:
        `The marker is ${marker}. STEP-TWO-COMPLETE(${marker}): step two built directly on STEP-ONE-RESULT(${marker}).`,
    }
  }
  if (/continue full auto/i.test(input.prompt) || /full auto/i.test(input.prompt)) {
    return { ok: true, text: "Autonomous packet complete." }
  }
  return { ok: true, text: "Acknowledged." }
}

// -----------------------------------------------------------------------
// Harness: every durable store the tests touch, rooted in one directory so
// a "restart" is simply re-opening the same root.
// -----------------------------------------------------------------------

export type FullAutoAcceptanceHarness = Readonly<{
  root: string
  store: ReturnType<typeof makeThreadStore>
  laneRegistry: ProviderLaneRegistry
  handoffRegistry: ProviderHandoffRegistry
  fullAutoRegistry: FullAutoRegistry
  runRegistry: FullAutoRunRegistry
  reportStore: FullAutoRunReportStore
  journal: LocalTurnJournal
  lanes: ReadonlyArray<ProviderLaneRegistryEntry>
  workspaceRef: string
}>

const fixtureLaneEntry = (
  laneRef: string,
  provider: string,
  displayName: string,
): ProviderLaneRegistryEntry => ({
  laneRef,
  provider,
  profileRef: `${laneRef}.fixture-profile.v1`,
  configuration: "configured",
  authentication: "ready",
  admission: "admitted",
  reason: null,
  capabilities: {
    laneRef,
    provider,
    displayName,
    admission: "admitted",
    reason: null,
    models: ["fixture-model"],
    reasoningEfforts: [],
    permissionModes: ["owner_full"],
    approvals: "host_mediated",
    questions: true,
    skills: false,
    images: false,
    fullAuto: true,
    interrupt: true,
    queueFollowup: true,
    steerTurn: true,
    extensions: [],
    evidence: "experimental",
  },
})

export const FIXTURE_ACCEPTANCE_LANES: ReadonlyArray<ProviderLaneRegistryEntry> = [
  fixtureLaneEntry("codex-local", "codex", "Codex (fixture)"),
  fixtureLaneEntry("claude-local", "claude", "Claude (fixture)"),
]

export const openFullAutoAcceptanceHarness = async (
  root: string,
  options?: Readonly<{ workspaceRef?: string }>,
): Promise<FullAutoAcceptanceHarness> => {
  const workspaceRef = options?.workspaceRef ?? "/granted/acceptance-workspace"
  return {
    root,
    store: makeThreadStore(path.join(root, "threads.json")),
    laneRegistry: makeProviderLaneRegistry({ file: path.join(root, "provider-lanes.json") }),
    handoffRegistry: openProviderHandoffRegistry(path.join(root, "provider-handoffs.json")),
    fullAutoRegistry: openFullAutoRegistry(path.join(root, "full-auto", "registry.json")),
    runRegistry: openFullAutoRunRegistry(path.join(root, "full-auto", "runs.json")),
    reportStore: openFullAutoRunReportStore(path.join(root, "full-auto", "reports.json")),
    journal: openLocalTurnJournal(path.join(root, "local-turns.json")),
    lanes: FIXTURE_ACCEPTANCE_LANES,
    workspaceRef,
  }
}

// -----------------------------------------------------------------------
// Execution result.
// -----------------------------------------------------------------------

export type FullAutoAcceptanceExecution = Readonly<{
  definition: FullAutoAcceptanceTestDefinition
  identity: FullAutoAcceptanceIdentity
  evidence: FullAutoAcceptanceEvidence
  verdict: FullAutoAcceptanceVerdict
  threadRef: string | null
  /** The sidebar title AFTER the disposition prefix was applied (which
   * happens strictly after evidence evaluation). */
  finalTitle: string | null
  report: FullAutoRunReport | null
  analysis: FullAutoRunAnalysis | null
}>

export type ExecuteFullAutoAcceptanceInput = Readonly<{
  definition: FullAutoAcceptanceTestDefinition
  harness: FullAutoAcceptanceHarness
  executor: AcceptanceLaneExecutor
  identity: FullAutoAcceptanceIdentity
  /** TEST 05 only: re-opens every durable store from the same root, exactly
   * like an app relaunch. Required for `full_auto_restart`. */
  reopenHarness?: () => Promise<FullAutoAcceptanceHarness>
  /** Adversarial knob for the harness's own regression tests: drop the
   * host-owned handoff history before invoking the target lane, simulating a
   * broken handoff. The marker-retention rule MUST fail under this. */
  sabotage?: Readonly<{ dropHandoffHistory?: boolean }>
  now?: () => Date
}>

const timestamp = (now: () => Date): string => now().toISOString()

/** Real Date.now() resolution is 1ms; consecutive synchronous store writes
 * can tie on updatedAt and randomize eviction order (see FA-RUN-02). */
const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 2))

const appendUser = (
  harness: FullAutoAcceptanceHarness,
  threadRef: string,
  text: string,
  now: () => Date,
): void => {
  harness.store.append(threadRef, {
    key: `user-${randomUUID()}`,
    role: "user",
    text,
    timestamp: timestamp(now),
  })
}

const appendAssistant = (
  harness: FullAutoAcceptanceHarness,
  threadRef: string,
  text: string,
  lane: string,
  now: () => Date,
): void => {
  harness.store.append(threadRef, {
    key: `assistant-${randomUUID()}`,
    role: "assistant",
    text,
    timestamp: timestamp(now),
    meta: { lane },
  })
}

/** The visible in-thread provider-transition event (the durable receipt
 * lives in the handoff registry; this note is the sidebar-visible mirror). */
const appendTransitionNote = (
  harness: FullAutoAcceptanceHarness,
  threadRef: string,
  from: string,
  to: string,
  handoffRef: string,
  now: () => Date,
): void => {
  harness.store.append(threadRef, {
    key: `transition-${handoffRef}`,
    role: "system",
    text: `Provider transition: ${from} → ${to} (${handoffRef})`,
    timestamp: timestamp(now),
  })
}

const recordHandoff = (
  harness: FullAutoAcceptanceHarness,
  envelope: ProviderHandoffEnvelope,
): ReturnType<ProviderHandoffRegistry["record"]> =>
  harness.handoffRegistry.record({
    ...(envelope.runRef === undefined ? {} : { runRef: envelope.runRef }),
    ...(envelope.threadRef === undefined ? {} : { threadRef: envelope.threadRef }),
    from: envelope.sourceLaneRef,
    to: envelope.targetLaneRef,
    actor: envelope.actor,
    at: envelope.at,
    reason: envelope.reason,
    disposition: providerHandoffDispositionForEnvelope(envelope),
    truncated: envelope.contextTruncated,
    envelopeSchema: envelope.schema,
  })

// -----------------------------------------------------------------------
// TEST 01 / TEST 02: same-thread cross-provider context handoff.
// -----------------------------------------------------------------------

const executeHandoffContext = async (
  input: ExecuteFullAutoAcceptanceInput,
): Promise<FullAutoAcceptanceEvidence> => {
  const { definition, harness, executor } = input
  const now = input.now ?? (() => new Date())
  const marker = definition.marker!
  const targetLaneRef = definition.targetLaneRef!

  const thread = harness.store.newThread(definition.title)
  harness.laneRegistry.bind(thread.id, definition.sourceLaneRef)

  // Step one, on the source lane.
  const stepOneAsk =
    `Establish marker ${marker} and complete step one of the bounded two-step task.`
  appendUser(harness, thread.id, stepOneAsk, now)
  const stepOne = await executor({
    laneRef: definition.sourceLaneRef,
    prompt: stepOneAsk,
    history: [],
  })
  if (!stepOne.ok) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      blockedReason: `source lane ${definition.sourceLaneRef} unavailable: ${stepOne.reason}`,
    }
  }
  appendAssistant(harness, thread.id, stepOne.text, definition.sourceLaneRef, now)

  // The REAL lane-switch seam: bounded host-owned history handoff.
  const switched = harness.laneRegistry.switchThread({
    threadRef: thread.id,
    laneRef: targetLaneRef,
    lanes: harness.lanes,
    thread: harness.store.open(thread.id),
  })
  if (!switched.ok) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      markerEstablishedInSource: stepOne.text.includes(marker),
      blockedReason: `lane switch refused (${switched.reason}): ${switched.message}`,
    }
  }

  // Durable transition receipt + the visible in-thread event.
  const envelope = buildProviderHandoffEnvelope({
    run: null,
    sourceLaneRef: definition.sourceLaneRef,
    targetLaneRef,
    thread: harness.store.open(thread.id),
    reason: `FA-QA-01 ${definition.id}: scripted provider switch.`,
    actor: "owner_ui",
    at: timestamp(now),
  })
  const transition = recordHandoff(harness, envelope)
  appendTransitionNote(
    harness, thread.id, definition.sourceLaneRef, targetLaneRef, transition.handoffRef, now,
  )

  // Step two, on the target lane -- context comes ONLY from the handoff.
  const stepTwoAsk = "State the marker and perform step two using the prior result."
  appendUser(harness, thread.id, stepTwoAsk, now)
  const handedHistory = input.sabotage?.dropHandoffHistory === true ? [] : switched.history
  const stepTwo = await executor({
    laneRef: targetLaneRef,
    prompt: stepTwoAsk,
    history: handedHistory,
  })
  if (!stepTwo.ok) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      markerEstablishedInSource: stepOne.text.includes(marker),
      transitions: harness.handoffRegistry.list({ threadRef: thread.id }),
      blockedReason: `target lane ${targetLaneRef} unavailable: ${stepTwo.reason}`,
    }
  }
  appendAssistant(harness, thread.id, stepTwo.text, targetLaneRef, now)

  return {
    ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
    threadRef: thread.id,
    threadRefsTouched: [thread.id],
    markerEstablishedInSource: stepOne.text.includes(marker),
    markerStatedByTarget: stepTwo.text.includes(`The marker is ${marker}`),
    targetMarkerStatement: stepTwo.text,
    stepTwoUsedPriorResult:
      stepTwo.text.includes(`STEP-TWO-COMPLETE(${marker})`) &&
      stepTwo.text.includes(`STEP-ONE-RESULT(${marker})`),
    hiddenRepairCount: 0,
    transitions: harness.handoffRegistry.list({ threadRef: thread.id }),
  }
}

// -----------------------------------------------------------------------
// TEST 03: objective retention under host-projection pressure.
// -----------------------------------------------------------------------

const executeHandoffObjective = async (
  input: ExecuteFullAutoAcceptanceInput,
): Promise<FullAutoAcceptanceEvidence> => {
  const { definition, harness } = input
  const now = input.now ?? (() => new Date())
  const targetLaneRef = definition.targetLaneRef!

  const thread = harness.store.newThread(definition.title)
  harness.laneRegistry.bind(thread.id, definition.sourceLaneRef)

  const started = harness.runRegistry.startNew({
    title: definition.title,
    objective: definition.objective!,
    doneCondition: definition.acceptanceRule!,
    objectiveSource: "user",
    workspaceRef: harness.workspaceRef,
    profile: { lane: definition.sourceLaneRef },
    threadRef: thread.id,
    actor: "owner_ui",
    reason: `FA-QA-01 ${definition.id}: run started with explicit objective and acceptance rule.`,
  })
  if (!started.ok) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      blockedReason: `run could not start: ${started.reason}`,
    }
  }
  const run = started.run

  // Bounded tool/system pressure: enough activity that the shared
  // 32-message host projection MUST truncate.
  appendUser(harness, thread.id, `Objective: ${definition.objective}`, now)
  appendUser(harness, thread.id, `Acceptance rule: ${definition.acceptanceRule}`, now)
  for (let index = 0; index < 40; index += 1) {
    harness.store.append(thread.id, {
      key: `pressure-${index}`,
      role: "system",
      text: `Tool activity ${index}: bounded scripted output pressuring the host projection.`,
      timestamp: timestamp(now),
    })
  }

  // Handoff requires a paused run (run_not_paused discipline).
  const paused = harness.runRegistry.transition(run.runRef, {
    to: "paused",
    actor: "owner_ui",
    reason: "Pausing for the scripted provider switch.",
  })
  if (!paused.ok) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      blockedReason: `run could not pause before the handoff: ${paused.reason}`,
    }
  }

  const switched = harness.laneRegistry.switchThread({
    threadRef: thread.id,
    laneRef: targetLaneRef,
    lanes: harness.lanes,
    thread: harness.store.open(thread.id),
  })
  if (!switched.ok) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      blockedReason: `lane switch refused (${switched.reason}): ${switched.message}`,
    }
  }

  const envelope = buildProviderHandoffEnvelope({
    run: paused.run,
    sourceLaneRef: definition.sourceLaneRef,
    targetLaneRef,
    thread: harness.store.open(thread.id),
    reason: `FA-QA-01 ${definition.id}: scripted objective-retention switch.`,
    actor: "owner_ui",
    at: timestamp(now),
  })
  const transition = recordHandoff(harness, envelope)
  appendTransitionNote(
    harness, thread.id, definition.sourceLaneRef, targetLaneRef, transition.handoffRef, now,
  )
  harness.runRegistry.rebindProfile(run.runRef, { lane: targetLaneRef })

  const truncationOmission = envelope.omissions.some(
    omission => omission.reason === "bounded_truncation",
  )
  const recordedDisposition = transition.disposition

  return {
    ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
    threadRef: thread.id,
    threadRefsTouched: [thread.id],
    hiddenRepairCount: 0,
    transitions: harness.handoffRegistry.list({ threadRef: thread.id }),
    // The priority channel is the run's durable objective/doneCondition --
    // delivered means the ENVELOPE carries them verbatim, independent of
    // whatever recent-message truncation did to the transcript projection.
    objectiveDeliveredToTarget: envelope.objective === definition.objective,
    acceptanceRuleDeliveredToTarget: envelope.doneCondition === definition.acceptanceRule,
    contextTruncated: envelope.contextTruncated,
    truncationAcknowledged: truncationOmission,
    truncationConfirmationRecorded: envelope.contextTruncated
      ? recordedDisposition === "truncated_with_confirmation"
      : false,
    initialRunRef: run.runRef,
    resumedRunRef: run.runRef,
    runFieldsContinuous: true,
  }
}

// -----------------------------------------------------------------------
// TEST 04 / 05 / 06 shared continuation machinery.
// -----------------------------------------------------------------------

type ContinuationCycleResult = Readonly<{ dispatchInvocations: number; dispatchedThreads: ReadonlyArray<string> }>

const runContinuationCycle = async (
  harness: FullAutoAcceptanceHarness,
  executor: AcceptanceLaneExecutor,
  laneRef: string,
  options?: Readonly<{ clearStaleLeases?: boolean; concurrentPasses?: number }>,
): Promise<ContinuationCycleResult> => {
  let dispatchInvocations = 0
  const dispatch = async (dispatchInput: Readonly<{ threadRef: string; turnRef: string; message: string }>) => {
    dispatchInvocations += 1
    // The exact fail-closed check production's dispatchTurn performs.
    const thread = harness.store.open(dispatchInput.threadRef)
    if (thread === null) return { ok: false, reason: "That conversation no longer exists." }
    const accepted = harness.journal.accept({
      threadRef: dispatchInput.threadRef,
      turnRef: dispatchInput.turnRef,
      lane: laneRef,
      userMessageKey: `${dispatchInput.turnRef}-user`,
      assistantMessageKey: `${dispatchInput.turnRef}-assistant`,
    })
    if (!accepted.accepted) return { ok: false, reason: "turn not accepted by the journal" }
    const result = await executor({ laneRef, prompt: dispatchInput.message, history: [] })
    if (!result.ok) {
      harness.journal.terminal(
        { threadRef: dispatchInput.threadRef, turnRef: dispatchInput.turnRef, lane: laneRef },
        "failed",
        "failed",
      )
      return { ok: false, reason: result.reason }
    }
    harness.journal.setAssistantText(
      { threadRef: dispatchInput.threadRef, turnRef: dispatchInput.turnRef, lane: laneRef },
      result.text,
    )
    harness.journal.terminal(
      { threadRef: dispatchInput.threadRef, turnRef: dispatchInput.turnRef, lane: laneRef },
      "completed",
      "completed",
    )
    harness.store.append(dispatchInput.threadRef, {
      key: `${dispatchInput.turnRef}-assistant`,
      role: "assistant",
      text: result.text,
      timestamp: new Date().toISOString(),
      meta: { lane: laneRef, turnRef: dispatchInput.turnRef },
    })
    return { ok: true }
  }
  const reconcileOnce = () => reconcileFullAutoThreads({
    registry: harness.fullAutoRegistry,
    nonterminalThreadRefs: () => new Set<string>(),
    resolveWorkspaceRef: () => harness.workspaceRef,
    journalHasNonterminalTurn: turnRef =>
      harness.journal.nonterminal().some(record => record.turnRef === turnRef),
    ...(options?.clearStaleLeases === true ? { clearStaleLeases: true } : {}),
    dispatch,
  })
  const passes = Math.max(1, options?.concurrentPasses ?? 1)
  const settled = await Promise.all(Array.from({ length: passes }, () => reconcileOnce()))
  return { dispatchInvocations, dispatchedThreads: settled.flat() }
}

const syncReportAndAnalyze = (
  harness: FullAutoAcceptanceHarness,
  runRef: string,
): Readonly<{ report: FullAutoRunReport | null; analysis: FullAutoRunAnalysis | null }> => {
  const run = harness.runRegistry.get(runRef)
  if (run === null) return { report: null, analysis: null }
  const report = harness.reportStore.sync({
    run,
    turns: harness.journal.list().filter(record => record.threadRef === run.threadRef),
    handoffs: harness.handoffRegistry.list({ runRef }),
  })
  return { report, analysis: analyzeFullAutoRunReport(report) }
}

const startAcceptanceRun = (
  harness: FullAutoAcceptanceHarness,
  definition: FullAutoAcceptanceTestDefinition,
  threadRef: string,
): FullAutoRun | null => {
  const started = harness.runRegistry.startNew({
    title: definition.title,
    objective: definition.objective!,
    doneCondition: definition.acceptanceRule!,
    objectiveSource: "user",
    workspaceRef: harness.workspaceRef,
    profile: { lane: definition.sourceLaneRef },
    ...(definition.plannedTurns === null ? {} : { turnCap: definition.plannedTurns }),
    threadRef,
    actor: "owner_ui",
    reason: `FA-QA-01 ${definition.id}: launched from the dedicated Full Auto action.`,
  })
  return started.ok ? started.run : null
}

// -----------------------------------------------------------------------
// TEST 04: three autonomous turns, no manual message between them.
// -----------------------------------------------------------------------

type FullAutoRunExtras = Readonly<{
  runRef: string | null
  report: FullAutoRunReport | null
  analysis: FullAutoRunAnalysis | null
}>

const executeFullAutoTurns = async (
  input: ExecuteFullAutoAcceptanceInput,
): Promise<FullAutoAcceptanceEvidence & FullAutoRunExtras> => {
  const { definition, harness, executor } = input
  const laneRef = definition.sourceLaneRef
  const turns = definition.plannedTurns ?? 3

  const thread = harness.store.newThread(definition.title)
  const run = startAcceptanceRun(harness, definition, thread.id)
  if (run === null) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      blockedReason: "run could not start (active run conflict)",
      runRef: null,
      report: null,
      analysis: null,
    }
  }
  harness.fullAutoRegistry.set(thread.id, true, {
    workspaceRef: harness.workspaceRef,
    profile: { lane: laneRef },
  })

  const cycleCounts: number[] = []
  for (let cycle = 0; cycle < turns; cycle += 1) {
    const result = await runContinuationCycle(harness, executor, laneRef)
    cycleCounts.push(result.dispatchInvocations)
    harness.runRegistry.recordAttempt(run.runRef, "success")
    await tick()
  }

  // Read the count BEFORE the toggle-off: disabling durably resets it.
  const continuationCount = harness.fullAutoRegistry.record(thread.id)?.continuationCount ?? 0
  harness.fullAutoRegistry.set(thread.id, false, { disabledBy: "ui_toggle" })
  harness.runRegistry.transition(run.runRef, {
    to: "completed",
    actor: "owner_ui",
    reason: `All ${turns} scripted packets complete; done condition met.`,
  })
  const { report, analysis } = syncReportAndAnalyze(harness, run.runRef)
  const finalRun = harness.runRegistry.get(run.runRef)

  return {
    ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
    threadRef: thread.id,
    threadRefsTouched: [thread.id],
    autonomousTurnsCompleted: continuationCount,
    manualMessagesBetweenTurns: 0,
    duplicateDispatchCount: cycleCounts.reduce((total, count) => total + count, 0) - continuationCount,
    continuationDispatchCounts: cycleCounts,
    reportPresent: report !== null,
    analysisPresent: analysis !== null,
    finalStateReason: finalRun?.terminalReason ?? null,
    initialRunRef: run.runRef,
    resumedRunRef: run.runRef,
    runFieldsContinuous: true,
    runRef: run.runRef,
    report,
    analysis,
  }
}

// -----------------------------------------------------------------------
// TEST 05: restart continuity -- one turn, relaunch, two more turns.
// -----------------------------------------------------------------------

const executeFullAutoRestart = async (
  input: ExecuteFullAutoAcceptanceInput,
): Promise<FullAutoAcceptanceEvidence & FullAutoRunExtras> => {
  const { definition, harness, executor } = input
  const laneRef = definition.sourceLaneRef
  if (input.reopenHarness === undefined) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      blockedReason: "full_auto_restart requires reopenHarness (the relaunch seam)",
      runRef: null,
      report: null,
      analysis: null,
    }
  }

  // Runtime A: launch, one autonomous turn, then "quit".
  const thread = harness.store.newThread(definition.title)
  const run = startAcceptanceRun(harness, definition, thread.id)
  if (run === null) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      blockedReason: "run could not start (active run conflict)",
      runRef: null,
      report: null,
      analysis: null,
    }
  }
  harness.fullAutoRegistry.set(thread.id, true, {
    workspaceRef: harness.workspaceRef,
    profile: { lane: laneRef },
  })
  const cycleCounts: number[] = []
  const first = await runContinuationCycle(harness, executor, laneRef)
  cycleCounts.push(first.dispatchInvocations)
  harness.runRegistry.recordAttempt(run.runRef, "success")
  const preRestart = syncReportAndAnalyze(harness, run.runRef)
  const preRestartRevision = preRestart.report?.reportRevision ?? 0
  await tick()

  // Runtime B: a fresh process re-opening the same durable files.
  const reopened = await input.reopenHarness()
  const resumedRun = reopened.runRegistry.findByThreadRef(thread.id)
  // Cycle 2 races the startup trigger against a turn-completion trigger,
  // exactly like main.ts's two call sites can after a relaunch.
  const second = await runContinuationCycle(reopened, executor, laneRef, {
    clearStaleLeases: true,
    concurrentPasses: 2,
  })
  cycleCounts.push(second.dispatchInvocations)
  if (resumedRun !== null) reopened.runRegistry.recordAttempt(resumedRun.runRef, "success")
  await tick()
  const third = await runContinuationCycle(reopened, executor, laneRef)
  cycleCounts.push(third.dispatchInvocations)
  if (resumedRun !== null) reopened.runRegistry.recordAttempt(resumedRun.runRef, "success")

  // Read the count BEFORE the toggle-off: disabling durably resets it.
  const continuationCount = reopened.fullAutoRegistry.record(thread.id)?.continuationCount ?? 0
  reopened.fullAutoRegistry.set(thread.id, false, { disabledBy: "ui_toggle" })
  if (resumedRun !== null) {
    reopened.runRegistry.transition(resumedRun.runRef, {
      to: "completed",
      actor: "owner_ui",
      reason: "All three scripted packets complete across the restart; done condition met.",
    })
  }
  const post = resumedRun === null
    ? { report: null, analysis: null }
    : syncReportAndAnalyze(reopened, resumedRun.runRef)
  const finalRun = resumedRun === null ? null : reopened.runRegistry.get(resumedRun.runRef)

  const runFieldsContinuous = resumedRun !== null &&
    resumedRun.runRef === run.runRef &&
    resumedRun.objective === run.objective &&
    resumedRun.workspaceRef === run.workspaceRef &&
    resumedRun.profile?.lane === run.profile?.lane &&
    resumedRun.turnCap === run.turnCap

  return {
    ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
    threadRef: thread.id,
    threadRefsTouched: [thread.id],
    autonomousTurnsCompleted: continuationCount,
    manualMessagesBetweenTurns: 0,
    restartBoundariesObserved: 1,
    initialRunRef: run.runRef,
    resumedRunRef: resumedRun?.runRef ?? null,
    runFieldsContinuous,
    duplicateDispatchCount:
      cycleCounts.reduce((total, count) => total + count, 0) - continuationCount,
    continuationDispatchCounts: cycleCounts,
    reportPresent: post.report !== null,
    analysisPresent: post.analysis !== null,
    reportSpansRestart: post.report !== null &&
      post.report.runRef === run.runRef &&
      post.report.reportRevision > preRestartRevision &&
      reopened.reportStore.list().filter(report => report.runRef === run.runRef).length === 1,
    finalStateReason: finalRun?.terminalReason ?? null,
    runRef: run.runRef,
    report: post.report,
    analysis: post.analysis,
  }
}

// -----------------------------------------------------------------------
// TEST 06: thread pressure -- >5 other chats around a three-turn run.
// -----------------------------------------------------------------------

const executeThreadPressure = async (
  input: ExecuteFullAutoAcceptanceInput,
): Promise<FullAutoAcceptanceEvidence & FullAutoRunExtras> => {
  const { definition, harness, executor } = input
  const laneRef = definition.sourceLaneRef

  const thread = harness.store.newThread(definition.title)
  const run = startAcceptanceRun(harness, definition, thread.id)
  if (run === null) {
    return {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: thread.id,
      threadRefsTouched: [thread.id],
      blockedReason: "run could not start (active run conflict)",
      runRef: null,
      report: null,
      analysis: null,
    }
  }
  harness.fullAutoRegistry.set(thread.id, true, {
    workspaceRef: harness.workspaceRef,
    profile: { lane: laneRef },
  })
  await tick()

  const cycleCounts: number[] = []
  // Cycle 1 -- while its "first turn" completes, pressure begins.
  const first = await runContinuationCycle(harness, executor, laneRef)
  cycleCounts.push(first.dispatchInvocations)
  harness.runRegistry.recordAttempt(run.runRef, "success")
  await tick()
  harness.store.newThread("Pressure chat 1")
  await tick()
  harness.store.newThread("Pressure chat 2")
  await tick()

  // Cycle 2 -- re-touches the run thread, then four more chats arrive: six
  // distinct other chats now exist against the five-slot bounded cache.
  const second = await runContinuationCycle(harness, executor, laneRef)
  cycleCounts.push(second.dispatchInvocations)
  harness.runRegistry.recordAttempt(run.runRef, "success")
  await tick()
  for (const label of ["Pressure chat 3", "Pressure chat 4", "Pressure chat 5", "Pressure chat 6"]) {
    harness.store.newThread(label)
    await tick()
  }

  const addressable = harness.store.open(thread.id) !== null

  // Cycle 3 -- the continuation the overnight incident lost.
  const third = await runContinuationCycle(harness, executor, laneRef)
  cycleCounts.push(third.dispatchInvocations)
  harness.runRegistry.recordAttempt(run.runRef, "success")

  // Read the count BEFORE the toggle-off: disabling durably resets it.
  const continuationCount = harness.fullAutoRegistry.record(thread.id)?.continuationCount ?? 0
  harness.fullAutoRegistry.set(thread.id, false, { disabledBy: "ui_toggle" })
  harness.runRegistry.transition(run.runRef, {
    to: "completed",
    actor: "owner_ui",
    reason: "All three scripted packets complete under thread pressure; done condition met.",
  })
  const { report, analysis } = syncReportAndAnalyze(harness, run.runRef)
  const finalRun = harness.runRegistry.get(run.runRef)

  return {
    ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
    threadRef: thread.id,
    threadRefsTouched: [thread.id],
    autonomousTurnsCompleted: continuationCount,
    manualMessagesBetweenTurns: 0,
    otherChatsOpened: 6,
    threadAddressableUnderPressure: addressable && continuationCount >= 3,
    duplicateDispatchCount:
      cycleCounts.reduce((total, count) => total + count, 0) - continuationCount,
    continuationDispatchCounts: cycleCounts,
    reportPresent: report !== null,
    analysisPresent: analysis !== null,
    finalStateReason: finalRun?.terminalReason ?? null,
    initialRunRef: run.runRef,
    resumedRunRef: run.runRef,
    runFieldsContinuous: true,
    runRef: run.runRef,
    report,
    analysis,
  }
}

// -----------------------------------------------------------------------
// The single entry point.
// -----------------------------------------------------------------------

export const executeFullAutoAcceptanceTest = async (
  input: ExecuteFullAutoAcceptanceInput,
): Promise<FullAutoAcceptanceExecution> => {
  const { definition, harness } = input
  let evidence: FullAutoAcceptanceEvidence
  let report: FullAutoRunReport | null = null
  let analysis: FullAutoRunAnalysis | null = null
  switch (definition.kind) {
    case "handoff_context":
      evidence = await executeHandoffContext(input)
      break
    case "handoff_objective":
      evidence = await executeHandoffObjective(input)
      break
    case "full_auto_turns":
    case "full_auto_restart":
    case "thread_pressure": {
      const executed = definition.kind === "full_auto_turns"
        ? await executeFullAutoTurns(input)
        : definition.kind === "full_auto_restart"
          ? await executeFullAutoRestart(input)
          : await executeThreadPressure(input)
      const { runRef: _runRef, report: executedReport, analysis: executedAnalysis, ...rest } = executed
      report = executedReport
      analysis = executedAnalysis
      evidence = rest
      break
    }
  }

  // Evidence FIRST, verdict SECOND, title prefix LAST -- the issue's
  // ordering discipline is structural here, not a convention.
  const verdict = evaluateFullAutoAcceptance(definition, evidence)
  let finalTitle: string | null = null
  if (evidence.threadRef !== null) {
    // thread-store re-reads its file per operation, so this rename lands on
    // the CURRENT durable state even after a TEST 05 restart boundary.
    const renamed = harness.store.open(evidence.threadRef) !== null
      ? harness.store.rename(
          evidence.threadRef,
          acceptanceTitleWithDisposition(definition.title, verdict.disposition),
        )
      : null
    finalTitle = renamed?.title ??
      acceptanceTitleWithDisposition(definition.title, verdict.disposition)
  }

  return {
    definition,
    identity: input.identity,
    evidence,
    verdict,
    threadRef: evidence.threadRef,
    finalTitle,
    report,
    analysis,
  }
}
