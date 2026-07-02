import { Schema as S } from "effect"

import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopUsage,
} from "../shared/rpc.js"

const ClaudeContentBlock = S.Record(S.String, S.Unknown)
const UnknownRecord = S.Record(S.String, S.Unknown)

const ClaudeSystemMessage = S.Struct({
  type: S.Literal("system"),
  subtype: S.optional(S.String),
  uuid: S.optional(S.String),
  session_id: S.optional(S.String),
})

const ClaudeAssistantMessage = S.Struct({
  type: S.Literal("assistant"),
  uuid: S.optional(S.String),
  session_id: S.optional(S.String),
  message: S.Struct({
    content: S.Array(ClaudeContentBlock),
  }),
})

const ClaudeUserMessage = S.Struct({
  type: S.Literal("user"),
  uuid: S.optional(S.String),
  session_id: S.optional(S.String),
  message: S.Struct({
    content: S.Union([S.String, S.Array(ClaudeContentBlock)]),
  }),
})

const ClaudeResultMessage = S.Struct({
  type: S.Literal("result"),
  subtype: S.optional(S.String),
  uuid: S.optional(S.String),
  session_id: S.optional(S.String),
  usage: S.optional(UnknownRecord),
  modelUsage: S.optional(UnknownRecord),
})

const ClaudeStreamEventMessage = S.Struct({
  type: S.Literal("stream_event"),
  uuid: S.optional(S.String),
  session_id: S.optional(S.String),
})

const ClaudeSessionStateChangedMessage = S.Struct({
  type: S.Literal("system"),
  subtype: S.Literal("session_state_changed"),
  uuid: S.optional(S.String),
  session_id: S.optional(S.String),
  state: S.optional(S.String),
})

export const ClaudeSdkMessageSchema = S.Union([
  ClaudeAssistantMessage,
  ClaudeUserMessage,
  ClaudeResultMessage,
  ClaudeStreamEventMessage,
  ClaudeSessionStateChangedMessage,
  ClaudeSystemMessage,
])
export type ClaudeSdkMessage = typeof ClaudeSdkMessageSchema.Type

export type ClaudeProjectedMessage = {
  readonly events: readonly KhalaCodeDesktopChatTurnEvent[]
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly status: string
  readonly toolNames: readonly string[]
  readonly usage?: KhalaCodeDesktopUsage | undefined
}

export type ClaudeThreadItemProjector = Readonly<{
  messages: () => readonly KhalaCodeDesktopMessage[]
  project: (message: unknown) => ClaudeProjectedMessage
  status: () => string
  toolNames: () => readonly string[]
  usage: () => KhalaCodeDesktopUsage | undefined
}>

const stringField = (value: Record<string, unknown>, field: string): string | null => {
  const candidate = value[field]
  return typeof candidate === "string" && candidate.length > 0 ? candidate : null
}

const objectField = (value: Record<string, unknown>, field: string): Record<string, unknown> | null => {
  const candidate = value[field]
  return typeof candidate === "object" && candidate !== null && !Array.isArray(candidate)
    ? candidate as Record<string, unknown>
    : null
}

const numberField = (value: Record<string, unknown> | null | undefined, field: string): number => {
  const candidate = value?.[field]
  return typeof candidate === "number" && Number.isFinite(candidate) ? candidate : 0
}

const contentText = (block: Record<string, unknown>): string => {
  const text = block.text
  if (typeof text === "string") return text
  const partial = objectField(block, "delta")
  const deltaText = partial === null ? null : stringField(partial, "text")
  return deltaText ?? ""
}

const messageId = (
  turnId: string,
  prefix: string,
  message: Record<string, unknown>,
  index: number,
): string => `${stringField(message, "uuid") ?? turnId}-${prefix}-${index}`

const usageFromClaude = (message: Record<string, unknown>): KhalaCodeDesktopUsage | undefined => {
  const usage = objectField(message, "usage") ?? objectField(message, "modelUsage")
  if (usage === null) return undefined
  const input = numberField(usage, "input_tokens") + numberField(usage, "cache_creation_input_tokens")
  const cachedInput = numberField(usage, "cache_read_input_tokens")
  const output = numberField(usage, "output_tokens")
  const reasoningOutput = numberField(usage, "reasoning_output_tokens")
  if (input + cachedInput + output + reasoningOutput === 0) return undefined
  return { input, cachedInput, output, reasoningOutput }
}

