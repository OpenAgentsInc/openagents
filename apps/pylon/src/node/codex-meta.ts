export type CodexSessionMeta = {
  title: string | null
  model: string | null
  cwd: string | null
  startedAt: string | null
}

const emptyMeta = (): CodexSessionMeta => ({
  title: null,
  model: null,
  cwd: null,
  startedAt: null,
})

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" ? (value as Record<string, unknown>) : null
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null
}

function firstString(record: Record<string, unknown>, keys: string[]): string | null {
  for (const key of keys) {
    const value = nonEmptyString(record[key])
    if (value !== null) return value
  }
  return null
}

function oneLine(value: string, max = 120): string {
  const text = value.replace(/\s+/g, " ").trim()
  return text.length > max ? `${text.slice(0, max)}...` : text
}

function parseLine(line: string): Record<string, unknown> | null {
  try {
    return asRecord(JSON.parse(line))
  } catch {
    return null
  }
}

function payloadOf(record: Record<string, unknown>): Record<string, unknown> {
  return asRecord(record.payload) ?? record
}

function mergeSessionMeta(out: CodexSessionMeta, record: Record<string, unknown>): void {
  const payload = payloadOf(record)
  out.title ??= firstString(payload, ["title", "name", "summary"])
  out.model ??= firstString(payload, ["model", "modelName", "model_name"])
  out.cwd ??= firstString(payload, ["cwd", "currentWorkingDirectory", "current_working_directory"])
  out.startedAt ??= firstString(payload, ["startedAt", "started_at", "createdAt", "created_at", "timestamp"])
  out.startedAt ??= firstString(record, ["timestamp", "startedAt", "started_at"])
}

function titleFromEventMsg(record: Record<string, unknown>): string | null {
  const payload = payloadOf(record)
  const message = firstString(payload, ["message", "text", "content"])
  if (message !== null) return oneLine(message)

  const input = asRecord(payload.input)
  if (input !== null) {
    const inputMessage = firstString(input, ["message", "text", "content"])
    if (inputMessage !== null) return oneLine(inputMessage)
  }

  return null
}

export function parseCodexSessionMeta(lines: string[]): CodexSessionMeta {
  const out = emptyMeta()
  let firstEventTitle: string | null = null

  for (const line of lines) {
    const record = parseLine(line)
    if (record === null) continue

    const type = nonEmptyString(record.type)
    if (type === "session_meta") {
      mergeSessionMeta(out, record)
    } else if (type === "event_msg" && firstEventTitle === null) {
      firstEventTitle = titleFromEventMsg(record)
      out.startedAt ??= firstString(record, ["timestamp"])
    }
  }

  out.title ??= firstEventTitle
  return out
}
