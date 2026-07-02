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

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

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

const usageFromUsageObject = (usage: Record<string, unknown> | null): KhalaCodeDesktopUsage | undefined => {
  if (usage === null) return undefined
  const input = numberField(usage, "input_tokens") + numberField(usage, "cache_creation_input_tokens")
  const cachedInput = numberField(usage, "cache_read_input_tokens")
  const output = numberField(usage, "output_tokens")
  const reasoningOutput = numberField(usage, "reasoning_output_tokens")
  if (input + cachedInput + output + reasoningOutput === 0) return undefined
  return { input, cachedInput, output, reasoningOutput }
}

const usageFromClaude = (message: Record<string, unknown>): KhalaCodeDesktopUsage | undefined =>
  usageFromUsageObject(objectField(message, "usage") ?? objectField(message, "modelUsage"))

const usageObjectsFrom = (value: Record<string, unknown> | null): readonly Record<string, unknown>[] => {
  if (value === null) return []
  if (
    numberField(value, "input_tokens") +
    numberField(value, "cache_creation_input_tokens") +
    numberField(value, "cache_read_input_tokens") +
    numberField(value, "output_tokens") +
    numberField(value, "reasoning_output_tokens") > 0
  ) return [value]
  return Object.values(value).filter(isObject)
}

const addUsage = (
  left: KhalaCodeDesktopUsage | undefined,
  right: KhalaCodeDesktopUsage | undefined,
): KhalaCodeDesktopUsage | undefined => {
  if (left === undefined) return right
  if (right === undefined) return left
  return {
    cachedInput: left.cachedInput + right.cachedInput,
    input: left.input + right.input,
    output: left.output + right.output,
    reasoningOutput: left.reasoningOutput + right.reasoningOutput,
  }
}

const usageFromClaudeResult = (message: Record<string, unknown>): KhalaCodeDesktopUsage | undefined => {
  const direct =
    usageFromClaude(message) ??
    usageFromClaude(objectField(message, "message") ?? {})
  const modelUsage = usageObjectsFrom(objectField(message, "modelUsage"))
    .map(usageFromUsageObject)
    .reduce(addUsage, undefined)
  return addUsage(direct, modelUsage)
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
  const streamBlockIds = new Map<number, string>()
  const streamBlockKinds = new Map<number, string>()

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
            kind: "tool_started",
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
            kind: "tool_completed",
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

  const projectStreamEvent = (raw: Record<string, unknown>): ClaudeProjectedMessage => {
    const event = objectField(raw, "event") ?? raw
    const eventType = stringField(event, "type") ?? stringField(raw, "event_type") ?? ""
    const indexValue = event.index ?? raw.index
    const index = typeof indexValue === "number" && Number.isInteger(indexValue) ? indexValue : 0
    const events: KhalaCodeDesktopChatTurnEvent[] = []

    if (eventType === "content_block_start") {
      const block = objectField(event, "content_block") ?? objectField(raw, "content_block") ?? {}
      const blockType = stringField(block, "type") ?? "text"
      if (blockType !== "text" && blockType !== "thinking") {
        return { events, messages, status, toolNames: [...toolNames], usage }
      }
      const id = `${input.turnId}-claude-stream-${index}`
      streamBlockIds.set(index, id)
      streamBlockKinds.set(index, blockType)
      const message: KhalaCodeDesktopMessage = {
        body: "",
        id,
        role: "assistant",
        ...(blockType === "thinking" ? {
          harnessItem: {
            itemId: id,
            itemType: "reasoning",
            status: "running",
            title: "Claude reasoning",
          },
        } : {}),
      }
      appendMessage(message)
      events.push({ message, turnId: input.turnId, type: "message_start" })
      return { events, messages, status, toolNames: [...toolNames], usage }
    }

    if (eventType === "content_block_delta") {
      const delta = objectField(event, "delta") ?? objectField(raw, "delta") ?? event
      const text = stringField(delta, "text") ?? ""
      if (text.length === 0) return { events, messages, status, toolNames: [...toolNames], usage }
      const id = streamBlockIds.get(index) ?? `${input.turnId}-claude-stream-${index}`
      const blockType = streamBlockKinds.get(index) ?? "text"
      if (!streamBlockIds.has(index)) {
        streamBlockIds.set(index, id)
        streamBlockKinds.set(index, blockType)
        const message: KhalaCodeDesktopMessage = {
          body: "",
          id,
          role: "assistant",
          ...(blockType === "thinking" ? {
            harnessItem: {
              itemId: id,
              itemType: "reasoning",
              status: "running",
              title: "Claude reasoning",
            },
          } : {}),
        }
        appendMessage(message)
        events.push({ message, turnId: input.turnId, type: "message_start" })
      }
      const existing = messages.find(message => message.id === id)
      appendMessage({
        ...(existing ?? { id, role: "assistant" as const }),
        body: `${existing?.body ?? ""}${text}`,
      })
      events.push({ delta: text, messageId: id, turnId: input.turnId, type: "message_delta" })
      return { events, messages, status, toolNames: [...toolNames], usage }
    }

    if (eventType === "content_block_stop") {
      const id = streamBlockIds.get(index)
      if (id !== undefined) events.push({ messageId: id, turnId: input.turnId, type: "message_done" })
      return { events, messages, status, toolNames: [...toolNames], usage }
    }

    return { events, messages, status, toolNames: [...toolNames], usage }
  }

  return {
    messages: () => messages,
    project(message) {
      if (!isObject(message)) return { events: [], messages, status, toolNames: [...toolNames], usage }
      if (message.type === "stream_event") return projectStreamEvent(message)
      let decoded: Record<string, unknown>
      try {
        decoded = S.decodeUnknownSync(ClaudeSdkMessageSchema as never)(message) as Record<string, unknown>
      } catch {
        return { events: [], messages, status, toolNames: [...toolNames], usage }
      }
      if (decoded.type === "assistant") return projectAssistant(decoded as Parameters<typeof projectAssistant>[0])
      if (decoded.type === "user") return projectUser(decoded as Parameters<typeof projectUser>[0])
      if (decoded.type === "result") {
        const subtype = typeof decoded.subtype === "string" ? decoded.subtype : ""
        status = subtype.startsWith("error") ? "failed" : subtype === "interrupted" ? "interrupted" : "completed"
        usage = usageFromClaudeResult(message)
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
