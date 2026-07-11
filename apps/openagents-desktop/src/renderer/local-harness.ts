/**
 * Local-mode harness routing (#8712): the chat host used when the app is NOT
 * signed in (Khala Sync conversation catalog not live).
 *
 * The law this module enforces in code: selecting a harness can never route
 * to the cloud gateway or another provider.
 * - "fable" streams a real local Claude turn through the fableLocal bridge
 *   (typed unavailable error when no linked Claude account home exists).
 * - "codex" has no local lane yet: an explicit typed refusal, never the
 *   legacy gateway.
 * - Only a laneless send (no harness — programmatic callers) reaches the
 *   base host's legacy `completeChatTurn` fallback.
 */
import type { DesktopMessage, DesktopThread } from "../chat-contract.ts"
import {
  fableLocalFailureMessage,
  fableLocalModelNoteText,
  fableLocalTraceNoteText,
  type FableLocalAvailability,
  type FableLocalEventEnvelope,
} from "../fable-local-contract.ts"
import type { ChatHost } from "./shell.ts"

export type FableLocalRendererBridge = Readonly<{
  availability: () => Promise<unknown>
  start: (value: unknown) => Promise<unknown>
  interrupt: (value: unknown) => Promise<unknown>
  onEvent: (listener: (envelope: FableLocalEventEnvelope) => void) => () => void
}>

export const codexLocalUnavailableMessage =
  "Codex requires an OpenAgents session. Sign in to route Codex turns through the runtime — no message was sent to any other lane."

const noteTimestamp = (now: Date = new Date()): string =>
  `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`

const decodeTurnResult = (
  raw: unknown,
): Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }> => {
  if (typeof raw === "object" && raw !== null && typeof (raw as { ok?: unknown }).ok === "boolean") {
    return raw as { ok: boolean; thread?: DesktopThread | null; error?: string }
  }
  return { ok: false, error: "The local Fable lane returned an invalid response." }
}

export type MakeLocalHarnessChatHostInput = Readonly<{
  base: ChatHost
  fable: FableLocalRendererBridge | null
  fableAvailability: () => FableLocalAvailability | null
  randomId?: () => string
  now?: () => Date
}>

export const makeLocalHarnessChatHost = (input: MakeLocalHarnessChatHostInput): ChatHost => {
  const randomId = input.randomId ?? (() => globalThis.crypto.randomUUID())
  const now = input.now ?? (() => new Date())
  return {
    ...input.base,
    sendMessage: async send => {
      if (send.harness === undefined) return input.base.sendMessage(send)
      if (send.harness === "codex") {
        return { ok: false, error: codexLocalUnavailableMessage }
      }
      const availability = input.fableAvailability()
      if (input.fable === null || availability === null || availability.state !== "available") {
        return {
          ok: false,
          error: fableLocalFailureMessage("no_claude_account", ""),
        }
      }
      const fable = input.fable
      const turnRef = `turn.fable.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
      let baseThread: DesktopThread | null = null
      const traceNotes: DesktopMessage[] = []
      let assistantNote: DesktopMessage | null = null
      let effectiveModel: string | null = null

      // Trace lines first, growing assistant bubble last — the same order the
      // finalized persisted thread carries (main appends traces before the
      // final assistant message), so finalize never reshuffles the transcript.
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

      const unsubscribe = fable.onEvent(envelope => {
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
                meta: { lane: "fable-local", turnRef, ...(effectiveModel === null ? {} : { model: effectiveModel }) },
              }
            : { ...assistantNote, text: assistantNote.text + event.text }
          project()
          return
        }
        if (event.kind === "model_effective") {
          // Effective-model caption above the assistant reply ("Fable ·
          // claude-fable-5") — model identity comes from the SDK init report,
          // never from the lane brand alone. Main persists the same line, so
          // finalize does not reshuffle the transcript.
          effectiveModel = event.model
          if (assistantNote !== null) {
            assistantNote = {
              ...assistantNote,
              meta: { ...assistantNote.meta, lane: "fable-local", turnRef, model: event.model },
            }
          }
          traceNotes.push({
            key: `${turnRef}-trace-${traceNotes.length}`,
            role: "system",
            text: fableLocalModelNoteText(event.model),
            timestamp: noteTimestamp(now()),
          })
          project()
          return
        }
        if (event.kind === "tool_use" || event.kind === "tool_result") {
          traceNotes.push({
            key: `${turnRef}-trace-${traceNotes.length}`,
            role: "system",
            text: fableLocalTraceNoteText(event),
            timestamp: noteTimestamp(now()),
          })
          project()
        }
        // turn_completed / turn_failed carry no transcript body of their
        // own; the invoke result finalizes the thread.
      })
      try {
        const raw = await fable.start({ turnRef, threadRef: send.id, message: send.message })
        return decodeTurnResult(raw)
      } finally {
        unsubscribe()
      }
    },
  }
}
