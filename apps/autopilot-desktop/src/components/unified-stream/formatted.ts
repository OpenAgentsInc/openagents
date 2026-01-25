import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
} from "../ai-elements/conversation.js"
import { Message, type MessageRole } from "../ai-elements/message.js"
import { Reasoning } from "../ai-elements/reasoning.js"
import { ToolCall } from "../ai-elements/tool-call.js"
import type {
  FormattedItem,
  FormattedMessage,
  FormattedReasoning,
  FormattedToolCall,
  FormattedState,
} from "./types.js"

const createItemId = (prefix: string, index: number) =>
  `${prefix}-${Date.now()}-${index}`

export const createFormattedState = (): FormattedState => ({
  items: [],
  messageIndex: null,
  reasoningIndex: null,
  toolIndexById: new Map(),
})

export const resetFormatted = (state: FormattedState) => {
  state.items = []
  state.messageIndex = null
  state.reasoningIndex = null
  state.toolIndexById.clear()
}

export const appendFormattedMessage = (
  state: FormattedState,
  role: MessageRole,
  text: string,
  isStreaming: boolean
): FormattedMessage => {
  const item: FormattedMessage = {
    kind: "message",
    id: createItemId("message", state.items.length),
    role,
    text,
    isStreaming,
  }
  state.items.push(item)
  state.messageIndex =
    role === "assistant" && isStreaming ? state.items.length - 1 : null
  return item
}

export const appendFormattedReasoning = (
  state: FormattedState,
  text: string,
  isStreaming: boolean
): FormattedReasoning => {
  const item: FormattedReasoning = {
    kind: "reasoning",
    id: createItemId("reasoning", state.items.length),
    summary: "Reasoning",
    content: text,
    isStreaming,
  }
  state.items.push(item)
  state.reasoningIndex = isStreaming ? state.items.length - 1 : null
  return item
}

export const applyToolCallStart = (
  state: FormattedState,
  toolId: string,
  title: string,
  detail: string
): FormattedToolCall => {
  const item: FormattedToolCall = {
    kind: "tool",
    id: toolId,
    title,
    detail,
    output: "",
    isStreaming: true,
    status: "running",
  }
  state.items.push(item)
  state.toolIndexById.set(toolId, state.items.length - 1)
  return item
}

export const applyToolCallUpdate = (
  state: FormattedState,
  toolId: string,
  output: string,
  isComplete: boolean
) => {
  const index = state.toolIndexById.get(toolId)
  if (index === undefined) {
    return
  }
  const item = state.items[index]
  if (item?.kind !== "tool") {
    return
  }
  item.output = `${item.output ?? ""}${output}`
  item.isStreaming = !isComplete
  item.status = isComplete ? "completed" : "running"
  if (isComplete) {
    state.toolIndexById.delete(toolId)
  }
}

export const applyMessageChunk = (
  state: FormattedState,
  content: string,
  isComplete: boolean
) => {
  const index = state.messageIndex
  if (
    index === null ||
    state.items[index]?.kind !== "message" ||
    state.items[index]?.role !== "assistant"
  ) {
    appendFormattedMessage(state, "assistant", content, !isComplete)
  } else {
    const message = state.items[index] as FormattedMessage
    message.text += content
    message.isStreaming = !isComplete
  }

  if (isComplete) {
    state.messageIndex = null
  }
}

export const applyReasoningChunk = (
  state: FormattedState,
  content: string,
  isComplete: boolean
) => {
  const index = state.reasoningIndex
  if (index === null || state.items[index]?.kind !== "reasoning") {
    appendFormattedReasoning(state, content, !isComplete)
  } else {
    const reasoning = state.items[index] as FormattedReasoning
    reasoning.content += content
    reasoning.isStreaming = !isComplete
  }

  if (isComplete) {
    state.reasoningIndex = null
  }
}

export const finalizeStreaming = (state: FormattedState) => {
  if (state.messageIndex !== null) {
    const message = state.items[state.messageIndex] as FormattedMessage
    message.isStreaming = false
    state.messageIndex = null
  }

  if (state.reasoningIndex !== null) {
    const reasoning = state.items[state.reasoningIndex] as FormattedReasoning
    reasoning.isStreaming = false
    state.reasoningIndex = null
  }

  for (const index of state.toolIndexById.values()) {
    const item = state.items[index]
    if (item?.kind === "tool") {
      item.isStreaming = false
      item.status = "completed"
    }
  }
  state.toolIndexById.clear()
}

export const renderFormattedConversation = (
  items: FormattedItem[]
): TemplateResult => {
  const content =
    items.length === 0
      ? ConversationEmptyState({
          title: "No formatted messages yet",
          description: "Send a prompt to see the formatted conversation.",
        })
      : html`${items.map((item) => {
          switch (item.kind) {
            case "message":
              return Message({
                role: item.role,
                text: item.text,
                isStreaming: item.isStreaming,
              })
            case "reasoning":
              return Reasoning({
                summary: item.summary,
                content: item.content,
                isStreaming: item.isStreaming,
                open: item.isStreaming,
              })
            case "tool":
              return ToolCall({
                title: item.title,
                detail: item.detail,
                output: item.output,
                isStreaming: item.isStreaming,
                status: item.status,
              })
          }
        })}`

  return Conversation({
    children: ConversationContent({ children: content }),
  })
}
