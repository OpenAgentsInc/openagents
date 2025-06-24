import * as JSONLParser from "./JSONLParser.js"

// Database record types that match our schema
export interface SessionRecord {
  id: string
  user_id: string
  project_path: string
  project_name?: string
  status: "active" | "inactive" | "archived"
  started_at: Date
  last_activity: Date
  message_count: number
  total_cost: number
}

export interface MessageRecord {
  session_id: string
  entry_uuid: string
  entry_type: "user" | "assistant" | "summary" | "tool_use" | "tool_result"
  role?: "user" | "assistant" | "system" | undefined
  content?: string | undefined
  thinking?: string | undefined
  summary?: string | undefined
  model?: string | undefined
  token_usage?: {
    input_tokens: number
    output_tokens: number
    total_tokens: number
  } | undefined
  cost?: number | undefined
  timestamp: Date
  turn_count?: number | undefined
  tool_name?: string | undefined
  tool_input?: unknown | undefined
  tool_use_id?: string | undefined
  tool_output?: string | undefined
  tool_is_error?: boolean | undefined
}

export interface ImageRecord {
  message_id?: number
  image_data: string
  mime_type: string
  position: number
}

// Convert a LogEntry to a MessageRecord for database insertion
export const logEntryToMessageRecord = (
  entry: JSONLParser.LogEntry,
  sessionId: string
): MessageRecord => {
  const base = {
    session_id: sessionId,
    entry_uuid: entry.uuid,
    timestamp: new Date(entry.timestamp)
  }

  switch (entry.type) {
    case "user":
      return {
        ...base,
        entry_type: "user",
        role: "user",
        content: entry.message.text
        // Images handled separately
      }

    case "assistant":
      return {
        ...base,
        entry_type: "assistant",
        role: "assistant",
        content: entry.message.content,
        thinking: entry.message.thinking,
        model: entry.message.model,
        token_usage: entry.message.usage,
        cost: calculateCost(entry.message.usage)
      }

    case "summary":
      return {
        ...base,
        entry_type: "summary",
        summary: entry.summary,
        turn_count: entry.turn_count
      }

    case "tool_use":
      return {
        ...base,
        entry_type: "tool_use",
        tool_name: entry.name,
        tool_input: entry.input,
        tool_use_id: entry.tool_use_id
      }

    case "tool_result":
      return {
        ...base,
        entry_type: "tool_result",
        tool_use_id: entry.tool_use_id,
        tool_output: entry.output,
        tool_is_error: entry.is_error
      }
  }
}

// Extract images from a user message
export const extractImages = (
  entry: JSONLParser.UserMessage
): ReadonlyArray<ImageRecord> => {
  if (!entry.message.images || entry.message.images.length === 0) {
    return []
  }

  return entry.message.images.map((imageData, index) => ({
    image_data: imageData,
    mime_type: detectMimeType(imageData),
    position: index
  }))
}

// Helper to detect MIME type from base64 data
const detectMimeType = (base64Data: string): string => {
  // Check the base64 header for common image formats
  if (base64Data.startsWith("data:image/png")) return "image/png"
  if (base64Data.startsWith("data:image/jpeg")) return "image/jpeg"
  if (base64Data.startsWith("data:image/jpg")) return "image/jpeg"
  if (base64Data.startsWith("data:image/gif")) return "image/gif"
  if (base64Data.startsWith("data:image/webp")) return "image/webp"

  // If it's raw base64 without data URI, try to detect from content
  const header = base64Data.substring(0, 16)
  if (header.includes("PNG")) return "image/png"
  if (header.includes("JFIF") || header.includes("Exif")) return "image/jpeg"
  if (header.includes("GIF")) return "image/gif"

  return "image/png" // Default fallback
}

// Calculate cost based on token usage
const calculateCost = (usage?: {
  input_tokens: number
  output_tokens: number
  total_tokens: number
}): number => {
  if (!usage) return 0

  // Claude 3.5 Sonnet pricing (as of Phase 1 implementation)
  const INPUT_COST_PER_MILLION = 3 // $3 per million input tokens
  const OUTPUT_COST_PER_MILLION = 15 // $15 per million output tokens

  const inputCost = (usage.input_tokens * INPUT_COST_PER_MILLION) / 1_000_000
  const outputCost = (usage.output_tokens * OUTPUT_COST_PER_MILLION) / 1_000_000

  return inputCost + outputCost
}

// Create a session record from metadata
export const createSessionRecord = (
  sessionId: string,
  userId: string,
  projectPath: string,
  entries: ReadonlyArray<JSONLParser.LogEntry>
): SessionRecord => {
  const metadata = JSONLParser.extractSessionMetadata(entries)

  return {
    id: sessionId,
    user_id: userId,
    project_path: projectPath,
    project_name: extractProjectName(projectPath),
    status: "active",
    started_at: new Date(metadata.firstMessage || Date.now()),
    last_activity: new Date(metadata.lastMessage || Date.now()),
    message_count: metadata.messageCount,
    total_cost: metadata.totalCost
  }
}

// Extract a friendly project name from the hashed path
const extractProjectName = (projectPath: string): string => {
  // Project paths are hashed, so we can't get the real name
  // In Phase 2, we might want to store a mapping
  return `Project ${projectPath.substring(0, 8)}`
}
