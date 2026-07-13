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
import type { DesktopMessage, DesktopThread } from "../chat-contract.ts"
import {
  fableLocalFailureMessage,
  fableLocalModelNoteText,
  fableLocalTraceNoteMeta,
  fableLocalTraceNoteText,
  type FableLocalAvailability,
  type FableLocalEventEnvelope,
  type FableLocalImageAttachment,
  type LocalProviderTarget,
} from "../fable-local-contract.ts"
import type { LocalSkillInvocation } from "../plugin-config-contract.ts"
import {
  CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT,
  codexLocalModelNoteText,
  type CodexLocalAvailability,
} from "../codex-local-contract.ts"
import type { ChatHost } from "./shell.ts"

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
}>

/** EP250 evidence-gated refusal: Send refuses with the chip's reason — the
 * message is never routed to any other lane. */
export const codexLocalUnavailableMessage =
  `${CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT}. No message was sent to any other lane.`

const noteTimestamp = (now: Date = new Date()): string =>
  `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`

const decodeTurnResult = (
  raw: unknown,
  laneLabel: string,
): Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }> => {
  if (typeof raw === "object" && raw !== null && typeof (raw as { ok?: unknown }).ok === "boolean") {
    return raw as { ok: boolean; thread?: DesktopThread | null; error?: string }
  }
  return { ok: false, error: `The local ${laneLabel} lane returned an invalid response.` }
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
}>

