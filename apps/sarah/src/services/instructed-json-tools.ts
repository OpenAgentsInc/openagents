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

export const SARAH_OPERATOR_INSTRUCTED_JSON_TOOLS = [
  ...SARAH_INSTRUCTED_JSON_TOOLS,
  "coding_fleet_start",
] as const

export type SarahInstructedJsonToolName =
  (typeof SARAH_OPERATOR_INSTRUCTED_JSON_TOOLS)[number]

export type SarahInstructedJsonToolPolicy = Readonly<{
  codingFleetStartAllowed: boolean
  relationshipMode: "prospect" | "customer" | "operator" | "administrator"
}>

export const SARAH_PUBLIC_INSTRUCTED_JSON_POLICY: SarahInstructedJsonToolPolicy = {
  codingFleetStartAllowed: false,
  relationshipMode: "prospect",
}

export type ParsedInstructedToolCall = {
  toolName: SarahInstructedJsonToolName
  args: Record<string, unknown>
  raw: string
}

const toolNamesForPolicy = (
  policy: SarahInstructedJsonToolPolicy,
): ReadonlyArray<SarahInstructedJsonToolName> =>
  policy.codingFleetStartAllowed &&
  (policy.relationshipMode === "operator" ||
    policy.relationshipMode === "administrator")
    ? SARAH_OPERATOR_INSTRUCTED_JSON_TOOLS
    : SARAH_INSTRUCTED_JSON_TOOLS

/**
 * System-prompt appendix that teaches the model the instructed-JSON protocol.
 * Appended only when the owner enables SARAH_INSTRUCTED_JSON_TOOLS=1.
 */
export function instructedJsonToolProtocolPrompt(
  policy: SarahInstructedJsonToolPolicy = SARAH_PUBLIC_INSTRUCTED_JSON_POLICY,
): string {
  const names = toolNamesForPolicy(policy).join(", ")
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
  policy: SarahInstructedJsonToolPolicy = SARAH_PUBLIC_INSTRUCTED_JSON_POLICY,
): ParsedInstructedToolCall | null {
  const trimmed = text.trim()
  if (!trimmed) return null
  const toolSet = new Set<string>(toolNamesForPolicy(policy))

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
      if (!name || !toolSet.has(name)) continue
      // The coding tool creates durable work. Unlike the public lookup tools,
      // it is accepted only as the complete model reply, never extracted from
      // prose or a markdown fence that could have been quoted/injected.
      if (name === "coding_fleet_start" && candidate !== trimmed) continue
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

/** Detect a denied high-authority attempt without retaining or echoing args. */
export function isDeniedCodingFleetToolAttempt(
  text: string,
  policy: SarahInstructedJsonToolPolicy,
): boolean {
  if (toolNamesForPolicy(policy).includes("coding_fleet_start")) return false

  // The model reply is already bounded by the inference lane. Bound the
  // additional structural work too: parse only complete JSON objects, cap
  // object size/count/depth, and fail closed if a structured reply exhausts
  // that budget. Plain prose that merely names the tool never enters JSON
  // parsing and is not mistaken for an attempted call.
  const maxCandidateChars = 16 * 1024
  const maxCandidates = 64
  const maxDepth = 32
  const starts: number[] = []
  let inString = false
  let escaped = false
  let candidates = 0

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index]!
    if (starts.length > 0 && inString) {
      if (escaped) {
        escaped = false
      } else if (character === "\\") {
        escaped = true
      } else if (character === '"') {
        inString = false
      }
      continue
    }
    if (starts.length > 0 && character === '"') {
      inString = true
      continue
    }
    if (character === "{") {
      if (starts.length >= maxDepth) return true
      starts.push(index)
      continue
    }
    if (character !== "}" || starts.length === 0) continue

    const start = starts.pop()!
    candidates += 1
    if (candidates > maxCandidates) return true
    if (index - start + 1 > maxCandidateChars) continue
    try {
      const parsed = JSON.parse(text.slice(start, index + 1)) as Record<
        string,
        unknown
      >
      if (
        [parsed.sarah_tool, parsed.tool, parsed.toolName].includes(
          "coding_fleet_start",
        )
      ) {
        return true
      }
    } catch {
      // A brace-delimited prose fragment is not a syntactic tool attempt.
    }
  }
  return false
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

/**
 * Coding command selection is always armed only after server-derived
 * operator/administrator policy. The legacy public lookup loop keeps its
 * explicit rollout flag, so enabling the P0 owner path does not broaden the
 * prospect tool surface.
 */
export function instructedJsonToolsArmed(
  policy: SarahInstructedJsonToolPolicy = SARAH_PUBLIC_INSTRUCTED_JSON_POLICY,
): boolean {
  return (
    toolNamesForPolicy(policy).includes("coding_fleet_start") ||
    process.env.SARAH_INSTRUCTED_JSON_TOOLS === "1"
  )
}
