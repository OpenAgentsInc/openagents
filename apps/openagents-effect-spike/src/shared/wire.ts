// Firm reason: 1 (Effect UI / Effect v4 browser and web-server prototype for docs/effect/half-day-sprint.md)
import { Schema } from "effect"

export const WorkerStatus = Schema.Literals([
  "starting",
  "ready",
  "running",
  "waiting_approval",
  "exited",
  "error"
])
export type WorkerStatus = typeof WorkerStatus.Type

export const ToolCallEntry = Schema.Struct({
  id: Schema.String,
  kind: Schema.Literals(["command", "view"]),
  command: Schema.String,
  path: Schema.String,
  found: Schema.Boolean,
  exit: Schema.NullOr(Schema.Number),
  timedOut: Schema.Boolean,
  output: Schema.String
})
export type ToolCallEntry = typeof ToolCallEntry.Type

export const PendingPermit = Schema.Struct({
  ticket: Schema.Number,
  kind: Schema.Literals(["ask", "permit"]),
  question: Schema.Boolean,
  title: Schema.String,
  command: Schema.String,
  options: Schema.Array(Schema.String)
})
export type PendingPermit = typeof PendingPermit.Type

export const TodoItem = Schema.Struct({
  id: Schema.String,
  content: Schema.String,
  status: Schema.String
})
export type TodoItem = typeof TodoItem.Type

export const InferenceStats = Schema.Struct({
  ttftMs: Schema.Number,
  decodeMs: Schema.Number,
  outputTokens: Schema.Number
})
export type InferenceStats = typeof InferenceStats.Type

export const ChatMessage = Schema.Struct({
  id: Schema.String,
  role: Schema.Literals(["user", "assistant"]),
  turn: Schema.Number,
  thinking: Schema.String,
  text: Schema.String,
  tools: Schema.Array(ToolCallEntry),
  ending: Schema.NullOr(Schema.String),
  streaming: Schema.Boolean
})
export type ChatMessage = typeof ChatMessage.Type

export const SessionSnapshot = Schema.Struct({
  sessionId: Schema.String,
  seq: Schema.Number,
  status: WorkerStatus,
  activeTurn: Schema.Number,
  title: Schema.String,
  cwd: Schema.String,
  model: Schema.String,
  messages: Schema.Array(ChatMessage),
  pendingPermit: Schema.NullOr(PendingPermit),
  todos: Schema.Array(TodoItem),
  stats: Schema.NullOr(InferenceStats),
  lastError: Schema.NullOr(Schema.String),
  workerPid: Schema.NullOr(Schema.Number)
})
export type SessionSnapshot = typeof SessionSnapshot.Type

// ---- Owner -> Worker JSON-line encoders (coax/lib/agent/engine/mcworker-wire.coax) ----

export interface TaskWireOptions {
  readonly task: string
  readonly session: string
  readonly cwd: string
  readonly gated?: boolean | undefined
  readonly local?: boolean | undefined
  readonly model?: string | undefined
  readonly effort?: string | undefined
  readonly mode?: string | undefined
}

export const encodeTaskLine = (opts: TaskWireOptions): string => {
  const payload: Record<string, unknown> = {
    task: opts.task,
    session: opts.session,
    cwd: opts.cwd
  }
  if (opts.gated) payload["gated"] = true
  if (opts.local) payload["local"] = true
  if (opts.model && opts.model.length > 0) payload["model"] = opts.model
  if (opts.effort && opts.effort.length > 0) payload["effort"] = opts.effort
  if (opts.mode && opts.mode.length > 0) payload["mode"] = opts.mode
  return `${JSON.stringify(payload)}\n`
}

export const encodeAnswerLine = (opts: {
  readonly answer: boolean
  readonly ticket: number
  readonly why?: string | undefined
  readonly option?: string | undefined
}): string => {
  const payload: Record<string, unknown> = {
    answer: opts.answer,
    ticket: opts.ticket
  }
  if (opts.why && opts.why.length > 0) payload["why"] = opts.why
  if (opts.option && opts.option.length > 0) payload["option"] = opts.option
  return `${JSON.stringify(payload)}\n`
}

export const encodeSteerLine = (opts: {
  readonly id: string
  readonly steer: string
  readonly turn?: number | undefined
}): string => {
  const payload: Record<string, unknown> = {
    steer: opts.steer,
    id: opts.id
  }
  if (opts.turn !== undefined && opts.turn > 0) payload["turn"] = opts.turn
  return `${JSON.stringify(payload)}\n`
}

export const encodeStopLine = (): string => "{\"stop\":true}\n"

export const encodeInterruptLine = (resubmit = false): string =>
  resubmit ? "{\"interrupt\":true,\"resubmit\":true}\n" : "{\"interrupt\":true}\n"

// ---- Worker -> Owner JSON-line decoder (forward-compatible: unknown kinds return null) ----

