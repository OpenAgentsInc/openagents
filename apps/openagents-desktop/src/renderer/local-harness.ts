/**
 * Local-mode harness routing (#8712 + EP250 codex-first-class): the chat host
 * used when the app is NOT signed in (Khala Sync conversation catalog not
 * live).
 *
 * The law this module enforces in code: selecting a harness can never route
 * to the cloud gateway or another provider.
 * - "fable" streams a real local Claude turn through the fableLocal bridge
 *   (typed unavailable error when no linked Claude account home exists).
 * - "codex" streams a real local `codex exec` turn through the codexLocal
 *   bridge (EP250: typed refusal naming the verified-account rule when no
 *   PROBE-VERIFIED Codex account exists — never the legacy gateway, never a
 *   silent substitution).
 * - Only a laneless send (no harness — programmatic callers) reaches the
 *   base host's legacy `completeChatTurn` fallback.
 *
 * Both lanes share ONE event-projection path (the codex lane reuses the
 * frozen fable-local event envelope), so codex turns render through the
 * exact same transcript cards.
 */
import type { DesktopMessage, DesktopMeterRateLimitWindow, DesktopMeterSnapshot, DesktopThread } from "../chat-contract.ts"
import {
  fableLocalFailureMessage,
  fableLocalModelNoteText,
  fableLocalTraceNoteMeta,
  fableLocalTraceNoteText,
  makeTranscriptOrderingBoundaryTracker,
  type FableLocalAvailability,
  type FableLocalEvent,
  type FableLocalEventEnvelope,
  type FableLocalImageAttachment,
  type LocalProviderTarget,
} from "../fable-local-contract.ts"
import type { LocalSkillInvocation } from "../plugin-config-contract.ts"
import {
  makeComposerInterruptIntent,
  makeComposerInterruptOutcome,
  makeComposerSubmitOutcome,
} from "../composer-admission.ts"
import {
  CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT,
  codexLocalModelNoteText,
  type CodexLocalAvailability,
} from "../codex-local-contract.ts"
import type { ChatHost, DesktopRuntimeFailureKind } from "./shell.ts"

export type FableLocalRendererBridge = Readonly<{
  availability: () => Promise<unknown>
  start: (value: unknown) => Promise<unknown>
  interrupt: (value: unknown) => Promise<unknown>
  onEvent: (listener: (envelope: FableLocalEventEnvelope) => void) => () => void
  /**
   * EP250 wave-2 runtime-capability channels (additive; absent on older
   * bridges). `steerChild` interrupts a running delegate child (G4);
   * `queueFollowup` enqueues a follow-up while a turn streams (A3).
   */
  steerChild?: (value: unknown) => Promise<unknown>
  steerCurrent?: (value: unknown) => Promise<unknown>
  queueFollowup?: (value: unknown) => Promise<unknown>
  queueList?: (threadRef: unknown) => Promise<unknown>
  queueEdit?: (value: unknown) => Promise<unknown>
  queueCancel?: (value: unknown) => Promise<unknown>
}>

/** EP250 evidence-gated refusal: Send refuses with the chip's reason — the
 * message is never routed to any other lane. */
export const codexLocalUnavailableMessage =
  `${CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT}. No message was sent to any other lane.`

const noteTimestamp = (now: Date = new Date()): string =>
  `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`

/**
 * Merge a `meter_updated` event onto the previous live snapshot (T11 #8868).
 * `account/rateLimits/updated` is documented as a SPARSE rolling update
 * ("clients should merge available values... does not clear a previously
 * observed value"), so a window this event omits keeps its last known value
 * instead of disappearing; a window it DOES report replaces the prior one by
 * label. Token fields likewise carry forward when a later event omits them
 * (Codex sends the full current tally together, but this stays defensive).
 */
