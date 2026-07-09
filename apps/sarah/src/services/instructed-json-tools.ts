/**
 * AV-3 residual (#8598): instructed-JSON tool calling for the Gemma lane.
 *
 * Gemma has no native function-calling on our path, so the model may request
 * a tool by emitting a single JSON object. Pricing / checkout tools stay OFF
 * this path — those remain deterministic guards + explicit toolCall inputs.
 */

/** Tools the model may request via instructed JSON (public-safe, no money). */
export const SARAH_INSTRUCTED_JSON_TOOLS = [
  "promise_lookup",
  "live_stats",
  "plan_catalog",
  "demo_sales_context",
  "human_handoff",
] as const

export type SarahInstructedJsonToolName =
  (typeof SARAH_INSTRUCTED_JSON_TOOLS)[number]

export type ParsedInstructedToolCall = {
  toolName: SarahInstructedJsonToolName
  args: Record<string, unknown>
  raw: string
}

const TOOL_SET = new Set<string>(SARAH_INSTRUCTED_JSON_TOOLS)

/**
 * System-prompt appendix that teaches the model the instructed-JSON protocol.
 * Appended only when the owner enables SARAH_INSTRUCTED_JSON_TOOLS=1.
 */
export function instructedJsonToolProtocolPrompt(): string {
  const names = SARAH_INSTRUCTED_JSON_TOOLS.join(", ")
  return [
    "Tool protocol (instructed JSON — native function calling unavailable):",
    `When you need a tool, reply with ONLY one JSON object and nothing else:`,
    `{"sarah_tool":"<name>","args":{...}}`,
    `Allowed tool names: ${names}.`,
    "Never invent prices, discounts, or checkout amounts via tools.",
    "If you do not need a tool, reply in normal prose (no JSON wrapper).",
  ].join("\n")
}

/**
 * Parse a model reply for an instructed tool call.
 * Accepts bare JSON or a fenced ```json block. Rejects money-moving tools.
 */
export function parseInstructedJsonToolCall(
  text: string,
): ParsedInstructedToolCall | null {
  const trimmed = text.trim()
  if (!trimmed) return null

  const candidates: string[] = []
  // Full-body JSON
  if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
    candidates.push(trimmed)
  }
  // Fenced block
  const fence = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/i)
  if (fence?.[1]) candidates.push(fence[1].trim())
  // First {...} slice
  const first = trimmed.indexOf("{")
  const last = trimmed.lastIndexOf("}")
  if (first >= 0 && last > first) {
    candidates.push(trimmed.slice(first, last + 1))
  }

  for (const candidate of candidates) {
    try {
      const parsed = JSON.parse(candidate) as Record<string, unknown>
      const name =
        typeof parsed.sarah_tool === "string"
          ? parsed.sarah_tool
          : typeof parsed.tool === "string"
            ? parsed.tool
            : typeof parsed.toolName === "string"
              ? parsed.toolName
              : null
      if (!name || !TOOL_SET.has(name)) continue
      const argsRaw = parsed.args
      const args =
        argsRaw && typeof argsRaw === "object" && !Array.isArray(argsRaw)
          ? (argsRaw as Record<string, unknown>)
          : {}
      return {
        toolName: name as SarahInstructedJsonToolName,
        args,
        raw: candidate,
      }
    } catch {
      // try next candidate
    }
  }
  return null
}

/** Format a tool result into a short prospect-facing reply (no secrets). */
export function formatInstructedToolReply(
  toolName: string,
  ok: boolean,
  output: unknown,
): string {
  if (!ok) {
    const err =
      output &&
      typeof output === "object" &&
      "error" in output &&
      typeof (output as { error: unknown }).error === "string"
        ? (output as { error: string }).error
        : "tool_failed"
    return `I couldn't complete that lookup (${toolName}: ${err}). Want me to try a different angle?`
  }
  // Keep replies short for avatar voice length.
  const summary =
    typeof output === "string"
      ? output.slice(0, 400)
      : JSON.stringify(output).slice(0, 400)
  return `Here's what I found via ${toolName}: ${summary}`
}

export function instructedJsonToolsArmed(): boolean {
  return process.env.SARAH_INSTRUCTED_JSON_TOOLS === "1"
}
