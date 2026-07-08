/**
 * Desktop chat-runtime event -> `khala.chat_turn_event.v1` projection.
 *
 * The codex and claude chat runtimes
 * (`clients/khala-code-desktop/src/bun/codex-app-server-chat-runtime.ts`,
 * `claude-app-sdk-chat-runtime.ts`) both emit the desktop-specialized
 * `KhalaCodeDesktopChatTurnEvent` union. MH-1's job (analysis §5 prerequisite)
 * is to prove those events map onto the shared, versioned, harness-neutral
 * `khala.chat_turn_event.v1` contract in `@openagentsinc/agent-runtime-schema`,
 * so mobile projections and Khala Sync capture can depend on the neutral spine
 * without pulling any desktop type.
 *
 * This projector is intentionally total over the real desktop event union
 * (imported, not re-declared): the desktop `message` shape is the neutral
 * `{ id, role, body }` message plus desktop-only `codexItem`/`harnessItem`
 * cards, and the desktop `tool_event` payload is already the neutral tool-event
 * envelope. Projection therefore drops the desktop-only card fields and keeps
 * the neutral spine verbatim.
 */
import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopMessage,
} from "@openagentsinc/khala-code-desktop/src/shared/rpc.ts"
import type {
  KhalaChatTurnEventMessage,
  KhalaChatTurnEventV1,
} from "@openagentsinc/agent-runtime-schema"

const projectMessage = (
  message: KhalaCodeDesktopMessage,
): KhalaChatTurnEventMessage => ({
  id: message.id,
  role: message.role,
  body: message.body,
})

export function projectDesktopChatTurnEventToV1(
  event: KhalaCodeDesktopChatTurnEvent,
): KhalaChatTurnEventV1 {
  switch (event.type) {
    case "thread_ready":
      return { type: "thread_ready", threadId: event.threadId, turnId: event.turnId }
    case "message_start":
      return { type: "message_start", turnId: event.turnId, message: projectMessage(event.message) }
    case "message_delta":
      return {
        type: "message_delta",
        turnId: event.turnId,
        messageId: event.messageId,
        delta: event.delta,
      }
    case "message_replace":
      return { type: "message_replace", turnId: event.turnId, message: projectMessage(event.message) }
    case "message_done":
      return { type: "message_done", turnId: event.turnId, messageId: event.messageId }
    case "tool_event":
      return {
        type: "tool_event",
        turnId: event.turnId,
        event: {
          eventId: event.event.eventId,
          ...(event.event.invocationId === undefined
            ? {}
            : { invocationId: event.event.invocationId }),
          kind: event.event.kind,
          sessionId: event.event.sessionId,
          payload: event.event.payload,
        },
      }
  }
}