const mergeMeterSnapshot = (
  previous: DesktopMeterSnapshot | null,
  event: Extract<FableLocalEvent, { kind: "meter_updated" }>,
): DesktopMeterSnapshot => {
  const rateLimitsByLabel = new Map<DesktopMeterRateLimitWindow["label"], DesktopMeterRateLimitWindow>()
  for (const window of previous?.rateLimits ?? []) rateLimitsByLabel.set(window.label, window)
  for (const window of event.rateLimits ?? []) rateLimitsByLabel.set(window.label, window)
  const rateLimits = [...rateLimitsByLabel.values()]
  const inputTokens = event.inputTokens ?? previous?.inputTokens
  const cachedInputTokens = event.cachedInputTokens ?? previous?.cachedInputTokens
  const outputTokens = event.outputTokens ?? previous?.outputTokens
  const reasoningTokens = event.reasoningTokens ?? previous?.reasoningTokens
  const totalTokens = event.totalTokens ?? previous?.totalTokens
  return {
    ...(inputTokens === undefined ? {} : { inputTokens }),
    ...(cachedInputTokens === undefined ? {} : { cachedInputTokens }),
    ...(outputTokens === undefined ? {} : { outputTokens }),
    ...(reasoningTokens === undefined ? {} : { reasoningTokens }),
    ...(totalTokens === undefined ? {} : { totalTokens }),
    ...(rateLimits.length === 0 ? {} : { rateLimits }),
  }
}

const decodeTurnResult = (
  raw: unknown,
  laneLabel: string,
): Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string; failureKind?: DesktopRuntimeFailureKind }> => {
  if (typeof raw === "object" && raw !== null && typeof (raw as { ok?: unknown }).ok === "boolean") {
    const result = raw as { ok: boolean; thread?: DesktopThread | null; error?: string; reason?: unknown }
    const failureKind: DesktopRuntimeFailureKind | undefined = result.ok
      ? undefined
      : result.reason === "incompatible_workflow" ? "incompatible"
        : result.reason === "interrupted" ? "interrupted"
          : result.reason === "no_codex_account" || result.reason === "account_reconnect_required" ? "signed_out"
            : "failed"
    return { ...result, ...(failureKind === undefined ? {} : { failureKind }) }
  }
  return { ok: false, error: `The local ${laneLabel} lane returned an invalid response.`, failureKind: "failed" }
}

export type MakeLocalHarnessChatHostInput = Readonly<{
  base: ChatHost
  fable: FableLocalRendererBridge | null
  fableAvailability: () => FableLocalAvailability | null
  /** Codex local lane (EP250) — same bridge shape, its own channels. */
  codex?: FableLocalRendererBridge | null
  codexAvailability?: () => CodexLocalAvailability | null
  randomId?: () => string
  now?: () => Date
  /** Injectable renderer cadence; one projection per frame by default. */
  scheduleProjection?: (flush: () => void) => () => void
  onComposerAdmission?: (threadRef: string, value: import("../composer-admission.ts").ComposerAdmission) => void
  onComposerQueue?: (threadRef: string, value: ReadonlyArray<import("../codex-durable-queue.ts").CodexQueuedIntent>) => void
}>

