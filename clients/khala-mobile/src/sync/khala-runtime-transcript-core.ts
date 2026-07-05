import type { KhalaRuntimeEvent, KhalaRuntimeLane, RuntimeEventEntity } from "@openagentsinc/khala-sync"

/** One renderable, ordered piece of a runtime transcript — folded from the
 * raw KhalaRuntimeEvent stream, preserving temporal order (unlike a
 * per-id-keyed reducer, which loses interleaving between text and tool
 * calls). Each part type maps to a distinct mobile UI component. */
export type TranscriptPart =
  | Readonly<{ kind: "text"; id: string; text: string }>
  | Readonly<{ kind: "reasoning"; id: string; text: string }>
  | Readonly<{
      kind: "tool"
      id: string
      toolCallId: string
      toolName: string
      status: "called" | "completed" | "failed"
      errorMessageSafe?: string
    }>
  | Readonly<{ kind: "usage"; id: string; inputTokens?: number; outputTokens?: number; totalTokens?: number }>
  | Readonly<{
      kind: "turn-status"
      id: string
      status: "running" | "completed" | "failed" | "interrupted"
      /** Which provider/adapter produced this turn (#8405) — read directly off
       * the `turn.started`/`turn.interrupted`/`turn.finished` event's own
       * `source.lane`, the same lane value `RuntimeTurnEntity.lane` carries
       * for this turn. Purely a display addition; never affects dispatch. */
      lane: KhalaRuntimeLane
    }>

export const sortEventsBySequence = (
  entities: ReadonlyArray<RuntimeEventEntity>
): ReadonlyArray<RuntimeEventEntity> => [...entities].sort((a, b) => a.sequence - b.sequence)

/** Folds an ordered event list into an ordered list of renderable parts.
 * Deltas (text.delta / reasoning.delta) accumulate into the LAST open part
 * for that messageId if it's already the most recent part; otherwise a new
 * part is appended — this keeps streamed chunks merged into one growing
 * bubble instead of one bubble per chunk. */
export const reduceRuntimeTranscript = (
  events: ReadonlyArray<KhalaRuntimeEvent>
): ReadonlyArray<TranscriptPart> => {
  const parts: Array<TranscriptPart> = []

  const appendOrGrowText = (kind: "text" | "reasoning", messageId: string, delta: string): void => {
    const last = parts[parts.length - 1]
    if (last !== undefined && last.kind === kind && last.id === messageId) {
      parts[parts.length - 1] = { ...last, text: last.text + delta }
      return
    }
    parts.push({ id: messageId, kind, text: delta })
  }

  for (const event of events) {
    switch (event.kind) {
      case "turn.started":
        parts.push({ id: `turn-status-${parts.length}`, kind: "turn-status", lane: event.source.lane, status: "running" })
        break
      case "turn.interrupted":
        parts.push({ id: `turn-status-${parts.length}`, kind: "turn-status", lane: event.source.lane, status: "interrupted" })
        break
      case "turn.finished":
        parts.push({
          id: `turn-status-${parts.length}`,
          kind: "turn-status",
          lane: event.source.lane,
          status: event.finishReason === "error" ? "failed" : "completed"
        })
        break
      case "text.delta":
        appendOrGrowText("text", event.messageId, event.text)
        break
      case "reasoning.delta":
        appendOrGrowText("reasoning", event.messageId, event.text)
        break
      case "tool.call":
        parts.push({
          id: `tool-${event.toolCallId}`,
          kind: "tool",
          status: "called",
          toolCallId: event.toolCallId,
          toolName: event.toolName
        })
        break
      case "tool.result":
      case "tool.error": {
        const idx = parts.findIndex(
          p => p.kind === "tool" && p.toolCallId === event.toolCallId
        )
        const updated: TranscriptPart = {
          id: `tool-${event.toolCallId}`,
          kind: "tool",
          status: event.kind === "tool.result" ? "completed" : "failed",
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          ...(event.kind === "tool.error" ? { errorMessageSafe: event.messageSafe } : {})
        }
        if (idx === -1) parts.push(updated)
        else parts[idx] = updated
        break
      }
      case "usage.recorded":
        parts.push({
          id: `usage-${parts.length}`,
          kind: "usage",
          ...(event.usage.inputTokens === undefined ? {} : { inputTokens: event.usage.inputTokens }),
          ...(event.usage.outputTokens === undefined ? {} : { outputTokens: event.usage.outputTokens }),
          ...(event.usage.totalTokens === undefined ? {} : { totalTokens: event.usage.totalTokens })
        })
        break
      default:
        break
    }
  }

  return parts
}
