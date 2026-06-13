type RawRecord = Record<string, unknown>

const STATUS_PREFIXES = new Set([
  "cancelled",
  "completed",
  "failed",
  "file change",
  "queued",
  "running",
  "started",
  "succeeded",
])

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function oneLine(value: unknown): string {
  return typeof value === "string" ? value.replace(/\s+/g, " ").trim() : ""
}

function plural(noun: string, count: number): string {
  return count === 1 ? noun : `${noun}s`
}

function hasToolPrefix(message: string): boolean {
  return /^(?:Bash|Glob|Grep|MultiEdit|NotebookEdit|Read|TodoWrite)(?:\b|:)/i.test(message) ||
    /^(?:Edit|Write)\s+\S/i.test(message) ||
    /^(?:apply_patch|exec_command):/i.test(message) ||
    /^web search:/i.test(message)
}

function hasFileEditPrefix(message: string): boolean {
  return /^(?:Edit|MultiEdit|NotebookEdit|Write)\s+\S/i.test(message) ||
    /^apply_patch:/i.test(message)
}

function stripStatusPrefix(message: string): string {
  const separator = message.indexOf(":")
  if (separator === -1) return message

  const prefix = message.slice(0, separator).trim().toLowerCase()
  return STATUS_PREFIXES.has(prefix) ? message.slice(separator + 1).trim() : message
}

function countPatchFiles(message: string): number {
  const matches = message.match(/\*\*\* (?:Add|Delete|Update) File:/g)
  return matches === null ? 0 : matches.length
}

function countFileChangeSegments(message: string): number {
  const body = stripStatusPrefix(message)
  const explicit = /\bedited\s+([0-9]+)\s+files?\b/i.exec(body)
  if (explicit !== null) {
    const count = Number(explicit[1])
    return Number.isSafeInteger(count) && count > 0 ? count : 0
  }

  return body
    .split(",")
    .filter(segment =>
      /^(?:add|added|change|changed|create|created|delete|deleted|edit|edited|modify|modified|remove|removed|update|updated|write|wrote)\s+\S/i
        .test(segment.trim()),
    )
    .length
}

function isComposerToolSummary(message: string): boolean {
  if (/^web search:/i.test(message)) return true

  const separator = message.indexOf(":")
  if (separator === -1) return false

  const prefix = message.slice(0, separator).trim().toLowerCase()
  return STATUS_PREFIXES.has(prefix)
}

function isToolCall(phase: string, message: string): boolean {
  const normalizedPhase = phase.toLowerCase()

  if (
    normalizedPhase === "command_execution" ||
    normalizedPhase === "file_change" ||
    normalizedPhase === "mcp_tool_call" ||
    normalizedPhase === "web_search"
  ) return true

  if (normalizedPhase.includes("tool") && !normalizedPhase.includes("result")) return true
  if (normalizedPhase === "composer_event" && isComposerToolSummary(message)) return true

  return hasToolPrefix(message)
}

function fileEditCount(phase: string, message: string): number {
  const normalizedPhase = phase.toLowerCase()

  if (/^apply_patch:/i.test(message)) {
    const patchFiles = countPatchFiles(message)
    return patchFiles > 0 ? patchFiles : 1
  }

  if (hasFileEditPrefix(message)) return 1

  if (normalizedPhase === "file_change" || normalizedPhase === "composer_event") {
    return countFileChangeSegments(message)
  }

  return 0
}

function headlineFor(toolCalls: number, fileEdits: number): string {
  const parts: string[] = []

  if (toolCalls > 0) parts.push(`ran ${toolCalls} ${plural("command", toolCalls)}`)
  if (fileEdits > 0) parts.push(`edited ${fileEdits} ${plural("file", fileEdits)}`)

  return parts.length > 0 ? parts.join(", ") : "no activity yet"
}

export function summarizeAgentActivity(
  events: { phase: string; messageText: string }[],
): { toolCalls: number; fileEdits: number; lastAction: string; headline: string } {
  const rows = Array.isArray(events) ? events : []
  let toolCalls = 0
  let fileEdits = 0
  let lastAction = ""
  let lastPhase = ""

  for (const row of rows) {
    if (!isRecord(row)) continue

    const phase = oneLine(row.phase)
    const message = oneLine(row.messageText)

    if (phase !== "") lastPhase = phase
    if (message !== "") lastAction = message
    if (isToolCall(phase, message)) toolCalls += 1

    fileEdits += fileEditCount(phase, message)
  }

  return {
    toolCalls,
    fileEdits,
    lastAction: lastAction || lastPhase,
    headline: headlineFor(toolCalls, fileEdits),
  }
}