export const makeLocalHarnessChatHost = (input: MakeLocalHarnessChatHostInput): ChatHost => {
  const randomId = input.randomId ?? (() => globalThis.crypto.randomUUID())
  const now = input.now ?? (() => new Date())
  const scheduleProjection = input.scheduleProjection ?? (flush => {
    const timer = setTimeout(flush, 16)
    return () => clearTimeout(timer)
  })

  /**
   * The turn currently streaming through a local lane, if any. Tracked so the
   * composer Stop button (EP250) can drive the lane's already-plumbed
   * interrupt IPC path by its exact turnRef. One local turn runs at a time.
   */
  let activeTurn: { bridge: FableLocalRendererBridge; turnRef: string; threadRef: string } | null = null

  /** One shared streaming projection for both local lanes: trace lines
   * first, growing assistant bubble last — the same order the finalized
   * persisted thread carries, so finalize never reshuffles the transcript. */
  const runLaneTurn = async (
    lane: "fable" | "codex",
    bridge: FableLocalRendererBridge,
    send: Readonly<{
      id: string
      message: string
      images?: ReadonlyArray<FableLocalImageAttachment>
      target?: LocalProviderTarget
      skill?: LocalSkillInvocation
      permissionMode?: "owner_full" | "plan_only"
      reasoningEffort?: import("../fable-local-contract.ts").CodexReasoningEffort
      model?: import("../fable-local-contract.ts").LocalModel
      queueRef?: string
      clientUserMessageId?: string
      /** Full Auto (#8852): Codex-lane only; ignored on the Claude lane. */
      fullAuto?: boolean
      onUpdate?: (thread: DesktopThread) => void
    }>,
  ): Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string; failureKind?: DesktopRuntimeFailureKind }>> => {
    const laneLabel = lane === "fable" ? "Claude" : "Codex"
    const turnRef = `turn.${lane}.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
    let baseThread: DesktopThread | null = null
    const orderedNotes: DesktopMessage[] = []
    let activeAssistantIndex: number | null = null
    let assistantSequence = 0
    let effectiveModel: string | null = null
    const laneName = lane === "fable" ? "fable-local" : "codex-local"
    let pendingAssistantChunks: string[] = []
    let cancelProjection: (() => void) | null = null
    let projectionDirty = false
    /** T11 #8868: live context/usage meter for this turn's ContextMeter mount
     * (header/rail — NOT a timeline note; see `header.tsx`). */
    let latestMeter: DesktopMeterSnapshot | null = null
    const opensTranscriptPosition = makeTranscriptOrderingBoundaryTracker()

    const commitAssistantText = (): void => {
      if (activeAssistantIndex === null || pendingAssistantChunks.length === 0) return
      const active = orderedNotes[activeAssistantIndex]!
      orderedNotes[activeAssistantIndex] = { ...active, text: active.text + pendingAssistantChunks.join("") }
      pendingAssistantChunks = []
    }
    const projectNow = (): void => {
      cancelProjection = null
      if (!projectionDirty) return
      projectionDirty = false
      commitAssistantText()
      if (baseThread === null || send.onUpdate === undefined) return
      send.onUpdate({
        ...baseThread,
        notes: [
          ...baseThread.notes,
          ...orderedNotes,
        ],
        ...(latestMeter === null ? {} : { meter: latestMeter }),
      })
    }
    const project = (): void => {
      projectionDirty = true
      if (cancelProjection === null) cancelProjection = scheduleProjection(projectNow)
    }
    const cancelScheduledProjection = (): void => {
      const cancel = cancelProjection as (() => void) | null
      cancel?.()
      cancelProjection = null
    }
    const flushProjection = (): void => {
      cancelScheduledProjection()
      projectNow()
    }
    const closeAssistantSegment = (): void => {
      commitAssistantText()
      activeAssistantIndex = null
    }
    const pushSystemNote = (text: string): void => {
      orderedNotes.push({
        key: `${turnRef}-trace-${orderedNotes.length}`,
        role: "system",
        text,
        timestamp: noteTimestamp(now()),
      })
      project()
    }
    // EP250 wave-2 runtime cards: upsert/remove a keyed system note carrying a
    // typed `runtime` payload (plan/child/queue). Plan updates in place (latest
    // wins); child status/steer merge onto the same childRef note; the queue
    // chip is removed when its follow-up is promoted.
    const upsertRuntimeNote = (
      key: string,
      runtime: NonNullable<DesktopMessage["runtime"]>,
      text: string,
    ): void => {
      const index = orderedNotes.findIndex(entry => entry.key === key)
      if (index === -1) {
        orderedNotes.push({ key, role: "system", text, timestamp: noteTimestamp(now()), runtime })
      } else {
        orderedNotes[index] = { ...orderedNotes[index]!, runtime, text }
      }
      project()
    }
    const removeRuntimeNote = (key: string): void => {
      const index = orderedNotes.findIndex(entry => entry.key === key)
      if (index !== -1) {
        orderedNotes.splice(index, 1)
        project()
      }
    }
    const childNoteKey = (childRef: string): string => `${turnRef}-child-${childRef}`
    const existingChildRuntime = (childRef: string): Extract<
      NonNullable<DesktopMessage["runtime"]>,
      { kind: "child" }
    > | null => {
      const found = orderedNotes.find(entry => entry.key === childNoteKey(childRef))?.runtime
      return found !== undefined && found.kind === "child" ? found : null
    }
    // A3: a follow-up promoted at the idle boundary becomes the next turn.
    let promotedFollowup: Readonly<{ message: string; queueRef: string; clientUserMessageId?: string }> | null = null
    const readPromotedFollowup = (): typeof promotedFollowup => promotedFollowup

    const unsubscribe = bridge.onEvent(envelope => {
      if (envelope.turnRef !== turnRef) return
      const event = envelope.event
      const opensNewTimelinePosition = opensTranscriptPosition(event)
      if (event.kind === "turn_started") {
        // Main attaches the persisted thread (user message included) so the
        // stream projects onto real state, not a synthesized transcript.
        if (event.thread !== undefined) {
          baseThread = event.thread
          project()
        }
        return
      }
      if (event.kind === "composer_admission") {
        input.onComposerAdmission?.(send.id, {
          state: event.state,
          activeTurnId: event.activeTurnId,
          reason: event.reason,
          queuedCount: 0,
        })
        if (lane === "codex" && input.codex?.queueList !== undefined) {
          void input.codex.queueList(send.id).then(value => {
            if (Array.isArray(value)) input.onComposerQueue?.(send.id, value as import("../codex-durable-queue.ts").CodexQueuedIntent[])
          })
        }
        return
      }
      if (event.kind === "text_delta") {
        if (activeAssistantIndex === null) {
          activeAssistantIndex = orderedNotes.length
          orderedNotes.push({
              key: `${turnRef}-assistant-${assistantSequence++}`,
              role: "assistant",
              text: "",
              timestamp: noteTimestamp(now()),
              // Streaming metadata (#8712): lane + turn are known here; the
              // effective model joins when its typed event lands. The
              // persisted note from main carries the authoritative meta.
              meta: { lane: laneName, turnRef, ...(effectiveModel === null ? {} : { model: effectiveModel }) },
            })
        }
        pendingAssistantChunks.push(event.text)
        project()
        return
      }
      if (event.kind === "model_effective") {
        // Effective-model caption above the assistant reply ("Fable ·
        // claude-fable-5" / "Codex · gpt-5.6-sol (requested)"). For fable the
        // value is the SDK init report; for codex it is spawn-config truth
        // and arrives already "(requested)"-labeled. Main persists the same
        // line, so finalize does not reshuffle the transcript.
        effectiveModel = event.model
        if (activeAssistantIndex !== null) {
          const active = orderedNotes[activeAssistantIndex]!
          orderedNotes[activeAssistantIndex] = {
            ...active,
            meta: { ...active.meta, lane: laneName, turnRef, model: event.model },
          }
        }
        closeAssistantSegment()
        pushSystemNote(lane === "fable"
          ? fableLocalModelNoteText(event.model)
          : codexLocalModelNoteText(event.model))
        return
      }
      // Only a NEW visible position is an ordering boundary. Keyed card
      // updates (tool progress/result, plan refreshes, resolved questions and
      // child status) stay at their original position and must not split prose
      // into phantom paragraph gaps.
      if (opensNewTimelinePosition) closeAssistantSegment()
      if (event.kind === "tool_use" || event.kind === "tool_progress" || event.kind === "tool_result") {
        const key = event.itemRef === undefined
          ? `${turnRef}-trace-${orderedNotes.length}`
          : `${turnRef}-tool-${event.itemRef}${event.kind === "tool_result" ? "-result" : ""}`
        const note = {
          key,
          role: "system",
          text: fableLocalTraceNoteText(event),
          timestamp: noteTimestamp(now()),
          // Typed trace facts (EP250 tool cards): same bounded payload as
          // the text line, so the renderer builds typed cards without
          // re-parsing display strings.
          meta: { trace: fableLocalTraceNoteMeta(event) },
        } as const
        const existing = orderedNotes.findIndex(entry => entry.key === key)
        if (existing === -1) orderedNotes.push(note)
        else orderedNotes[existing] = note
        project()
        return
      }
      // Reasoning treatment (EP250): compact system line, the same rendering
      // the runtime timeline gives reasoning items.
      if (event.kind === "reasoning") {
        pushSystemNote(`Reasoning · ${event.text}`)
        return
      }
      // Typed VISIBLE rotation/lane notices (EP250): never a silent skip.
      if (event.kind === "lane_notice") {
        pushSystemNote(event.text)
        return
      }
      // Interactive question cards (EP250): question_pending projects an
      // interactive card note; question_resolved updates it in place with
      // the runtime-authoritative outcome.
      if (event.kind === "question_pending") {
        orderedNotes.push({
          key: `${turnRef}-question-${event.questionRef}`,
          role: "system",
          text: event.questions[0]?.question ?? "Question",
          timestamp: noteTimestamp(now()),
          question: {
            turnRef,
            questionRef: event.questionRef,
            status: "pending",
            ...(event.interactionKind === undefined ? {} : { kind: event.interactionKind }),
            ...(event.decisionRef === undefined ? {} : { decisionRef: event.decisionRef }),
            questions: event.questions,
          },
        })
        project()
        return
      }
      if (event.kind === "question_resolved") {
        const index = orderedNotes.findIndex(
          note => note.question?.questionRef === event.questionRef,
        )
        const existing = index === -1 ? undefined : orderedNotes[index]
        if (existing?.question !== undefined) {
          orderedNotes[index] = {
            ...existing,
            question: { ...existing.question, status: event.outcome },
          }
          project()
        }
        return
      }
      // -----------------------------------------------------------------
      // EP250 wave-2 runtime-capability cards.
      // -----------------------------------------------------------------
      // Plan/todo progress (J2/J4): ONE card per turn, updated in place.
      if (event.kind === "plan_updated") {
        upsertRuntimeNote(
          `${turnRef}-plan`,
          { kind: "plan", entries: event.entries.map(entry => ({ step: entry.step, status: entry.status })) },
          "Plan updated",
        )
        return
      }
      // Context/usage meter (T11 #8868): NOT a timeline note — it drives the
      // conversation header's live ContextMeter mount (`header.tsx`), merged
      // (never replaced) so a sparse rate-limit update can't erase an
      // already-known window.
      if (event.kind === "meter_updated") {
        latestMeter = mergeMeterSnapshot(latestMeter, event)
        project()
        return
      }
      // Delegate-child lifecycle (G4): a running child card offers Interrupt;
      // status/steer merge onto the same childRef note.
      if (event.kind === "child_started") {
        upsertRuntimeNote(
          childNoteKey(event.childRef),
          {
            kind: "child",
            turnRef,
            childRef: event.childRef,
            ...(event.parentChildRef === undefined ? {} : { parentChildRef: event.parentChildRef }),
            status: "running",
            title: event.summary,
            detail: "",
            transcript: [{ role: "user", text: event.prompt ?? event.summary }],
            steered: null,
          },
          `Delegate child started · ${event.summary}`,
        )
        return
      }
      if (event.kind === "child_activity") {
        const existing = existingChildRuntime(event.childRef)
        upsertRuntimeNote(
          childNoteKey(event.childRef),
          {
            kind: "child",
            turnRef,
            childRef: event.childRef,
            ...((event.parentChildRef ?? existing?.parentChildRef) === undefined
              ? {}
              : { parentChildRef: event.parentChildRef ?? existing?.parentChildRef }),
            status: existing?.status ?? "running",
            title: existing?.title ?? "",
            detail: event.summary,
            transcript: [
              ...(existing?.transcript ?? []),
              { role: "system" as const, text: event.summary },
            ].slice(-128),
            steered: existing?.steered ?? null,
          },
          `Delegate child · ${event.summary}`,
        )
        return
      }
      if (event.kind === "child_completed") {
        const existing = existingChildRuntime(event.childRef)
        upsertRuntimeNote(
          childNoteKey(event.childRef),
          {
            kind: "child",
            turnRef,
            childRef: event.childRef,
            ...((event.parentChildRef ?? existing?.parentChildRef) === undefined
              ? {}
              : { parentChildRef: event.parentChildRef ?? existing?.parentChildRef }),
            status: "completed",
            title: existing?.title ?? event.summary,
            detail: event.summary,
            transcript: [
              ...(existing?.transcript ?? []),
              { role: "assistant" as const, text: event.response ?? event.summary },
            ].slice(-128),
            steered: existing?.steered ?? null,
          },
          `Delegate child completed · ${event.summary}`,
        )
        return
      }
      if (event.kind === "child_failed") {
        const existing = existingChildRuntime(event.childRef)
        const detail = event.detail.trim() === "" ? event.reason : `${event.reason} · ${event.detail}`
        upsertRuntimeNote(
          childNoteKey(event.childRef),
          {
            kind: "child",
            turnRef,
            childRef: event.childRef,
            ...((event.parentChildRef ?? existing?.parentChildRef) === undefined
              ? {}
              : { parentChildRef: event.parentChildRef ?? existing?.parentChildRef }),
            status: "failed",
            title: existing?.title ?? "",
            detail,
            transcript: [
              ...(existing?.transcript ?? []),
              { role: "system" as const, text: detail },
            ].slice(-128),
            steered: existing?.steered ?? null,
          },
          `Delegate child failed · ${detail}`,
        )
        return
      }
      if (event.kind === "child_steered") {
        const existing = existingChildRuntime(event.childRef)
        if (existing === null) return
        upsertRuntimeNote(
          childNoteKey(event.childRef),
          { ...existing, steered: { action: event.action, outcome: event.outcome, detail: event.detail } },
          `Delegate child steered · ${event.action} · ${event.outcome}`,
        )
        return
      }
      // Queued follow-up (A3): a chip while queued; promoted becomes next turn.
      if (event.kind === "followup_queued") {
        upsertRuntimeNote(
          `${turnRef}-queue-${event.queueRef}`,
          { kind: "queue", turnRef, queueRef: event.queueRef, position: event.position },
          `Follow-up queued (#${event.position})`,
        )
        return
      }
      if (event.kind === "followup_promoted") {
        removeRuntimeNote(`${turnRef}-queue-${event.queueRef}`)
        promotedFollowup = { message: event.message, queueRef: event.queueRef, ...(event.clientUserMessageId === undefined ? {} : { clientUserMessageId: event.clientUserMessageId }) }
        return
      }
      // turn_completed / turn_failed carry no transcript body of their
      // own; the invoke result finalizes the thread.
    })
    try {
      activeTurn = { bridge, turnRef, threadRef: send.id }
      const raw = await bridge.start({
        turnRef,
        threadRef: send.id,
        message: send.message,
        ...(send.queueRef === undefined ? {} : { queueRef: send.queueRef }),
        ...(send.clientUserMessageId === undefined ? {} : { clientUserMessageId: send.clientUserMessageId }),
        // Capability I1: images ride the frozen start request additively.
        ...(send.images !== undefined && send.images.length > 0 ? { images: send.images } : {}),
        ...(send.target === undefined ? {} : { target: send.target }),
        ...(send.skill === undefined ? {} : { skill: send.skill }),
        ...(send.permissionMode === undefined ? {} : { permissionMode: send.permissionMode }),
        ...(lane !== "codex" || send.reasoningEffort === undefined ? {} : { reasoningEffort: send.reasoningEffort }),
        ...(send.model === undefined ? {} : { model: send.model }),
        ...(lane !== "codex" || send.fullAuto !== true ? {} : { fullAuto: true }),
      })
      const result = decodeTurnResult(raw, laneLabel)
      flushProjection()
      // A3 queue-until-idle: a follow-up promoted at this turn's idle boundary
      // becomes the NEXT turn (the runtime emits it but does not run it). Chain
      // it on the same lane/onUpdate and return its result so the shell's
      // withTurnResult applies the final thread; `pending` stays true across
      // the chain because onUpdate keeps setting it. The promoted follow-up is
      // text-only (no images re-ride the chained turn).
      // `?? ""` reads the closure-assigned value at runtime (the onEvent
      // assignment above is invisible to CFA, which linearly narrows the
      // `= null` init and would otherwise make a `!== null` guard `never`).
      const promoted = readPromotedFollowup()
      if (promoted !== null && promoted.message.trim() !== "") {
        return runLaneTurn(lane, bridge, { ...send, message: promoted.message, images: undefined, queueRef: promoted.queueRef, ...(promoted.clientUserMessageId === undefined ? {} : { clientUserMessageId: promoted.clientUserMessageId }) })
      }
      return result
    } finally {
      activeTurn = null
      input.onComposerAdmission?.(send.id, { state: "idle", activeTurnId: null, reason: null, queuedCount: 0 })
      cancelScheduledProjection()
      unsubscribe()
    }
  }

  const interruptActiveControlIdentity: NonNullable<ChatHost["interruptActiveControlIdentity"]> = async threadRef => {
    const active = activeTurn
    if (active === null || (threadRef !== undefined && active.threadRef !== threadRef)) return null
    const intentRef = `intent.desktop.interrupt.${active.turnRef}`
    return { threadRef: active.threadRef, intentRef, idempotencyKey: intentRef }
  }

  const interruptActiveControl: NonNullable<ChatHost["interruptActiveControl"]> = async threadRef => {
    const active = activeTurn
    if (active === null || (threadRef !== undefined && active.threadRef !== threadRef)) return null
    const createdAt = now().toISOString()
    const control = makeComposerInterruptIntent({
      threadRef: active.threadRef,
      turnRef: active.turnRef,
      intentRef: `intent.desktop.interrupt.${active.turnRef}`,
      createdAt,
    })
    const raw = await active.bridge.interrupt({ turnRef: active.turnRef })
    const observedAt = now().toISOString()
    return makeComposerInterruptOutcome({
      control,
      observedAt,
      admission: { status: "accepted", acceptedAt: createdAt },
      delivery: raw === true
        ? { status: "applied", appliedAt: observedAt }
        : { status: "failed", reasonRef: "reason.adapter_refused" },
    })
  }

  const queueFollowupControl: NonNullable<ChatHost["queueFollowupControl"]> = async request => {
    const active = activeTurn
    const observedAt = now().toISOString()
    if (
      active === null || active.bridge.queueFollowup === undefined ||
      request.control.kind !== "turn.queue" ||
      request.control.threadRef !== request.threadRef ||
      request.control.threadRef !== active.threadRef ||
      request.control.messageRef !== request.clientUserMessageId
    ) {
      return makeComposerSubmitOutcome({
        control: request.control,
        observedAt,
        admission: { status: "rejected", reasonRef: "reason.target_mismatch" },
        delivery: { status: "failed", reasonRef: "reason.target_mismatch" },
      })
    }
    const raw = await active.bridge.queueFollowup({
      threadRef: request.threadRef,
      message: request.message,
      intentRef: request.intentRef,
      clientUserMessageId: request.clientUserMessageId,
    })
    const queued = typeof raw === "object" && raw !== null && (raw as { queued?: unknown }).queued === true
    const queueRef = queued && typeof (raw as { queueRef?: unknown }).queueRef === "string"
      ? (raw as { queueRef: string }).queueRef
      : `queue.${request.intentRef}`
    const acknowledgedAt = now().toISOString()
    return makeComposerSubmitOutcome({
      control: request.control,
      observedAt: acknowledgedAt,
      admission: queued
        ? { status: "accepted", acceptedAt: observedAt }
        : { status: "rejected", reasonRef: "reason.adapter_refused" },
      delivery: queued
        ? { status: "queued", queueRef }
        : { status: "failed", reasonRef: "reason.adapter_refused" },
    })
  }

  const steerCurrentControl: NonNullable<ChatHost["steerCurrentControl"]> = async request => {
    const active = activeTurn
    const observedAt = now().toISOString()
    if (
      active === null || active.bridge.steerCurrent === undefined ||
      request.control.kind !== "turn.steer" ||
      request.control.threadRef !== request.threadRef ||
      request.control.threadRef !== active.threadRef ||
      request.control.turnRef !== request.expectedTurnId ||
      request.control.messageRef !== request.clientUserMessageId
    ) {
      return makeComposerSubmitOutcome({
        control: request.control,
        observedAt,
        admission: { status: "rejected", reasonRef: "reason.target_mismatch" },
        delivery: { status: "failed", reasonRef: "reason.target_mismatch" },
      })
    }
    const raw = await active.bridge.steerCurrent({
      threadRef: request.threadRef,
      message: request.message,
      intentRef: request.intentRef,
      clientUserMessageId: request.clientUserMessageId,
      expectedTurnId: request.expectedTurnId,
    })
    const outcome = typeof raw === "object" && raw !== null && typeof (raw as { outcome?: unknown }).outcome === "string"
      ? (raw as { ok?: unknown; outcome: string })
      : { ok: false, outcome: "invalid_response" }
    const acknowledgedAt = now().toISOString()
    const delivered = outcome.ok === true && outcome.outcome === "delivered"
    return makeComposerSubmitOutcome({
      control: request.control,
      observedAt: acknowledgedAt,
      admission: delivered
        ? { status: "accepted", acceptedAt: observedAt }
        : { status: "rejected", reasonRef: outcome.outcome === "unsupported" ? "reason.adapter_unsupported" : "reason.adapter_refused" },
      delivery: delivered
        ? { status: "applied", appliedAt: acknowledgedAt }
        : outcome.outcome === "unsupported"
          ? { status: "unsupported", reasonRef: "reason.adapter_unsupported" }
          : { status: "failed", reasonRef: "reason.adapter_refused" },
    })
  }

  return {
    ...input.base,
    /**
     * Interrupt the active local turn (EP250 Stop button). Signals the lane's
     * frozen interrupt channel by the exact active turnRef; the runtime aborts
     * and emits a typed `interrupted` failure that finalizes the turn.
     */
    interruptActive: async threadRef =>
      (await interruptActiveControl(threadRef))?.delivery.status === "applied",
    interruptActiveControlIdentity,
    interruptActiveControl,
    /**
     * Interrupt a running delegate child of the active turn (EP250 wave-2 G4).
     * Signals the active lane's frozen steer-child channel by exact ref (only
     * `interrupt` — message is capability-unsupported). The runtime's typed
     * child_steered event renders the outcome; no active lane returns not_found.
     */
    steerChild: async input => {
      const active = activeTurn
      if (active === null || active.bridge.steerChild === undefined) {
        return { ok: false, outcome: "not_found" }
      }
      const raw = await active.bridge.steerChild({
        turnRef: input.turnRef,
        childRef: input.childRef,
        action: "interrupt",
      })
      return typeof raw === "object" && raw !== null && typeof (raw as { outcome?: unknown }).outcome === "string"
        ? (raw as { ok: boolean; outcome: string })
        : { ok: false, outcome: "not_found" }
    },
    /**
     * Enqueue a follow-up while a turn streams (EP250 wave-2 A3). Routes through
     * the active lane's frozen queue channel; delivery is queue-until-idle (the
     * runtime emits followup_queued now and followup_promoted at turn end). No
     * active lane returns a no-queue result.
     */
    queueFollowup: async input => {
      const active = activeTurn
      if (active === null || active.bridge.queueFollowup === undefined) return { ok: false, queued: false }
      const raw = await active.bridge.queueFollowup({
        threadRef: input.threadRef,
        message: input.message,
        ...(input.intentRef === undefined ? {} : { intentRef: input.intentRef }),
        ...(input.clientUserMessageId === undefined ? {} : { clientUserMessageId: input.clientUserMessageId }),
      })
      return typeof raw === "object" && raw !== null && typeof (raw as { queued?: unknown }).queued === "boolean"
        ? (raw as { ok: boolean; queued: boolean })
        : { ok: false, queued: false }
    },
    queueFollowupControl,
    queueList: async threadRef => {
      const values = await input.codex?.queueList?.(threadRef)
      return Array.isArray(values) ? values as import("../codex-durable-queue.ts").CodexQueuedIntent[] : []
    },
    queueEdit: async request => input.codex?.queueEdit?.(request) as Promise<Readonly<{ ok: boolean }>> ?? Promise.resolve({ ok: false }),
    queueCancel: async request => input.codex?.queueCancel?.(request) as Promise<Readonly<{ ok: boolean }>> ?? Promise.resolve({ ok: false }),
    steerCurrent: async input => {
      const active = activeTurn
      if (active === null || active.bridge.steerCurrent === undefined) {
        return { ok: false, outcome: "not_found" }
      }
      const raw = await active.bridge.steerCurrent({
        threadRef: input.threadRef,
        message: input.message,
        ...(input.intentRef === undefined ? {} : { intentRef: input.intentRef }),
        ...(input.clientUserMessageId === undefined ? {} : { clientUserMessageId: input.clientUserMessageId }),
        ...(input.expectedTurnId === undefined ? {} : { expectedTurnId: input.expectedTurnId }),
      })
      return typeof raw === "object" && raw !== null && typeof (raw as { outcome?: unknown }).outcome === "string"
        ? raw as { ok: boolean; outcome: string }
        : { ok: false, outcome: "not_found" }
    },
    steerCurrentControl,
    sendMessage: async send => {
      if (send.harness === undefined) return input.base.sendMessage(send)
      if (send.harness === "codex") {
        // EP250 chip-verified-evidence rule: a send routes to the local
        // codex lane only over a PROBE-VERIFIED account; otherwise Send
        // refuses with the reason — never a silent substitution.
        const availability = input.codexAvailability?.() ?? null
        const bridge = input.codex ?? null
        if (bridge === null || availability === null || availability.state !== "available") {
          const reason = availability?.state === "unavailable" ? availability.reason : null
          const failureKind: DesktopRuntimeFailureKind = reason === "policy_denied"
            ? "policy_denied"
            : reason === "quota_exhausted"
              ? "quota_exhausted"
              : reason === "rate_limited"
                ? "rate_limited"
                : "signed_out"
          return { ok: false, error: codexLocalUnavailableMessage, failureKind }
        }
        return runLaneTurn("codex", bridge, send)
      }
      const availability = input.fableAvailability()
      if (input.fable === null || availability === null || availability.state !== "available") {
        return {
          ok: false,
          error: fableLocalFailureMessage("no_claude_account", ""),
          failureKind: availability?.state === "unavailable" && availability.reason === "no_claude_account"
            ? "signed_out" as const
            : "failed" as const,
        }
      }
      return runLaneTurn("fable", input.fable, send)
    },
  }
}
