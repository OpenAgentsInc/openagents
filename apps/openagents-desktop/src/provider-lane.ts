/**
 * Provider lane SPI (L1 #8899, epic #8898).
 *
 * One typed adapter interface every desktop agent lane implements, modeled on
 * T3 Code's ProviderAdapterShape/ProviderInstance pattern (drivers are plain
 * values behind one interface; everything above them is provider-agnostic).
 * The two existing lanes — codex-local and claude-local (`claude_agent`) — are
 * plain `ProviderLane` values constructed in main.ts; the ACP runtime bridge
 * lanes (#8891/#8892, Grok + Cursor) land as further implementations.
 *
 * STREAM ENVELOPE: the SPI's streaming envelope IS the frozen claude-local
 * event envelope (`ClaudeLocalEvent` / `ClaudeLocalEventEnvelopeSchema`) —
 * plan (`plan_updated`), reasoning (`reasoning`), tool/exec
 * (`tool_use`/`tool_progress`/`tool_result` + typed WorkbenchItem payloads),
 * approval/question (`question_pending`/`question_resolved`), item deltas
 * (`text_delta` + segment boundaries), and usage (`turn_completed.usage` /
 * `meter_updated`). This is deliberately NOT a third vocabulary: it is the
 * exact contract the renderer transcript components (epic #8857 T-series)
 * already render for both existing lanes, and the ACP runtime bridge's
 * canonical event vocabulary maps onto it explicitly — see
 * ./provider-lane-acp.ts for the typed mapping.
 *
 * SHARED DISPATCH ENGINE: `makeProviderLaneDispatcher` owns the plumbing the
 * two lanes previously duplicated in main.ts — content admission, thread
 * existence, the durable local-turn-journal accept/terminal lifecycle, the
 * user-note append, host-owned history assembly (the renderer can never
 * inject synthetic history), coalesced assistant-text persistence, the live
 * agent graph fold, pre/post-turn workspace checkpoints, exact usage-ledger
 * attribution, shared tool-trace/effective-model transcript notes, and the
 * renderer event forward. Lanes contribute only what is genuinely
 * lane-specific through the typed hooks below.
 *
 * RECOVERY CONTRACT: restart recovery mirrors the local-turn journal.
 * `accept` is the exactly-once admission (a duplicate turnRef is refused
 * typed); `recordDispatch`/`recordProviderSession` bind the account and the
 * provider-native session ref while streaming; `terminal` settles the
 * disposition. After a restart, `reconcileLocalTurns` replays only a lane
 * whose capability report declares `recovery: "provider_session_replay"`
 * (codex-local, via its durable provider thread id); every other lane —
 * including one that has never been hand-wired — fails closed to an honest
 * `interrupted_by_restart` disposition with an owner-visible notice.
 *
 * This module never imports `electron` (unit-testable under `pnpm vp test`);
 * the renderer sender is typed structurally.
 */
import { randomUUID } from "node:crypto"

import type { DesktopMessage, DesktopMessageMeta, DesktopThread } from "./chat-contract.ts"
import {
  CLAUDE_LOCAL_FINAL_TEXT_LIMIT,
  claudeLocalTraceNoteMeta,
  claudeLocalTraceNoteText,
  makeTranscriptOrderingBoundaryTracker,
  startRequestHasContent,
  type ClaudeChildUsage,
  type ClaudeLocalEvent,
  type ClaudeLocalFailureReason,
  type ClaudeLocalStartRequest,
} from "./claude-local-contract.ts"
import type { LocalTurnJournal } from "./local-turn-journal.ts"
import { makeLocalTurnTextPersistence } from "./local-turn-text-persistence.ts"
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./ide/portable-mutation-authority.ts"
import {
  appendSpecLaneContext,
  type SpecLaneTurnProjection,
} from "./spec-lane-workflow.ts"
import type { makeThreadStore } from "./thread-store.ts"
import {
  projectProviderLaneCapabilities,
  type ProviderLaneCapabilityReport,
} from "./provider-lane-capabilities.ts"

export type { ProviderLaneCapabilityReport } from "./provider-lane-capabilities.ts"

