import { Effect } from "effect"

// Claude Code JSONL entry types
export interface UserMessage {
  readonly type: "user"
  readonly uuid: string
  readonly timestamp: string
  readonly message: {
    readonly text: string
    readonly images?: ReadonlyArray<string>
  }
}

export interface AssistantMessage {
  readonly type: "assistant"
  readonly uuid: string
  readonly timestamp: string
  readonly message: {
    readonly role: "assistant"
    readonly content: string
    readonly thinking?: string
    readonly model?: string
    readonly usage?: {
      readonly input_tokens: number
      readonly output_tokens: number
      readonly total_tokens: number
    }
  }
}

export interface ToolUse {
  readonly type: "tool_use"
  readonly uuid: string
  readonly timestamp: string
  readonly name: string
  readonly input: unknown
  readonly tool_use_id: string
}

export interface ToolResult {
  readonly type: "tool_result"
  readonly uuid: string
  readonly timestamp: string
  readonly tool_use_id: string
  readonly output: string
  readonly is_error?: boolean
}

export interface ConversationSummary {
  readonly type: "summary"
  readonly uuid: string
  readonly timestamp: string
  readonly summary: string
  readonly turn_count: number
}

export type LogEntry = UserMessage | AssistantMessage | ConversationSummary | ToolUse | ToolResult

// Parse JSONL content
export const parseJSONL = (content: string): Effect.Effect<ReadonlyArray<LogEntry>, Error> =>
  Effect.gen(function*() {
    const lines = content.trim().split("\n").filter((line) => line.trim())
    const entries: Array<LogEntry> = []

    for (const [index, line] of lines.entries()) {
      try {
        const parsed = JSON.parse(line)

        // Validate and transform based on type
        if (parsed.type === "user") {
          // Extract text content from various possible structures
          let textContent = ""
          if (parsed.message?.text) {
            // Simple text format
            textContent = parsed.message.text
          } else if (parsed.message?.content) {
            // Array of content objects format
            if (Array.isArray(parsed.message.content)) {
              const textParts = parsed.message.content
                .filter((item: any) => item.type === "text")
                .map((item: any) => item.text)
              textContent = textParts.join("\n")
            } else if (typeof parsed.message.content === "string") {
              textContent = parsed.message.content
            }
          } else if (parsed.content) {
            // Direct content fallback
            textContent = typeof parsed.content === "string" ? parsed.content : ""
          }

          entries.push({
            type: "user",
            uuid: parsed.uuid || `user-${index}`,
            timestamp: parsed.timestamp || new Date().toISOString(),
            message: {
              text: textContent,
              images: parsed.message?.images
            }
          })
        } else if (parsed.type === "assistant") {
          entries.push({
            type: "assistant",
            uuid: parsed.uuid || `assistant-${index}`,
            timestamp: parsed.timestamp || new Date().toISOString(),
            message: {
              role: "assistant",
              content: parsed.message?.content || parsed.content || "",
              thinking: parsed.message?.thinking,
              model: parsed.message?.model || parsed.model,
              usage: parsed.message?.usage || parsed.usage
            }
          })
        } else if (parsed.type === "summary") {
          entries.push({
            type: "summary",
            uuid: parsed.uuid || `summary-${index}`,
            timestamp: parsed.timestamp || new Date().toISOString(),
            summary: parsed.summary || "",
            turn_count: parsed.turn_count || 0
          })
        } else if (parsed.type === "tool_use") {
          entries.push({
            type: "tool_use",
            uuid: parsed.uuid || `tool-use-${index}`,
            timestamp: parsed.timestamp || new Date().toISOString(),
            name: parsed.name || "",
            input: parsed.input || {},
            tool_use_id: parsed.id || parsed.tool_use_id || `tool-${index}`
          })
        } else if (parsed.type === "tool_result") {
          entries.push({
            type: "tool_result",
            uuid: parsed.uuid || `tool-result-${index}`,
            timestamp: parsed.timestamp || new Date().toISOString(),
            tool_use_id: parsed.tool_use_id || "",
            output: parsed.output || parsed.content || "",
            is_error: parsed.is_error || false
          })
        }
      } catch (error) {
        yield* Effect.logWarning(`Failed to parse line ${index + 1}: ${error}`)
        // Continue parsing other lines
      }
    }

    return entries
  })

// Extract session metadata from entries
export const extractSessionMetadata = (entries: ReadonlyArray<LogEntry>) => {
  const metadata = {
    messageCount: entries.length,
    userMessages: entries.filter((e) => e.type === "user").length,
    assistantMessages: entries.filter((e) => e.type === "assistant").length,
    firstMessage: entries[0]?.timestamp,
    lastMessage: entries[entries.length - 1]?.timestamp,
    totalTokens: 0,
    totalCost: 0,
    models: new Set<string>()
  }

  // Calculate token usage and models used
  for (const entry of entries) {
    if (entry.type === "assistant" && entry.message.usage) {
      metadata.totalTokens += entry.message.usage.total_tokens
    }
    if (entry.type === "assistant" && entry.message.model) {
      metadata.models.add(entry.message.model)
    }
  }

  // Estimate cost (rough approximation)
  // Claude 3.5 Sonnet: $3/1M input, $15/1M output
  const inputTokens = entries
    .filter((e) => e.type === "assistant")
    .reduce((sum, e) => sum + (e.message.usage?.input_tokens || 0), 0)
  const outputTokens = entries
    .filter((e) => e.type === "assistant")
    .reduce((sum, e) => sum + (e.message.usage?.output_tokens || 0), 0)

  metadata.totalCost = (inputTokens * 3 / 1_000_000) + (outputTokens * 15 / 1_000_000)

  return {
    ...metadata,
    models: Array.from(metadata.models)
  }
}
