/**
 * Harness-owned compatibility names for the neutral chat-turn contract.
 *
 * The removed Khala Code desktop app specialized this shape only with optional
 * presentation cards. Harness conformance never consumes those cards, so its
 * executable boundary is the canonical versioned runtime schema.
 */
export { KhalaChatTurnEventV1 as KhalaCodeDesktopChatTurnEventSchema } from "@openagentsinc/agent-runtime-schema"
export type {
  KhalaChatTurnEventV1 as KhalaCodeDesktopChatTurnEvent,
  KhalaChatTurnEventMessage as KhalaCodeDesktopMessage,
} from "@openagentsinc/agent-runtime-schema"