type ThreadStore = ReturnType<typeof makeThreadStore>

/** The renderer WebContents surface the dispatcher needs — structural so the
 * module stays electron-free and tests can capture forwarded envelopes. */
export type ProviderLaneEventSender = Readonly<{
  isDestroyed: () => boolean
  send: (channel: string, payload: unknown) => void
}>

export type ProviderLaneHistoryMessage = Readonly<{
  role: "user" | "assistant" | "system"
  text: string
}>

/**
 * Capability report (input to L2 capability negotiation): per-lane feature
 * truth for composer affordances. A lane must never advertise what it cannot
 * do — these values drive honest UI admission, not marketing.
 */
/** Typed admission verdict. `context` carries lane-private admission facts
 * (e.g. the resolved local skill) forward into `runTurn` without re-deriving
 * them, mirroring T3 Code's opaque driver-config envelope. */
export type ProviderLaneAdmission<Context> =
  | Readonly<{ ok: true; model: string; context: Context }>
  | Readonly<{ ok: false; error: string }>

export type ProviderLaneTurnSuccess = Readonly<{
  ok: true
  text: string
  totalTokens: number | null
  /** The account the turn actually ran on (a ref, never a path). */
  accountRef?: string
  /** Exact provider token split, when the provider reports one. */
  usage?: ClaudeChildUsage
  /** Provider-native session/thread continuity ref, when reported. */
  providerSessionRef?: string | null
}>
export type ProviderLaneTurnFailure = Readonly<{
  ok: false
  reason: ClaudeLocalFailureReason
  detail: string
}>
export type ProviderLaneTurnResult = ProviderLaneTurnSuccess | ProviderLaneTurnFailure

/** Facts available while a turn streams (meta hooks + the turn projector). */
export type ProviderLaneTurnContext<Context> = Readonly<{
  request: ClaudeLocalStartRequest
  requestedModel: string
  context: Context
  /** The provider-reported effective model so far (null before init). */
  effectiveModel: () => string | null
  store: ThreadStore
  timestamp: () => string
}>

export type ProviderLaneRunInput<Context> = Readonly<{
  request: ClaudeLocalStartRequest
  model: string
  context: Context
  /** Host-owned prior history (the just-appended user note excluded). */
  history: ReadonlyArray<ProviderLaneHistoryMessage>
  /** The prompt text actually sent to the model (images-only turns get a
   * neutral instruction; text turns keep the user's text verbatim). */
  message: string
  /** True when no renderer initiated the turn (a main-owned background
   * continuation) — such a turn has nothing that could answer a question. */
  background: boolean
  emit: (event: ClaudeLocalEvent) => void
}>

/**
 * The provider lane SPI. Each lane is a plain value implementing this
 * interface; the dispatcher owns everything else. `Context` is the lane's
 * private admission payload (use `null` when a lane needs none).
 */
