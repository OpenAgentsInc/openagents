import { Schema as S } from "effect"
import type { KhalaToolEvent } from "@openagentsinc/khala-tools"
import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopUsage,
} from "./rpc.js"

export const KhalaCodeHeadlessUsage = S.Struct({
  cached_input: S.Number,
  input: S.Number,
  output: S.Number,
  reasoning_output: S.Number,
})
export type KhalaCodeHeadlessUsage = typeof KhalaCodeHeadlessUsage.Type

export const KhalaCodeHeadlessItemKind = S.Literals([
  "message",
  "command_execution",
  "file_change",
  "mcp_tool_call",
  "todo_list",
  "error",
])
export type KhalaCodeHeadlessItemKind = typeof KhalaCodeHeadlessItemKind.Type

export const KhalaCodeHeadlessThreadEvent = S.Union([
  S.Struct({
    session_id: S.String,
    thread_id: S.String,
    type: S.Literal("thread.started"),
  }),
  S.Struct({
    thread_id: S.optional(S.String),
    turn_id: S.String,
    type: S.Literal("turn.started"),
  }),
  S.Struct({
    codex_turn_id: S.optional(S.String),
    final_message: S.optional(S.String),
    ok: S.Boolean,
    status: S.optional(S.String),
    thread_id: S.optional(S.String),
    turn_id: S.String,
    type: S.Literal("turn.completed"),
    usage: KhalaCodeHeadlessUsage,
  }),
  S.Struct({
    error: S.String,
    status: S.optional(S.String),
    thread_id: S.optional(S.String),
    turn_id: S.String,
    type: S.Literal("turn.failed"),
    usage: KhalaCodeHeadlessUsage,
  }),
  S.Struct({
    item: S.Struct({
      id: S.String,
      kind: KhalaCodeHeadlessItemKind,
      role: S.optional(S.String),
      tool_name: S.optional(S.String),
    }),
    turn_id: S.String,
    type: S.Literal("item.started"),
  }),
  S.Struct({
    delta: S.String,
    item_id: S.String,
    turn_id: S.String,
    type: S.Literal("item.delta"),
  }),
  S.Struct({
    item_id: S.String,
    payload: S.Unknown,
    turn_id: S.String,
    type: S.Literal("item.updated"),
  }),
  S.Struct({
    item_id: S.String,
    turn_id: S.String,
    type: S.Literal("item.completed"),
  }),
])
export type KhalaCodeHeadlessThreadEvent =
  typeof KhalaCodeHeadlessThreadEvent.Type

export function khalaCodeHeadlessThreadStarted(input: {
  readonly sessionId: string
  readonly threadId?: string
}): KhalaCodeHeadlessThreadEvent {
  return {
    session_id: input.sessionId,
    thread_id: input.threadId ?? input.sessionId,
    type: "thread.started",
  }
}

export function khalaCodeHeadlessTurnStarted(
  turnId: string,
  input: { readonly threadId?: string } = {},
): KhalaCodeHeadlessThreadEvent {
  return {
    ...(input.threadId === undefined ? {} : { thread_id: input.threadId }),
    turn_id: turnId,
    type: "turn.started",
  }
}

export function khalaCodeHeadlessTurnCompleted(input: {
  readonly codexTurnId?: string
  readonly finalMessage?: string
  readonly ok: boolean
  readonly status?: string
  readonly threadId?: string
  readonly turnId: string
  readonly usage?: KhalaCodeDesktopUsage
}): KhalaCodeHeadlessThreadEvent {
  return {
    ...(input.codexTurnId === undefined ? {} : { codex_turn_id: input.codexTurnId }),
    ...(input.finalMessage === undefined ? {} : { final_message: input.finalMessage }),
    ok: input.ok,
    ...(input.status === undefined ? {} : { status: input.status }),
    ...(input.threadId === undefined ? {} : { thread_id: input.threadId }),
    turn_id: input.turnId,
    type: "turn.completed",
    usage: projectUsage(input.usage),
  }
}

export function khalaCodeHeadlessTurnFailed(input: {
  readonly error: string
  readonly status?: string
  readonly threadId?: string
  readonly turnId: string
  readonly usage?: KhalaCodeDesktopUsage
}): KhalaCodeHeadlessThreadEvent {
  return {
    error: input.error,
    ...(input.status === undefined ? {} : { status: input.status }),
    ...(input.threadId === undefined ? {} : { thread_id: input.threadId }),
    turn_id: input.turnId,
    type: "turn.failed",
    usage: projectUsage(input.usage),
  }
}

