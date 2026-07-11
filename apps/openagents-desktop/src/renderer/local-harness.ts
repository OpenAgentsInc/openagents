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
} from "../fable-local-contract.ts"
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

  /** One shared streaming projection for both local lanes: trace lines
   * first, growing assistant bubble last — the same order the finalized
   * persisted thread carries, so finalize never reshuffles the transcript. */
  const runLaneTurn = async (
    lane: "fable" | "codex",
    bridge: FableLocalRendererBridge,
    send: Readonly<{
      id: string
      message: string
      onUpdate?: (thread: DesktopThread) => void
    }>,
  ): Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>> => {
    const laneLabel = lane === "fable" ? "Fable" : "Codex"
    const turnRef = `turn.${lane}.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
    let baseThread: DesktopThread | null = null
    const traceNotes: DesktopMessage[] = []
    let assistantNote: DesktopMessage | null = null
    let effectiveModel: string | null = null
    const laneName = lane === "fable" ? "fable-local" : "codex-local"

    const project = (): void => {
      if (baseThread === null || send.onUpdate === undefined) return
      send.onUpdate({
        ...baseThread,
        notes: [
          ...baseThread.notes,
          ...traceNotes,
          ...(assistantNote === null ? [] : [assistantNote]),
        ],
      })
    }
    const pushSystemNote = (text: string): void => {
      traceNotes.push({
        key: `${turnRef}-trace-${traceNotes.length}`,
        role: "system",
        text,
        timestamp: noteTimestamp(now()),
      })
      project()
    }

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
        assistantNote = assistantNote === null
          ? {
              key: `${turnRef}-assistant`,
              role: "assistant",
              text: event.text,
              timestamp: noteTimestamp(now()),
              // Streaming metadata (#8712): lane + turn are known here; the
              // effective model joins when its typed event lands. The
              // persisted note from main carries the authoritative meta.
              meta: { lane: laneName, turnRef, ...(effectiveModel === null ? {} : { model: effectiveModel }) },
            }
          : { ...assistantNote, text: assistantNote.text + event.text }
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
        if (assistantNote !== null) {
          assistantNote = {
            ...assistantNote,
            meta: { ...assistantNote.meta, lane: laneName, turnRef, model: event.model },
          }
        }
        pushSystemNote(lane === "fable"
          ? fableLocalModelNoteText(event.model)
          : codexLocalModelNoteText(event.model))
        return
      }
      if (event.kind === "tool_use" || event.kind === "tool_result") {
        traceNotes.push({
          key: `${turnRef}-trace-${traceNotes.length}`,
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
        traceNotes.push({
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
        const index = traceNotes.findIndex(
          note => note.question?.questionRef === event.questionRef,
        )
        const existing = index === -1 ? undefined : traceNotes[index]
        if (existing?.question !== undefined) {
          traceNotes[index] = {
            ...existing,
            question: { ...existing.question, status: event.outcome },
          }
          project()
        }
        return
      }
      // turn_completed / turn_failed carry no transcript body of their
      // own; the invoke result finalizes the thread.
    })
    try {
      const raw = await bridge.start({ turnRef, threadRef: send.id, message: send.message })
      return decodeTurnResult(raw, laneLabel)
    } finally {
      unsubscribe()
    }
  }

  return {
    ...input.base,
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
