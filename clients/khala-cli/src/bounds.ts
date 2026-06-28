import {
  KHALA_CHAT_MAX_MESSAGE_CHARS,
  KHALA_CHAT_MAX_MESSAGES,
  KHALA_CHAT_MAX_TOTAL_CHARS,
  KhalaCliError,
  type KhalaChatMessage,
} from "./types.js"

export function totalMessageChars(messages: ReadonlyArray<KhalaChatMessage>): number {
  return messages.reduce((sum, message) => sum + message.content.length, 0)
}

export function prepareUserTurn(
  history: ReadonlyArray<KhalaChatMessage>,
  prompt: string,
): ReadonlyArray<KhalaChatMessage> {
  const content = prompt.trim()
  if (content.length === 0) {
    throw new KhalaCliError({ reason: "Prompt cannot be empty.", code: "empty_prompt" })
  }
  if (content.length > KHALA_CHAT_MAX_MESSAGE_CHARS) {
    throw new KhalaCliError({
      reason: `Prompt is ${content.length} characters; Khala accepts at most ${KHALA_CHAT_MAX_MESSAGE_CHARS} per message.`,
      code: "message_too_long",
    })
  }
  return trimConversation([...history, { role: "user", content }])
}

export function appendAssistantTurn(
  history: ReadonlyArray<KhalaChatMessage>,
  content: string,
): ReadonlyArray<KhalaChatMessage> {
  return trimConversation([...history, { role: "assistant", content }])
}

export function validatePublicConversation(messages: ReadonlyArray<KhalaChatMessage>): void {
  if (messages.length === 0) {
    throw new KhalaCliError({ reason: "Conversation cannot be empty.", code: "empty_conversation" })
  }
  if (messages.length > KHALA_CHAT_MAX_MESSAGES) {
    throw new KhalaCliError({
      reason: `Conversation has ${messages.length} messages; Khala accepts at most ${KHALA_CHAT_MAX_MESSAGES}.`,
      code: "too_many_messages",
    })
  }
  for (const message of messages) {
    if (message.content.trim().length === 0) {
      throw new KhalaCliError({ reason: "Conversation contains an empty message.", code: "empty_message" })
    }
    if (message.content.length > KHALA_CHAT_MAX_MESSAGE_CHARS) {
      throw new KhalaCliError({
        reason: `A ${message.role} message is ${message.content.length} characters; Khala accepts at most ${KHALA_CHAT_MAX_MESSAGE_CHARS}.`,
        code: "message_too_long",
      })
    }
  }
  if (totalMessageChars(messages) > KHALA_CHAT_MAX_TOTAL_CHARS) {
    throw new KhalaCliError({
      reason: `Conversation is ${totalMessageChars(messages)} characters; Khala accepts at most ${KHALA_CHAT_MAX_TOTAL_CHARS}.`,
      code: "conversation_too_long",
    })
  }
  const last = messages[messages.length - 1]
  if (last?.role !== "user") {
    throw new KhalaCliError({ reason: "The newest message must be from the user.", code: "last_message_not_user" })
  }
}

export function trimConversation(messages: ReadonlyArray<KhalaChatMessage>): ReadonlyArray<KhalaChatMessage> {
  const trimmed = [...messages]
  while (trimmed.length > KHALA_CHAT_MAX_MESSAGES) {
    trimmed.shift()
  }
  while (trimmed.length > 1 && totalMessageChars(trimmed) > KHALA_CHAT_MAX_TOTAL_CHARS) {
    trimmed.shift()
  }
  if (totalMessageChars(trimmed) > KHALA_CHAT_MAX_TOTAL_CHARS) {
    throw new KhalaCliError({
      reason: `Newest prompt exceeds Khala's total ${KHALA_CHAT_MAX_TOTAL_CHARS} character conversation bound.`,
      code: "conversation_too_long",
    })
  }
  return trimmed
}
