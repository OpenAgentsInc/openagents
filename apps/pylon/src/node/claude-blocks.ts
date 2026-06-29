import type { ExternalEvent } from "./external-sessions.js"

function clip(value: string, max = 200): string {
  const oneLine = value.replace(/\s+/g, " ").trim()
  return oneLine.length > max ? `${oneLine.slice(0, max)}…` : oneLine
}

function summarizeToolUse(name: string, input: unknown): string {
  const rec = (input ?? {}) as Record<string, unknown>
  const str = (k: string): string => (typeof rec[k] === "string" ? (rec[k] as string) : "")
  switch (name) {
    case "Bash":
      return `Bash: ${clip(str("command"), 160)}`
    case "Read":
      return `Read ${clip(str("file_path"), 120)}`
    case "Edit":
    case "Write":
    case "NotebookEdit":
      return `${name} ${clip(str("file_path"), 120)}`
    case "Grep":
      return `Grep ${clip(str("pattern"), 80)}`
    case "Glob":
      return `Glob ${clip(str("pattern"), 80)}`
    case "Task":
    case "Agent":
      return `→ sub-agent: ${clip(str("description") || str("prompt"), 120)}`
    case "TodoWrite":
      return "TodoWrite (plan update)"
    default: {
      const preview = clip(JSON.stringify(rec), 100)
      return `${name}: ${preview}`
    }
  }
}

function fullJson(value: unknown): string {
  return JSON.stringify(value ?? {}, null, 2)
}

function toolResultText(content: unknown): string {
  if (typeof content === "string") return content
  if (Array.isArray(content)) {
    return content
      .map((block) => {
        if (block !== null && typeof block === "object" && (block as { type?: unknown }).type === "text") {
          const text = (block as { text?: unknown }).text
          return typeof text === "string" ? text : ""
        }
        return ""
      })
      .filter((text) => text.trim().length > 0)
      .join("\n")
  }
  return content === undefined || content === null ? "" : String(content)
}

function blockFull(value: unknown): string {
  if (typeof value === "string") return value
  return JSON.stringify(value ?? null, null, 2)
}

export function expandClaudeMessage(raw: unknown): ExternalEvent[] {
  if (raw === null || typeof raw !== "object") return []

  const o = raw as Record<string, unknown>
  const type = typeof o.type === "string" ? o.type : ""
  if (type !== "user" && type !== "assistant") return []

  const message = o.message !== null && typeof o.message === "object" ? (o.message as Record<string, unknown>) : {}
  const observedAt = typeof o.timestamp === "string" ? o.timestamp : ""
  const role = type === "user" ? "you" : "agent"
  const phase = type === "user" ? "user" : "agent_message"
  const content = message.content

  if (typeof content === "string") {
    if (content.trim().length === 0) return []
    return [{ observedAt, phase, messageText: `${role}: ${clip(content)}`, messageFull: content }]
  }

  if (!Array.isArray(content)) return []

  const events: ExternalEvent[] = []
  for (const block of content) {
    if (block === null || typeof block !== "object") continue
    const b = block as Record<string, unknown>
    const blockType = typeof b.type === "string" ? b.type : ""

    if (blockType === "text") {
      const text = b.text
      if (typeof text !== "string" || text.trim().length === 0) continue
      events.push({ observedAt, phase, messageText: `${role}: ${clip(text)}`, messageFull: text })
      continue
    }

    if (blockType === "tool_use") {
      const name = b.name
      if (typeof name !== "string" || name.trim().length === 0) continue
      events.push({
        observedAt,
        phase: "tool_use",
        messageText: summarizeToolUse(name, b.input),
        messageFull: fullJson(b.input),
      })
      continue
    }

    if (blockType === "tool_result") {
      const text = toolResultText(b.content)
      if (text.trim().length === 0) continue
      events.push({
        observedAt,
        phase: "tool_result",
        messageText: `result: ${clip(text, 160)}`,
        messageFull: blockFull(b.content),
      })
      continue
    }

    if (blockType === "thinking") {
      const thinking = b.thinking
      if (typeof thinking !== "string" || thinking.trim().length === 0) continue
      events.push({
        observedAt,
        phase: "reasoning",
        messageText: "thinking…",
        messageFull: thinking,
      })
    }
  }

  return events
}
