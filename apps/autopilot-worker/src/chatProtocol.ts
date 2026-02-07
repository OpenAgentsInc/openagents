export const MessageType = {
  CF_AGENT_CHAT_MESSAGES: "cf_agent_chat_messages",
  CF_AGENT_USE_CHAT_REQUEST: "cf_agent_use_chat_request",
  CF_AGENT_USE_CHAT_RESPONSE: "cf_agent_use_chat_response",
  CF_AGENT_CHAT_CLEAR: "cf_agent_chat_clear",
  CF_AGENT_CHAT_REQUEST_CANCEL: "cf_agent_chat_request_cancel",
  CF_AGENT_STREAM_RESUMING: "cf_agent_stream_resuming",
  CF_AGENT_STREAM_RESUME_ACK: "cf_agent_stream_resume_ack",
  CF_AGENT_TOOL_RESULT: "cf_agent_tool_result",
  CF_AGENT_MESSAGE_UPDATED: "cf_agent_message_updated",
} as const

export type MessageType = (typeof MessageType)[keyof typeof MessageType]

export type ChatRole = "user" | "assistant"

export type ChatTextPart = {
  readonly type: "text"
  readonly text: string
  readonly state?: "streaming" | "done"
}

export type ChatToolPart = {
  readonly type: `tool-${string}` | "dynamic-tool"
  readonly toolName?: string
  readonly toolCallId: string
  readonly state: string
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
  readonly preliminary?: boolean
  readonly approval?: { readonly id: string; readonly approved?: boolean; readonly reason?: string }
  readonly rawInput?: unknown
}

export type ChatPart = ChatTextPart | ChatToolPart | { readonly type: string; readonly [k: string]: unknown }

export type ChatMessage = {
  readonly id: string
  readonly role: ChatRole
  readonly parts: ReadonlyArray<ChatPart>
}

export type ChatMessagesOutgoing = {
  readonly type: typeof MessageType.CF_AGENT_CHAT_MESSAGES
  readonly messages: ReadonlyArray<ChatMessage>
}

export type MessageUpdatedOutgoing = {
  readonly type: typeof MessageType.CF_AGENT_MESSAGE_UPDATED
  readonly message: ChatMessage
}

export type ChatResponseOutgoing = {
  readonly type: typeof MessageType.CF_AGENT_USE_CHAT_RESPONSE
  readonly id: string
  readonly body: string
  readonly done: boolean
  readonly error?: boolean
  readonly continuation?: boolean
}

export type ChatClearOutgoing = { readonly type: typeof MessageType.CF_AGENT_CHAT_CLEAR }