export function projectKhalaCodeDesktopEventToThreadEvents(
  event: KhalaCodeDesktopChatTurnEvent,
): readonly KhalaCodeHeadlessThreadEvent[] {
  switch (event.type) {
    case "thread_ready":
      return []
    case "message_start":
      return [messageStarted(event.turnId, event.message)]
    case "message_delta":
      return [{
        delta: event.delta,
        item_id: event.messageId,
        turn_id: event.turnId,
        type: "item.delta",
      }]
    case "message_replace":
      return [
        messageStarted(event.turnId, event.message),
        {
          item_id: event.message.id,
          payload: {
            body: event.message.body,
            role: event.message.role,
          },
          turn_id: event.turnId,
          type: "item.updated",
        },
      ]
    case "message_done":
      return [{
        item_id: event.messageId,
        turn_id: event.turnId,
        type: "item.completed",
      }]
    case "tool_event":
      return projectToolEvent(event.turnId, event.event)
  }
}

export function stringifyKhalaCodeHeadlessThreadEvent(
  event: KhalaCodeHeadlessThreadEvent,
): string {
  return JSON.stringify(event)
}

function messageStarted(
  turnId: string,
  message: KhalaCodeDesktopMessage,
): KhalaCodeHeadlessThreadEvent {
  return {
    item: {
      id: message.id,
      kind: "message",
      role: message.role,
    },
    turn_id: turnId,
    type: "item.started",
  }
}

function projectToolEvent(
  turnId: string,
  event: KhalaToolEvent,
): readonly KhalaCodeHeadlessThreadEvent[] {
  const itemId = event.invocationId ?? event.eventId
  const itemKind = itemKindForToolEvent(event)
  if (event.kind === "tool_requested" || event.kind === "tool_started") {
    return [{
      item: {
        id: itemId,
        kind: itemKind,
        ...(toolNameFromPayload(event.payload) === undefined
          ? {}
          : { tool_name: toolNameFromPayload(event.payload) }),
      },
      turn_id: turnId,
      type: "item.started",
    }]
  }
  if (event.kind === "tool_completed" || event.kind === "tool_failed") {
    return [{
      item_id: itemId,
      turn_id: turnId,
      type: "item.completed",
    }]
  }
  if (event.kind === "stdout_chunk" || event.kind === "stderr_chunk" || event.kind === "diff_chunk") {
    return [{
      delta: textFromPayload(event.payload),
      item_id: itemId,
      turn_id: turnId,
      type: "item.delta",
    }]
  }
  return [{
    item_id: itemId,
    payload: event.payload,
    turn_id: turnId,
    type: "item.updated",
  }]
}

function itemKindForToolEvent(event: KhalaToolEvent): KhalaCodeHeadlessItemKind {
  const toolName = toolNameFromPayload(event.payload)
  if (event.kind === "tool_failed") return "error"
  if (toolName === "exec_command" || event.kind === "stdout_chunk" || event.kind === "stderr_chunk" || event.kind === "stdin_chunk") {
    return "command_execution"
  }
  if (
    toolName === "read" ||
    toolName === "ls" ||
    toolName === "glob" ||
    toolName === "grep" ||
    toolName === "edit" ||
    toolName === "write" ||
    toolName === "apply_patch" ||
    event.kind === "diff_chunk" ||
    event.kind === "artifact_written"
  ) {
    return "file_change"
  }
  if (toolName === "todo_write" || event.kind === "todo_list_updated") return "todo_list"
  return "mcp_tool_call"
}

function toolNameFromPayload(payload: unknown): string | undefined {
  return isRecord(payload) && typeof payload.name === "string" ? payload.name : undefined
}

function textFromPayload(payload: unknown): string {
  if (typeof payload === "string") return payload
  if (!isRecord(payload)) return ""
  if (typeof payload.text === "string") return payload.text
  if (typeof payload.chunk === "string") return payload.chunk
  if (typeof payload.output === "string") return payload.output
  return JSON.stringify(payload)
}

function projectUsage(usage: KhalaCodeDesktopUsage | undefined): KhalaCodeHeadlessUsage {
  return {
    cached_input: usage?.cachedInput ?? 0,
    input: usage?.input ?? 0,
    output: usage?.output ?? 0,
    reasoning_output: usage?.reasoningOutput ?? 0,
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