export type ProviderLane<Context = null> = Readonly<{
  /** Durable journal lane ref — recorded on every local-turn record. */
  laneRef: string
  /** Live agent graph lane label for root turns on this lane. */
  graphLaneRef: string
  /** Renderer channel the lane's stream envelope forwards on. */
  eventChannel: string
  /** Usage-ledger provider attribution for completed turns. */
  usageProvider: string
  capabilities: () => ProviderLaneCapabilityReport
  /**
   * Typed request admission: provider/model targeting, lane feature refusals
   * (skills, plan-only, …), and lane-private context resolution. Runs before
   * any durable write; a refusal never reaches the journal.
   */
  admit: (request: ClaudeLocalStartRequest) => ProviderLaneAdmission<Context>
  /** Optional hook after thread-existence passes, before the journal accept
   * (codex-local binds the Full Auto execution profile here). Receives the
   * admitted model so the durable profile records spawn-config truth. */
  prepare?: (
    request: ClaudeLocalStartRequest,
    sender: ProviderLaneEventSender | null,
    model: string,
  ) => void
  /** Optional hook after the journal accepted the turn, before the user note
   * persists (codex-local binds the ProductSpec handoff turn here). */
  bound?: (request: ClaudeLocalStartRequest) => void
  /** Streaming assistant-note metadata (persisted by the text checkpointer). */
  streamMeta: (ctx: ProviderLaneTurnContext<Context>) => DesktopMessageMeta
  /** Lane-branded effective-model transcript caption. */
  modelNoteText: (model: string) => string
  /**
   * Thread-scoped turn dispatch. The lane owns provider execution, account
   * selection/rotation, and its own history/recovery semantics; it emits the
   * frozen envelope through `emit` and resolves with a typed result.
   */
  runTurn: (input: ProviderLaneRunInput<Context>) => Promise<ProviderLaneTurnResult>
  /** Abort a running turn by exact turnRef; false when none matched. */
  interrupt: (turnRef: string) => boolean
  /**
   * Optional per-turn projector factory for lane-specific durable projection
   * beyond the shared trace/model notes (claude-local: final plan card, child
   * usage attribution; codex-local: reasoning/lane-notice lines, structured
   * runtime-card persistence). Called once per dispatched turn; the returned
   * function runs for every stream event AFTER the shared projection and
   * BEFORE the renderer forward, so persisted-note ordering is stable.
   */
  makeTurnProjector?: (ctx: ProviderLaneTurnContext<Context>) => (event: ClaudeLocalEvent) => void
  /** Final assistant-note metadata for the completed turn. */
  finalMeta: (
    ctx: ProviderLaneTurnContext<Context> & Readonly<{
      result: ProviderLaneTurnSuccess
      durationMs: number
    }>,
  ) => DesktopMessageMeta
  /** Lane-branded renderer copy for a typed turn failure. */
  failureMessage: (reason: ClaudeLocalFailureReason, detail: string) => string
  /** Optional hook after a completed dispatch settled durably (codex-local
   * kicks the Full Auto reconciliation loop for flagged turns). */
  completed?: (request: ClaudeLocalStartRequest) => void
}>

export type ProviderLaneDispatchResult = Readonly<{
  ok: boolean
  thread?: DesktopThread | null
  error?: string
  /** Preserve the provider's typed terminal reason across the IPC boundary. */
  reason?: ClaudeLocalFailureReason
  /** Host-store ownership is not a provider session failure. Keep this
   * machine-readable so Full Auto liveness never infers from display copy. */
  failureCause?: "host_thread_missing"
}>

/** Host services the dispatcher folds every lane through — narrow structural
 * shapes so main.ts passes thin adapters and tests pass recorders. */
export type ProviderLaneDispatcherDeps = Readonly<{
  threads: () => ThreadStore
  journal: LocalTurnJournal
  liveAgentGraph: Readonly<{
    beginTurn: (input: Readonly<{ turnRef: string; threadRef: string; lane: string }>) => void
    applyEvent: (threadRef: string, envelope: Readonly<{ turnRef: string; event: ClaudeLocalEvent }>) => void
  }>
  usageLedger: Readonly<{
    record: (input: Readonly<{
      provider: string
      accountRef: string
      requestedModel: string | null
      kind: "turn"
      usage: ClaudeChildUsage | null
    }>) => void
  }>
  captureTurnCheckpoint: (
    threadRef: string,
    turnRef: string,
    phase: "turn_start" | "turn_completed",
  ) => Promise<void>
  /** Quit-flush registry: pending text checkpoints flushed on app quit. */
  localTurnFlushers: Set<() => unknown>
  /** True while the desktop host is quitting (an interrupted turn then stays
   * nonterminal in the journal so restart recovery owns its disposition). */
  isQuitting: () => boolean
  /** L7: host-owned spec projection/revalidation. Providers receive bounded
   * context only; they never parse specs or produce verdicts. */
  specWorkflow?: Readonly<{
    beforeTurn: (laneRef: string, request: ClaudeLocalStartRequest) => SpecLaneTurnProjection
    afterTurn: (
      laneRef: string,
      request: ClaudeLocalStartRequest,
      before: SpecLaneTurnProjection,
    ) => void
  }>
  /** Optional owner-local graph projection for an ordinary foreground turn. */
  graphMemoryWorkflow?: Readonly<{
    beforeTurn: (input: Readonly<{
      laneRef: string
      request: ClaudeLocalStartRequest
      history: ReadonlyArray<ProviderLaneHistoryMessage>
      message: string
    }>) => Promise<Readonly<{ message: string }>>
  }>
  now?: () => Date
  /** Main-owned observation after durable projection. Background Full Auto
   * uses this to publish bounded progress and own queued-message promotion
   * without pretending a renderer initiated the turn. */
  onTurnEventProjected?: (
    request: ClaudeLocalStartRequest,
    event: ClaudeLocalEvent,
    background: boolean,
  ) => void
  /**
   * Optional bridge to the canonical IDE portable mutation authority. The
   * resolver runs once after lane admission and captures one immutable permit
   * for that exact turn. A null result keeps the existing local-only behavior.
   * Movement remains owned by the durable portable-session authority; this
   * bridge only fences process-local provider effects.
   */
  portableMutation?: Readonly<{
    resolve: (input: Readonly<{
      laneRef: string
      request: ClaudeLocalStartRequest
      requestedModel: string
    }>) => Readonly<{
      grantRef: string
      authority: IdePortableMutationAuthority
    }> | null
    /** Maximum time quiesce waits for provider calls to return to the host. */
    quiesceTimeoutMs?: number
    /** Testable bounded cadence for detecting attachment revocation. */
    permitMonitorMs?: number
  }>
}>

