import type * as AiResponse from "@effect/ai/Response"

import type { ChatPart } from "./chatProtocol"

export type ActiveStream = {
  readonly id: string
  readonly messageId: string
  parts: Array<ChatPart>
}

function stableStringify(value: unknown): string {
  if (value == null) return String(value)
  if (typeof value === "string") return value
  try {
    return JSON.stringify(value)
  } catch {
    return String(value)
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value)
}

function upsertDsePart(active: ActiveStream, part: Record<string, unknown>): ActiveStream {
  const type = typeof part.type === "string" ? part.type : null
  const id = typeof part.id === "string" ? part.id : null
  if (!type || !type.startsWith("dse.") || !id) return active

  const index = active.parts.findIndex(
    (p) => p && typeof p === "object" && (p as any).type === type && (p as any).id === id,
  )

  if (index >= 0) {
    const prev = active.parts[index]
    active.parts[index] = { ...(prev as any), ...(part as any) } as any
    return active
  }

  active.parts.push(part as any)
  return active
}

/**
 * Applies a single `messageParts.part` payload onto the in-progress assistant message.
 *
 * This is a pure decoder/accumulator that supports:
 * - `@effect/ai` stream parts (text/tool/finish)
 * - DSE action parts (`type: "dse.*"`) which are upserted by stable `id`
 */
export function applyChatWirePart(active: ActiveStream, chunkData: unknown): ActiveStream {
  if (!isRecord(chunkData)) return active

  const rawType = chunkData.type
  const type = typeof rawType === "string" ? rawType : ""

  if (type.startsWith("dse.")) {
    return upsertDsePart(active, chunkData)
  }

  switch (type as AiResponse.StreamPartEncoded["type"]) {
    case "text-start": {
      active.parts.push({ type: "text", text: "", state: "streaming" })
      return active
    }
    case "text-delta": {
      const lastTextPart = [...active.parts].reverse().find((p) => p?.type === "text") as any
      if (lastTextPart && lastTextPart.type === "text") {
        lastTextPart.text += String((chunkData as any).delta ?? "")
      } else {
        active.parts.push({ type: "text", text: String((chunkData as any).delta ?? "") })
      }
      return active
    }
    case "text-end": {
      const lastTextPart = [...active.parts].reverse().find((p) => p?.type === "text") as any
      if (lastTextPart && "state" in lastTextPart) lastTextPart.state = "done"
      return active
    }
    case "reasoning-start":
    case "reasoning-delta":
    case "reasoning-end": {
      // Autopilot must not render reasoning. Ignore these parts on the UI wire.
      return active
    }
    case "tool-call": {
      const toolName = String((chunkData as any).name ?? "tool")
      active.parts.push({
        type: `tool-${toolName}`,
        toolCallId: String((chunkData as any).id ?? ""),
        toolName,
        state: "input-available",
        input: (chunkData as any).params,
      } as any)
      return active
    }
    case "tool-result": {
      const toolCallId = String((chunkData as any).id ?? "")
      const isFailure = Boolean((chunkData as any).isFailure)
      const result = (chunkData as any).result

      let didUpdate = false
      active.parts = active.parts.map((p) => {
        if (
          p &&
          typeof p === "object" &&
          "toolCallId" in p &&
          String((p as any).toolCallId) === toolCallId &&
          "state" in p
        ) {
          didUpdate = true
          return {
            ...(p as any),
            state: isFailure ? "output-error" : "output-available",
            output: result,
            ...(isFailure ? { errorText: stableStringify(result) } : {}),
          }
        }
        return p
      })

      if (!didUpdate) {
        const toolName = String((chunkData as any).name ?? "tool")
        active.parts.push({
          type: `tool-${toolName}`,
          toolCallId,
          toolName,
          state: isFailure ? "output-error" : "output-available",
          output: result,
          ...(isFailure ? { errorText: stableStringify(result) } : {}),
        } as any)
      }

      return active
    }
    case "error": {
      const raw = (chunkData as any).error
      const message = typeof raw === "string" ? raw : raw == null ? "Unknown error" : stableStringify(raw)

      // Render the error as visible assistant text so users are never left with a silent stall.
      active.parts.push({
        type: "text",
        text: message ? `Error: ${message}` : "Error",
        state: "done",
      })
      return active
    }
    case "finish": {
      // No-op for now; message finalization is reflected via message.status/text.
      return active
    }
    default:
      return active
  }
}