export function createClaudeThreadItemProjector(input: {
  readonly desktopSessionId: string
  readonly turnId: string
}): ClaudeThreadItemProjector {
  const messages: KhalaCodeDesktopMessage[] = []
  const toolNames = new Set<string>()
  let status = "running"
  let usage: KhalaCodeDesktopUsage | undefined
  let assistantCount = 0

  const appendMessage = (message: KhalaCodeDesktopMessage): void => {
    const existing = messages.findIndex(candidate => candidate.id === message.id)
    if (existing === -1) messages.push(message)
    else messages[existing] = message
  }

  const projectAssistant = (decoded: {
    readonly message: { readonly content: readonly Record<string, unknown>[] }
    readonly uuid?: string
  }): ClaudeProjectedMessage => {
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const raw = decoded as Record<string, unknown>
    const content = decoded.message.content as readonly Record<string, unknown>[]
    for (const [index, block] of content.entries()) {
      const blockType = stringField(block, "type") ?? "text"
      if (blockType === "text" || blockType === "thinking") {
        const body = contentText(block)
        if (body.length === 0) continue
        const id = messageId(input.turnId, blockType, raw, assistantCount++)
        const message: KhalaCodeDesktopMessage = {
          body,
          id,
          role: "assistant",
          ...(blockType === "thinking" ? {
            harnessItem: {
              itemId: id,
              itemType: "reasoning",
              status: "completed",
              title: "Claude reasoning",
            },
          } : {}),
        }
        appendMessage(message)
        events.push({ message: { ...message, body: "" }, turnId: input.turnId, type: "message_start" })
        events.push({ delta: body, messageId: id, turnId: input.turnId, type: "message_delta" })
        events.push({ messageId: id, turnId: input.turnId, type: "message_done" })
        continue
      }
      if (blockType === "tool_use") {
        const toolName = stringField(block, "name") ?? "claude_tool"
        const toolUseId = stringField(block, "id") ?? `${input.turnId}-claude-tool-${index}`
        toolNames.add(toolName)
        events.push({
          event: {
            eventId: `${toolUseId}-started`,
            invocationId: toolUseId,
            kind: "tool.started",
            payload: block,
            sessionId: input.desktopSessionId,
          },
          turnId: input.turnId,
          type: "tool_event",
        })
      }
    }
    return { events, messages, status, toolNames: [...toolNames], usage }
  }

  const projectUser = (decoded: {
    readonly message: { readonly content: string | readonly Record<string, unknown>[] }
  }): ClaudeProjectedMessage => {
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const content = decoded.message.content
    if (Array.isArray(content)) {
      for (const [index, block] of content.entries()) {
        if (stringField(block, "type") !== "tool_result") continue
        const toolUseId = stringField(block, "tool_use_id") ?? `${input.turnId}-claude-tool-result-${index}`
        events.push({
          event: {
            eventId: `${toolUseId}-completed`,
            invocationId: toolUseId,
            kind: "tool.completed",
            payload: block,
            sessionId: input.desktopSessionId,
          },
          turnId: input.turnId,
          type: "tool_event",
        })
      }
    }
    return { events, messages, status, toolNames: [...toolNames], usage }
  }

  return {
    messages: () => messages,
    project(message) {
      const decoded = S.decodeUnknownSync(ClaudeSdkMessageSchema as never)(message) as Record<string, unknown>
      if (decoded.type === "assistant") return projectAssistant(decoded as Parameters<typeof projectAssistant>[0])
      if (decoded.type === "user") return projectUser(decoded as Parameters<typeof projectUser>[0])
      if (decoded.type === "result") {
        const subtype = typeof decoded.subtype === "string" ? decoded.subtype : ""
        status = subtype.startsWith("error") ? "failed" : "completed"
        usage = usageFromClaude(decoded as Record<string, unknown>)
        return { events: [], messages, status, toolNames: [...toolNames], usage }
      }
      if (decoded.type === "system" && decoded.subtype === "session_state_changed") {
        status = typeof decoded.state === "string" ? decoded.state : status
      }
      return { events: [], messages, status, toolNames: [...toolNames], usage }
    },
    status: () => status,
    toolNames: () => [...toolNames],
    usage: () => usage,
  }
}