/**
 * Persisted user-note text for a turn (capability I1). An images-only turn
 * (empty message) gets an honest bounded placeholder so the transcript row is
 * never blank; a turn with text keeps the user's text verbatim.
 */
export const userNoteText = (message: string, images?: ReadonlyArray<unknown>): string => {
  const trimmed = message.trim()
  if (trimmed !== "") return trimmed
  const count = images?.length ?? 0
  return count === 1 ? "(1 image attached)" : `(${count} images attached)`
}

/**
 * The text block sent to the model (capability I1). Images-only turns get a
 * neutral instruction so the SDK/codex receive non-empty prompt text alongside
 * the image; a turn with text keeps the user's text verbatim.
 */
export const turnPromptText = (message: string, images?: ReadonlyArray<unknown>): string => {
  const trimmed = message.trim()
  if (trimmed !== "") return trimmed
  const count = images?.length ?? 0
  return count > 0
    ? `Please look at the attached image${count === 1 ? "" : "s"}.`
    : trimmed
}

export type ProviderLaneDispatcher = Readonly<{
  dispatchTurn: <Context>(
    lane: ProviderLane<Context>,
    request: ClaudeLocalStartRequest,
    sender: ProviderLaneEventSender | null,
  ) => Promise<ProviderLaneDispatchResult>
  /** Stop new turns, interrupt active turns, and wait for a bounded safe point. */
  quiesce: () => Promise<ProviderLaneQuiesceResult>
  /** Idempotent alias that permanently keeps this dispatcher quiesced. */
  dispose: () => Promise<ProviderLaneQuiesceResult>
}>

export type ProviderLaneQuiesceResult = Readonly<{
  state: "safe" | "timed_out"
  /** A timeout is not proof that provider-side execution stopped. */
  pendingTurnRefs: ReadonlyArray<string>
}>

type ActiveProviderTurn = Readonly<{
  key: string
  turnRef: string
  revoke: (reason?: "host_quiesced" | "portable_authority_revoked") => void
  safePoint: Promise<void>
}>