export type WorkerEvent =
  | { readonly kind: "ready" }
  | { readonly kind: "journal"; readonly path: string }
  | { readonly kind: "turn"; readonly turn: number; readonly source: string; readonly id: string; readonly text: string }
  | { readonly kind: "thought"; readonly text: string }
  | { readonly kind: "reply"; readonly text: string }
  | {
    readonly kind: "command"
    readonly command: string
    readonly exit: number
    readonly timedOut: boolean
    readonly output: string
  }
  | { readonly kind: "view"; readonly path: string; readonly found: boolean }
  | { readonly kind: "ask"; readonly command: string; readonly ticket: number }
  | {
    readonly kind: "permit"
    readonly ticket: number
    readonly question: boolean
    readonly title: string
    readonly command: string
    readonly options: ReadonlyArray<string>
  }
  | { readonly kind: "todos"; readonly todos: ReadonlyArray<TodoItem> }
  | { readonly kind: "stats"; readonly ttftMs: number; readonly decodeMs: number; readonly outputTokens: number }
  | { readonly kind: "meta"; readonly title: string }
  | {
    readonly kind: "steer"
    readonly id: string
    readonly status: string
    readonly turn: number
    readonly reason: string
  }
  | { readonly kind: "ended"; readonly ending: string; readonly text: string }

const asString = (v: unknown, fallback = ""): string => typeof v === "string" ? v : fallback
const asNumber = (v: unknown, fallback = 0): number => typeof v === "number" && Number.isFinite(v) ? v : fallback
const asBool = (v: unknown, fallback = false): boolean => typeof v === "boolean" ? v : fallback

const parseOptions = (v: unknown): ReadonlyArray<string> => {
  let list: unknown = v
  if (typeof v === "string" && v.trim().startsWith("[")) {
    try {
      list = JSON.parse(v)
    } catch {
      return v.split("\n").map((s) => s.trim()).filter((s) => s.length > 0)
    }
  }
  if (Array.isArray(list)) {
    const out: Array<string> = []
    for (const item of list) {
      if (typeof item === "string" && item.length > 0) {
        out.push(item)
      } else if (typeof item === "object" && item !== null) {
        const rec = item as Record<string, unknown>
        const id = asString(rec["optionId"]) || asString(rec["name"])
        if (id.length > 0) out.push(id)
      }
    }
    return out
  }
  if (typeof v === "string" && v.length > 0) {
    return v.split("\n").map((s) => s.trim()).filter((s) => s.length > 0)
  }
  return []
}

export const parseWorkerLine = (raw: string): WorkerEvent | null => {
  const trimmed = raw.trim()
  if (trimmed.length === 0 || trimmed[0] !== "{") return null
  let obj: Record<string, unknown>
  try {
    const parsed: unknown = JSON.parse(trimmed)
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return null
    obj = parsed as Record<string, unknown>
  } catch {
    return null
  }
  const kind = obj["kind"]
  if (typeof kind !== "string") return null

  switch (kind) {
    case "ready":
      return { kind: "ready" }
    case "journal":
      return { kind: "journal", path: asString(obj["path"]) }
    case "turn":
      return {
        kind: "turn",
        turn: asNumber(obj["turn"]),
        source: asString(obj["source"]),
        id: asString(obj["id"]),
        text: asString(obj["text"])
      }
    case "thought":
      return { kind: "thought", text: asString(obj["text"]) }
    case "reply":
      return { kind: "reply", text: asString(obj["text"]) }
    case "command":
      return {
        kind: "command",
        command: asString(obj["command"]),
        exit: asNumber(obj["exit"]),
        timedOut: asBool(obj["timed_out"]),
        output: asString(obj["output"])
      }
    case "view":
      return {
        kind: "view",
        path: asString(obj["path"]),
        found: asBool(obj["found"])
      }
    case "ask":
      return {
        kind: "ask",
        command: asString(obj["command"]),
        ticket: asNumber(obj["ticket"])
      }
    case "permit":
      return {
        kind: "permit",
        ticket: asNumber(obj["ticket"]),
        question: asBool(obj["question"]),
        title: asString(obj["title"]),
        command: asString(obj["command"]),
        options: parseOptions(obj["options"])
      }
    case "todos": {
      const rawTodos = Array.isArray(obj["todos"]) ? obj["todos"] : []
      const todos: Array<TodoItem> = []
      for (const item of rawTodos) {
        if (typeof item === "object" && item !== null) {
          const rec = item as Record<string, unknown>
          todos.push({
            id: asString(rec["id"]),
            content: asString(rec["content"]),
            status: asString(rec["status"])
          })
        }
      }
      return { kind: "todos", todos }
    }
    case "stats":
      return {
        kind: "stats",
        ttftMs: asNumber(obj["ttft_ms"]),
        decodeMs: asNumber(obj["decode_ms"]),
        outputTokens: asNumber(obj["output_tokens"])
      }
    case "meta":
      return { kind: "meta", title: asString(obj["title"]) }
    case "steer":
      return {
        kind: "steer",
        id: asString(obj["id"]),
        status: asString(obj["status"]),
        turn: asNumber(obj["turn"]),
        reason: asString(obj["reason"])
      }
    case "ended":
      return {
        kind: "ended",
        ending: asString(obj["ending"], "finished"),
        text: asString(obj["text"])
      }
    default:
      return null
  }
}
