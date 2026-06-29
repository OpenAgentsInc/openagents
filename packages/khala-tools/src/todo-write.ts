import { Effect } from "effect"
import {
  khalaToolError,
  khalaToolOk,
  type KhalaTodoEvent,
  type KhalaTodoItemInput,
  type KhalaTodoStatus,
  type KhalaTodoWriteResult,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export const todoWriteToolDefinition: KhalaToolDefinition = {
  authority: "session_state",
  availability: ["coding", "owner_local_full"],
  description: "Replace the current session-local todo list with an ordered, non-authoritative plan projection.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      todos: {
        description: "Ordered session-local todo items.",
        items: {
          additionalProperties: false,
          properties: {
            blocker_reason: {
              description: "Required when status is blocked.",
              type: "string",
            },
            content: {
              description: "Short user-visible todo text.",
              type: "string",
            },
            id: {
              description: "Stable model-chosen item id for later updates/reordering.",
              type: "string",
            },
            status: {
              enum: ["pending", "in_progress", "blocked", "completed", "cancelled"],
              type: "string",
            },
          },
          required: ["id", "content", "status"],
          type: "object",
        },
        type: "array",
      },
    },
    required: ["todos"],
    type: "object",
  },
  internalId: "khala.session.todo_write",
  label: "Todo Write",
  name: "todo_write",
  outputSchema: {
    additionalProperties: false,
    properties: {
      nonAuthoritative: { const: true, type: "boolean" },
      revision: { type: "integer" },
      todoCount: { type: "integer" },
    },
    required: ["revision", "todoCount", "nonAuthoritative"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Update the session-local todo list without claiming execution authority.",
  promptGuidelines: [
    "Use stable ids so later calls can update or reorder items.",
    "Keep at most one item in_progress.",
    "Treat completed todos as planning/progress display only; they are not evidence, receipts, payouts, or accepted work.",
  ],
  renderer: { kind: "todo_list", rendererRef: "khala.renderer.todo_list.v1" },
}

export function createTodoWriteTool(): RegisteredKhalaTool {
  return {
    definition: todoWriteToolDefinition,
    execute: executeTodoWriteTool,
  }
}

function executeTodoWriteTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const todos = decodeTodoWriteInput(input)
      const result = await Effect.runPromise(
        context.services.todo.writeTodos({
          invocationId: context.invocation.id,
          khalaSessionId: context.invocation.sessionId,
          todos,
        }),
      )
      return renderTodoWriteResult(result)
    } catch (error) {
      return khalaToolError("todo_write_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

function decodeTodoWriteInput(input: Readonly<Record<string, unknown>>): ReadonlyArray<KhalaTodoItemInput> {
  if (!Array.isArray(input.todos)) throw new Error("todo_write requires todos array")
  if (input.todos.length > 50) throw new Error("todo_write accepts at most 50 todos")
  const seen = new Set<string>()
  let inProgressCount = 0
  return input.todos.map((value, index) => {
    if (!isRecord(value)) throw new Error(`todo_write todo ${index + 1} must be an object`)
    const id = stringField(value.id, `todo ${index + 1} id`, 80)
    const content = stringField(value.content, `todo ${index + 1} content`, 500)
    if (seen.has(id)) throw new Error(`todo_write duplicate todo id: ${id}`)
    seen.add(id)
    const status = decodeStatus(value.status, index)
    if (status === "in_progress") inProgressCount += 1
    if (inProgressCount > 1) throw new Error("todo_write allows at most one in_progress todo")
    const blockerReason = typeof value.blocker_reason === "string" ? value.blocker_reason.trim() : undefined
    if (status === "blocked" && (blockerReason === undefined || blockerReason.length === 0)) {
      throw new Error(`todo_write blocked todo ${id} requires blocker_reason`)
    }
    if (blockerReason !== undefined && blockerReason.length > 500) {
      throw new Error(`todo_write blocker_reason for ${id} must be 500 characters or fewer`)
    }
    return {
      ...(blockerReason === undefined ? {} : { blockerReason }),
      content,
      id,
      status,
    }
  })
}

function renderTodoWriteResult(result: KhalaTodoWriteResult): KhalaToolResult {
  const completedCount = result.todos.filter(todo => todo.status === "completed").length
  return khalaToolOk({
    modelText: [
      `Session todo list updated to revision ${result.revision}.`,
      "This is session-local planning state, not proof of implementation or acceptance.",
      "",
      ...result.todos.map(formatTodoForModel),
    ].join("\n").trimEnd(),
    publicSafety: "private",
    publicSummary:
      `Session todo list updated with ${result.todos.length} item${result.todos.length === 1 ? "" : "s"}` +
      ` (${completedCount} marked completed). Non-authoritative planning state only.`,
    ui: {
      events: result.events.map(eventToUi),
      kind: "todo_list",
      nonAuthoritative: true,
      revision: result.revision,
      sessionId: result.sessionId,
      todos: result.todos,
    },
  })
}

function formatTodoForModel(todo: KhalaTodoWriteResult["todos"][number]): string {
  const marker = todo.status === "completed"
    ? "[x]"
    : todo.status === "in_progress"
      ? "[>]"
      : todo.status === "blocked"
        ? "[!]"
        : todo.status === "cancelled"
          ? "[-]"
          : "[ ]"
  const blocker = todo.status === "blocked" && todo.blockerReason !== undefined ? ` — blocked: ${todo.blockerReason}` : ""
  return `${marker} ${todo.content} (${todo.id}, ${todo.status})${blocker}`
}

function eventToUi(event: KhalaTodoEvent): unknown {
  return {
    kind: event.kind,
    payload: event.payload,
    timestampMs: event.timestampMs,
  }
}

function stringField(value: unknown, field: string, maxLength: number): string {
  const text = typeof value === "string" ? value.trim() : ""
  if (text.length === 0) throw new Error(`todo_write ${field} is required`)
  if (text.length > maxLength) throw new Error(`todo_write ${field} must be ${maxLength} characters or fewer`)
  return text
}

function decodeStatus(value: unknown, index: number): KhalaTodoStatus {
  if (
    value === "pending" ||
    value === "in_progress" ||
    value === "blocked" ||
    value === "completed" ||
    value === "cancelled"
  ) {
    return value
  }
  throw new Error(`todo_write todo ${index + 1} status is invalid`)
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
