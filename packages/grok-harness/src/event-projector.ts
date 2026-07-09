import type { NeutralChatTurnEvent } from "./types.ts"

export type AcpSessionUpdate = {
  readonly sessionUpdate?: string
  readonly content?: { readonly type?: string; readonly text?: string }
  readonly toolCallId?: string
  readonly title?: string
  readonly status?: string
}

/**
 * Project ACP session/update params into neutral chat turn events.
 * Assistant text streams as agent_message_chunk; we emit message_start once,
 * then deltas, then message_done when the prompt result settles.
 */
export function createGrokAcpEventProjector(input: {
  readonly threadId: string
  readonly turnId: string
  readonly messageId?: string
}): {
  readonly onUpdate: (update: AcpSessionUpdate) => readonly NeutralChatTurnEvent[]
  readonly finish: () => readonly NeutralChatTurnEvent[]
  readonly text: () => string
} {
  const messageId = input.messageId ?? `msg_${input.turnId}`
  let started = false
  let full = ""

  return {
    text: () => full,
    onUpdate(update) {
      const events: NeutralChatTurnEvent[] = []
      if (update.sessionUpdate === "agent_message_chunk" && update.content?.text) {
        const delta = update.content.text
        if (!started) {
          started = true
          events.push({
            type: "message_start",
            turnId: input.turnId,
            message: { id: messageId, role: "assistant", content: "" },
          })
        }
        full += delta
        events.push({
          type: "message_delta",
          turnId: input.turnId,
          messageId,
          delta,
        })
      }
      if (update.sessionUpdate === "tool_call" || update.sessionUpdate === "tool_call_update") {
        events.push({
          type: "tool_event",
          turnId: input.turnId,
          event: {
            kind: update.sessionUpdate,
            ...(update.title === undefined ? {} : { name: update.title }),
            ...(update.status === undefined ? {} : { detail: update.status }),
          },
        })
      }
      return events
    },
    finish() {
      if (!started) {
        return [
          {
            type: "message_start",
            turnId: input.turnId,
            message: { id: messageId, role: "assistant", content: "" },
          },
          {
            type: "message_done",
            turnId: input.turnId,
            messageId,
          },
        ]
      }
      return [
        {
          type: "message_done",
          turnId: input.turnId,
          messageId,
        },
      ]
    },
  }
}