export const makeLocalHarnessChatHost = (input: MakeLocalHarnessChatHostInput): ChatHost => {
  const randomId = input.randomId ?? (() => globalThis.crypto.randomUUID())
  const now = input.now ?? (() => new Date())

  /**
   * The turn currently streaming through a local lane, if any. Tracked so the
   * composer Stop button (EP250) can drive the lane's already-plumbed
   * interrupt IPC path by its exact turnRef. One local turn runs at a time.
   */
  let activeTurn: { bridge: FableLocalRendererBridge; turnRef: string } | null = null

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
      onUpdate?: (thread: DesktopThread) => void
    }>,
  ): Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>> => {
    const laneLabel = lane === "fable" ? "Claude" : "Codex"
    const turnRef = `turn.${lane}.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
    let baseThread: DesktopThread | null = null
    const orderedNotes: DesktopMessage[] = []
    let activeAssistantIndex: number | null = null
    let assistantSequence = 0
    let effectiveModel: string | null = null
    const laneName = lane === "fable" ? "fable-local" : "codex-local"

    const project = (): void => {
      if (baseThread === null || send.onUpdate === undefined) return
      send.onUpdate({
        ...baseThread,
        notes: [
          ...baseThread.notes,
          ...orderedNotes,
        ],
      })
    }
    const closeAssistantSegment = (): void => {
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
    let promotedFollowup: string | null = null

    const unsubscribe = bridge.onEvent(envelope => {
      if (envelope.turnRef !== turnRef) return
      const event = envelope.event
      if (event.kind === "turn_started") {
        // Main attaches the persisted thread (user message included) so the
        // stream projects onto real state, not a synthesized transcript.
        if (event.thread !== undefined) {
          baseThread = event.thread
          project()
        }
        return
      }
      if (event.kind === "text_delta") {
        if (activeAssistantIndex === null) {
          activeAssistantIndex = orderedNotes.length
          orderedNotes.push({
              key: `${turnRef}-assistant-${assistantSequence++}`,
              role: "assistant",
              text: event.text,
              timestamp: noteTimestamp(now()),
              // Streaming metadata (#8712): lane + turn are known here; the
              // effective model joins when its typed event lands. The
              // persisted note from main carries the authoritative meta.
              meta: { lane: laneName, turnRef, ...(effectiveModel === null ? {} : { model: effectiveModel }) },
            })
        } else {
          const active = orderedNotes[activeAssistantIndex]!
          orderedNotes[activeAssistantIndex] = { ...active, text: active.text + event.text }
        }
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
      // Every non-text event is an ordering boundary. A later text delta
      // starts a new assistant segment after this event instead of mutating an
      // earlier bubble and teleporting it to the end of the transcript.
      closeAssistantSegment()
      if (event.kind === "tool_use" || event.kind === "tool_result") {
        orderedNotes.push({
          key: `${turnRef}-trace-${orderedNotes.length}`,
          role: "system",
          text: fableLocalTraceNoteText(event),
          timestamp: noteTimestamp(now()),
          // Typed trace facts (EP250 tool cards): same bounded payload as
          // the text line, so the renderer builds typed cards without
          // re-parsing display strings.
          meta: { trace: fableLocalTraceNoteMeta(event) },
        })
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
        promotedFollowup = event.message
        return
      }
      // turn_completed / turn_failed carry no transcript body of their
      // own; the invoke result finalizes the thread.
    })
    try {
      activeTurn = { bridge, turnRef }
      const raw = await bridge.start({
        turnRef,
        threadRef: send.id,
        message: send.message,
        // Capability I1: images ride the frozen start request additively.
        ...(send.images !== undefined && send.images.length > 0 ? { images: send.images } : {}),
        ...(send.target === undefined ? {} : { target: send.target }),
        ...(send.skill === undefined ? {} : { skill: send.skill }),
        ...(send.permissionMode === undefined ? {} : { permissionMode: send.permissionMode }),
        ...(lane !== "codex" || send.reasoningEffort === undefined ? {} : { reasoningEffort: send.reasoningEffort }),
        ...(send.model === undefined ? {} : { model: send.model }),
      })
      const result = decodeTurnResult(raw, laneLabel)
      // A3 queue-until-idle: a follow-up promoted at this turn's idle boundary
      // becomes the NEXT turn (the runtime emits it but does not run it). Chain
      // it on the same lane/onUpdate and return its result so the shell's
      // withTurnResult applies the final thread; `pending` stays true across
      // the chain because onUpdate keeps setting it. The promoted follow-up is
      // text-only (no images re-ride the chained turn).
      // `?? ""` reads the closure-assigned value at runtime (the onEvent
      // assignment above is invisible to CFA, which linearly narrows the
      // `= null` init and would otherwise make a `!== null` guard `never`).
      const promoted = promotedFollowup ?? ""
      if (promoted.trim() !== "" && result.ok) {
        return runLaneTurn(lane, bridge, { ...send, message: promoted, images: undefined })
      }
      return result
    } finally {
      activeTurn = null
      unsubscribe()
    }
  }

  return {
    ...input.base,
    /**
     * Interrupt the active local turn (EP250 Stop button). Signals the lane's
     * frozen interrupt channel by the exact active turnRef; the runtime aborts
     * and emits a typed `interrupted` failure that finalizes the turn.
     */
    interruptActive: async () => {
      const active = activeTurn
      if (active === null) return false
      const raw = await active.bridge.interrupt({ turnRef: active.turnRef })
      return raw === true
    },
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
      if (active === null || active.bridge.queueFollowup === undefined) {
        return { ok: false, queued: false }
      }
      const raw = await active.bridge.queueFollowup({
        threadRef: input.threadRef,
        message: input.message,
      })
      return typeof raw === "object" && raw !== null && typeof (raw as { queued?: unknown }).queued === "boolean"
        ? (raw as { ok: boolean; queued: boolean })
        : { ok: false, queued: false }
    },
    steerCurrent: async input => {
      const active = activeTurn
      if (active === null || active.bridge.steerCurrent === undefined) {
        return { ok: false, outcome: "not_found" }
      }
      const raw = await active.bridge.steerCurrent({
        threadRef: input.threadRef,
        message: input.message,
      })
      return typeof raw === "object" && raw !== null && typeof (raw as { outcome?: unknown }).outcome === "string"
        ? raw as { ok: boolean; outcome: string }
        : { ok: false, outcome: "not_found" }
    },
    sendMessage: async send => {
      if (send.harness === undefined) return input.base.sendMessage(send)
      if (send.harness === "codex") {
        // EP250 chip-verified-evidence rule: a send routes to the local
        // codex lane only over a PROBE-VERIFIED account; otherwise Send
        // refuses with the reason — never a silent substitution.
        const availability = input.codexAvailability?.() ?? null
        const bridge = input.codex ?? null
        if (bridge === null || availability === null || availability.state !== "available") {
          return { ok: false, error: codexLocalUnavailableMessage }
        }
        return runLaneTurn("codex", bridge, send)
      }
      const availability = input.fableAvailability()
      if (input.fable === null || availability === null || availability.state !== "available") {
        return {
          ok: false,
          error: fableLocalFailureMessage("no_claude_account", ""),
        }
      }
      return runLaneTurn("fable", input.fable, send)
    },
  }
}