export const makeProviderLaneDispatcher = (
  deps: ProviderLaneDispatcherDeps,
): ProviderLaneDispatcher => {
  const now = deps.now ?? (() => new Date())
  const timestamp = (): string =>
    now().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
  const activeTurns = new Map<string, ActiveProviderTurn>()
  const quiesceTimeoutMs = Math.max(10, Math.min(deps.portableMutation?.quiesceTimeoutMs ?? 5_000, 30_000))
  const permitMonitorMs = Math.max(5, Math.min(deps.portableMutation?.permitMonitorMs ?? 25, 250))
  let accepting = true
  let quiescePromise: Promise<ProviderLaneQuiesceResult> | null = null

  const activeKey = (laneRef: string, request: ClaudeLocalStartRequest): string =>
    `${laneRef}\u0000${request.threadRef}\u0000${request.turnRef}`

  const quiesce = (): Promise<ProviderLaneQuiesceResult> => {
    accepting = false
    if (quiescePromise !== null) return quiescePromise
    const snapshot = [...activeTurns.values()]
    for (const active of snapshot) active.revoke()
    quiescePromise = (async () => {
      if (snapshot.length === 0) return { state: "safe", pendingTurnRefs: [] }
      let timer: ReturnType<typeof setTimeout> | null = null
      const timeout = new Promise<"timed_out">(resolve => {
        timer = setTimeout(() => resolve("timed_out"), quiesceTimeoutMs)
      })
      const result = await Promise.race([
        Promise.all(snapshot.map(active => active.safePoint)).then(() => "safe" as const),
        timeout,
      ])
      if (timer !== null) clearTimeout(timer)
      return result === "safe"
        ? { state: "safe", pendingTurnRefs: [] }
        : {
            state: "timed_out",
            pendingTurnRefs: snapshot
              .filter(active => activeTurns.has(active.key))
              .map(active => active.turnRef),
          }
    })()
    return quiescePromise
  }

  const dispatchTurn = async <Context>(
    lane: ProviderLane<Context>,
    request: ClaudeLocalStartRequest,
    sender: ProviderLaneEventSender | null,
  ): Promise<ProviderLaneDispatchResult> => {
    if (!accepting) {
      return { ok: false, error: "Provider turns are quiesced on this host." }
    }
    if (!startRequestHasContent(request)) {
      return { ok: false, error: "That message could not be sent." }
    }
    const capabilityAdmission = projectProviderLaneCapabilities(lane.capabilities())
    if (capabilityAdmission.admission !== "admitted") {
      return { ok: false, error: capabilityAdmission.reason ?? "That provider lane is quarantined." }
    }
    const admission = lane.admit(request)
    if (!admission.ok) return { ok: false, error: admission.error }
    const requestedModel = admission.model
    const store = deps.threads()
    if (store.open(request.threadRef) === null) {
      return { ok: false, error: "That conversation no longer exists.", failureCause: "host_thread_missing" }
    }
    let portablePermit: Readonly<{
      authority: IdePortableMutationAuthority
      permit: IdePortableMutationPermit
    }> | null = null
    try {
      const binding = deps.portableMutation?.resolve({ laneRef: lane.laneRef, request, requestedModel }) ?? null
      if (binding !== null) {
        const authorization = binding.authority.authorize(binding.grantRef)
        if (authorization._tag === "Refused") {
          return { ok: false, error: "That workspace is no longer authorized on this device." }
        }
        portablePermit = Object.freeze({
          authority: binding.authority,
          permit: Object.freeze({ ...authorization.permit }),
        })
      }
    } catch {
      return { ok: false, error: "That workspace is no longer authorized on this device." }
    }
    const permitIsCurrent = (): boolean => {
      if (portablePermit === null) return true
      try {
        return portablePermit.authority.reauthorize(portablePermit.permit)
      } catch {
        return false
      }
    }
    if (!permitIsCurrent()) {
      return { ok: false, error: "That workspace is no longer authorized on this device." }
    }
    lane.prepare?.(request, sender, requestedModel)
    const turnKey = { threadRef: request.threadRef, turnRef: request.turnRef, lane: lane.laneRef }
    const user: DesktopMessage = {
      key: `${request.turnRef}-user`,
      role: "user",
      text: userNoteText(request.message, request.images),
      timestamp: timestamp(),
    }
    if (!permitIsCurrent()) {
      return { ok: false, error: "That workspace is no longer authorized on this device." }
    }
    const accepted = deps.journal.accept({
      ...turnKey,
      userMessageKey: user.key,
      assistantMessageKey: `${request.turnRef}-assistant-0`,
      accountRef: request.target?.accountRef ?? null,
      model: requestedModel,
    })
    if (!accepted.accepted) return { ok: false, error: "That turn is already accepted." }
    const closePrelaunchRevocation = (): ProviderLaneDispatchResult => {
      deps.journal.terminal(turnKey, "interrupted", "owner_interrupted")
      return {
        ok: false,
        reason: "interrupted",
        error: lane.failureMessage("interrupted", "Portable workspace authority changed before provider launch."),
      }
    }
    if (!permitIsCurrent()) {
      return closePrelaunchRevocation()
    }
    lane.bound?.(request)
    if (!permitIsCurrent()) {
      return closePrelaunchRevocation()
    }
    const saved = store.upsert(request.threadRef, user)
    if (saved === null) {
      return { ok: false, error: "That conversation no longer exists.", failureCause: "host_thread_missing" }
    }
    // History authority is main's own thread store — the renderer supplies
    // only the new message. The just-appended user note is the prompt, not
    // history.
    const history = saved.notes
      .filter(note => note.key !== user.key)
      .map(note => ({ role: note.role, text: note.text }))
    const startedAt = Date.now()
    let effectiveModel: string | null = null
    const turnContext: ProviderLaneTurnContext<Context> = {
      request,
      requestedModel,
      context: admission.context,
      effectiveModel: () => effectiveModel,
      store,
      timestamp,
    }
    let revoked = false
    let revocationReason: "host_quiesced" | "portable_authority_revoked" | null = null
    let interruptRequested = false
    let lifecycleClosed = false
    let reachSafePoint = (): void => {}
    const safePoint = new Promise<void>(resolve => {
      reachSafePoint = resolve
    })
    let revoke: ActiveProviderTurn["revoke"] = () => {}
    const isAuthorized = (): boolean => {
      if (revoked) return false
      if (permitIsCurrent()) return true
      revoke("portable_authority_revoked")
      return false
    }
    revoke = (reason: "host_quiesced" | "portable_authority_revoked" = "host_quiesced"): void => {
      if (revoked) return
      revoked = true
      revocationReason = reason
      if (!interruptRequested) {
        interruptRequested = true
        // This asks the local lane to cancel. It is not proof that a remote
        // provider stopped execution; the safe-point wait below is bounded.
        try {
          lane.interrupt(request.turnRef)
        } catch {
          // A broken cancel hook cannot restore authority or strand quiesce.
        }
      }
    }
    const guardedJournal: LocalTurnJournal = {
      ...deps.journal,
      appendAssistantText: (key, text, segmentKey) =>
        isAuthorized() ? deps.journal.appendAssistantText(key, text, segmentKey) : null,
      setAssistantText: (key, text) =>
        isAuthorized() ? deps.journal.setAssistantText(key, text) : null,
      terminal: (key, phase, disposition) =>
        isAuthorized() ? deps.journal.terminal(key, phase, disposition) : null,
    }
    const textPersistence = makeLocalTurnTextPersistence({
      journal: guardedJournal,
      store,
      key: turnKey,
      meta: () => lane.streamMeta(turnContext),
    })
    const opensTranscriptPosition = makeTranscriptOrderingBoundaryTracker()
    deps.localTurnFlushers.add(textPersistence.flush)
    const key = activeKey(lane.laneRef, request)
    const active: ActiveProviderTurn = {
      key,
      turnRef: request.turnRef,
      revoke,
      safePoint,
    }
    activeTurns.set(key, active)
    let permitMonitor: ReturnType<typeof setInterval> | null = null
    if (portablePermit !== null) {
      permitMonitor = setInterval(() => { void isAuthorized() }, permitMonitorMs)
      permitMonitor.unref?.()
    }
    try {
    const interruptedResult = (): ProviderLaneDispatchResult => {
      // This terminal row is host lifecycle metadata only. It does not admit
      // late provider output, workspace state, or a completion receipt.
      if (!lifecycleClosed) {
        lifecycleClosed = true
        deps.journal.terminal(turnKey, "interrupted", "owner_interrupted")
      }
      return {
        ok: false,
        reason: "interrupted",
        error: lane.failureMessage("interrupted", revocationReason === "portable_authority_revoked"
          ? "Portable workspace authority changed before the turn reached a safe point."
          : "The provider turn was quiesced before it reached a safe point."),
      }
    }
    if (!isAuthorized()) return interruptedResult()
    let specProjection: SpecLaneTurnProjection | undefined
    try {
      specProjection = deps.specWorkflow?.beforeTurn(lane.laneRef, request)
    } catch {
      // Spec context is additive. A projection failure cannot strand an
      // already journal-accepted owner turn; revalidation remains fail-closed.
    }
    const projectLaneEvent = lane.makeTurnProjector?.(turnContext)
    // CUT-11 (#8691): register the root turn on the canonical live agent
    // graph before its stream events arrive.
    if (!isAuthorized()) return interruptedResult()
    deps.liveAgentGraph.beginTurn({
      turnRef: request.turnRef,
      threadRef: request.threadRef,
      lane: lane.graphLaneRef,
    })
    // GIT-1 (#8781): checkpoint the pre-turn workspace state as a hidden ref
    // before the model can write files. Awaited so the snapshot cannot race
    // the turn's first edit.
    if (!isAuthorized()) return interruptedResult()
    await deps.captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_start")
    if (!isAuthorized()) return interruptedResult()
    const emitTurnEvent = (turnEvent: ClaudeLocalEvent): void => {
        if (!isAuthorized()) return
        // CUT-11 (#8691): fold the SAME typed envelope the renderer receives
        // into the canonical live agent graph.
        deps.liveAgentGraph.applyEvent(request.threadRef, {
          turnRef: request.turnRef,
          event: turnEvent,
        })
        if (turnEvent.kind === "model_effective") effectiveModel = turnEvent.model
        if (turnEvent.kind === "text_delta") textPersistence.append(turnEvent.text)
        else if (opensTranscriptPosition(turnEvent)) textPersistence.boundary()
        // Session usage ledger (#8712 Lane C): exact usage from the typed
        // completion event, attributed to the lane's provider with the
        // owner-selected model as spawn-config truth. A split-less emitter
        // is recorded honestly as total only.
        if (turnEvent.kind === "turn_completed" && turnEvent.accountRef !== undefined) {
          deps.usageLedger.record({
            provider: lane.usageProvider,
            accountRef: turnEvent.accountRef,
            requestedModel,
            kind: "turn",
            usage: turnEvent.usage ?? (turnEvent.totalTokens === null
              ? null
              : {
                  inputTokens: 0,
                  cachedInputTokens: 0,
                  outputTokens: 0,
                  reasoningTokens: 0,
                  totalTokens: turnEvent.totalTokens,
                }),
          })
        }
        // Persist the shared tool trace so the finalized transcript keeps the
        // same evidence the live stream showed (bounded by the store's note
        // cap). The SAME note key format both existing lanes always used.
        if (
          turnEvent.kind === "tool_use" || turnEvent.kind === "tool_progress" ||
          turnEvent.kind === "tool_result"
        ) {
          const traceNote = {
            key: turnEvent.itemRef === undefined
              ? randomUUID()
              : `${request.turnRef}-tool-${turnEvent.itemRef}${turnEvent.kind === "tool_result" ? "-result" : ""}`,
            role: "system",
            text: claudeLocalTraceNoteText(turnEvent),
            timestamp: timestamp(),
            // Typed trace facts (EP250 tool cards): the persisted note carries
            // the same typed payload the live stream note does.
            meta: { trace: claudeLocalTraceNoteMeta(turnEvent) },
          } as const
          if (turnEvent.itemRef === undefined) store.append(request.threadRef, traceNote)
          else store.upsert(request.threadRef, traceNote)
        }
        // Effective-model caption line, lane-branded.
        if (turnEvent.kind === "model_effective") {
          store.append(request.threadRef, {
            key: randomUUID(),
            role: "system",
            text: lane.modelNoteText(turnEvent.model),
            timestamp: timestamp(),
          })
        }
        // Lane-specific durable projection (plan cards, child usage,
        // reasoning lines, structured runtime cards, …).
        projectLaneEvent?.(turnEvent)
        deps.onTurnEventProjected?.(request, turnEvent, sender === null)
        if (sender === null || sender.isDestroyed()) return
        // Attach the persisted thread snapshot (user message included) to the
        // start event so the renderer can stream onto real thread state.
        const forwarded = turnEvent.kind === "turn_started" ? { ...turnEvent, thread: saved } : turnEvent
        sender.send(lane.eventChannel, { turnRef: request.turnRef, event: forwarded })
    }
    const baseMessage = specProjection === undefined
      ? turnPromptText(request.message, request.images)
      : appendSpecLaneContext(turnPromptText(request.message, request.images), specProjection)
    let providerMessage = baseMessage
    if (sender !== null && request.fullAuto !== true) {
      try {
        providerMessage = (await deps.graphMemoryWorkflow?.beforeTurn({
          laneRef: lane.laneRef,
          request,
          history,
          message: baseMessage,
        }))?.message ?? baseMessage
      } catch {
        // Graph memory is advisory. A failed projection cannot strand a turn
        // or change the prompt bytes that the existing lane would receive.
        providerMessage = baseMessage
      }
    }
    let result: ProviderLaneTurnResult
    try {
      if (!isAuthorized()) return interruptedResult()
      result = await lane.runTurn({
        request,
        model: requestedModel,
        context: admission.context,
        history,
        message: providerMessage,
        background: sender === null,
        emit: emitTurnEvent,
      })
    } catch {
      const detail = "The provider lane stopped unexpectedly."
      emitTurnEvent({ kind: "turn_failed", reason: "session_failed", detail })
      result = { ok: false, reason: "session_failed", detail }
    }
    if (!isAuthorized()) return interruptedResult()
    if (specProjection !== undefined && isAuthorized()) {
      try {
        deps.specWorkflow?.afterTurn(lane.laneRef, request, specProjection)
      } catch {
        // Provider completion truth remains independent of the optional note.
      }
    }
    if (!result.ok) {
      if (!isAuthorized()) return interruptedResult()
      textPersistence.flush()
      deps.localTurnFlushers.delete(textPersistence.flush)
      if (!(deps.isQuitting() && result.reason === "interrupted")) {
        guardedJournal.terminal(
          turnKey,
          result.reason === "interrupted" ? "interrupted" : "failed",
          result.reason === "interrupted" ? "owner_interrupted" : "failed",
        )
      }
      return {
        ok: false,
        reason: result.reason,
        error: lane.failureMessage(result.reason, result.detail),
      }
    }
    // Admit the completed checkpoint while the captured permit is current,
    // then reauthorize again after the asynchronous host call. Final journal
    // and receipt writes are synchronous and happen only after that check.
    if (!isAuthorized()) return interruptedResult()
    await deps.captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_completed")
    if (!isAuthorized()) return interruptedResult()
    textPersistence.complete(result.text.slice(0, CLAUDE_LOCAL_FINAL_TEXT_LIMIT))
    deps.localTurnFlushers.delete(textPersistence.flush)
    const finalMeta = lane.finalMeta({
      ...turnContext,
      result,
      durationMs: Date.now() - startedAt,
    })
    const assistantKeys = new Set(
      guardedJournal.get(turnKey)?.assistantSegments.map(segment => segment.key) ?? [],
    )
    for (const assistant of store.open(request.threadRef)?.notes.filter(note => assistantKeys.has(note.key)) ?? []) {
      store.upsert(request.threadRef, { ...assistant, meta: finalMeta })
    }
    const thread = assistantKeys.size === 0 ? null : store.open(request.threadRef)
    guardedJournal.terminal(turnKey, "completed", "completed")
    if (!isAuthorized()) return interruptedResult()
    lane.completed?.(request)
    return thread === null
      ? { ok: false, error: "That conversation no longer exists.", failureCause: "host_thread_missing" }
      : { ok: true, thread }
    } finally {
      if (permitMonitor !== null) clearInterval(permitMonitor)
      deps.localTurnFlushers.delete(textPersistence.flush)
      activeTurns.delete(key)
      reachSafePoint()
    }
  }

  return { dispatchTurn, quiesce, dispose: quiesce }
}
