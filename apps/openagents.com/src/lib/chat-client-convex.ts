/**
 * Convex-backed chat client for OpenAgents.com
 * This replaces the in-memory storage with Convex persistence
 */
import { client } from "@openagentsinc/convex"
import { Effect } from "effect"

const ConvexClient = client.ConvexClient

// Type definitions for Convex data
interface ConvexSession {
  id: string
  user_id: string
  project_path: string
  project_name?: string
  status: string
  started_at: number
  last_activity: number
  message_count: number
  total_cost: number
}

interface ConvexMessage {
  entry_uuid: string
  session_id: string
  entry_type: string
  role?: string
  content?: string
  thinking?: string
  summary?: string
  model?: string
  token_usage?: {
    input_tokens: number
    output_tokens: number
    total_tokens: number
  }
  cost?: number
  timestamp: number
  turn_count?: number
  tool_name?: string
  tool_input?: any
  tool_use_id?: string
  tool_output?: string
  tool_is_error?: boolean
}

// Hardcoded user ID for now (as requested)
// NOTE: Sessions were imported with "claude-code-user" as the user_id
const HARDCODED_USER_ID = "claude-code-user"

// Debug logging helper - DISABLED
const DEBUG = false
function debug(message: string, data?: any) {
  if (DEBUG) {
    console.log(`[chat-client-convex] ${message}`, data || "")
  }
}

/**
 * Get all sessions for the hardcoded user
 */
export async function getConversations() {
  debug("getConversations called for user:", HARDCODED_USER_ID)

  try {
    debug("Calling ConvexClient.sessions.listByUser")
    const sessions = await Effect.runPromise(
      ConvexClient.sessions.listByUser(HARDCODED_USER_ID)
    ) as Array<ConvexSession>

    debug("Sessions received from Convex:", { count: sessions.length, sessions })

    // Transform to match expected format
    const transformed = sessions.map((session: ConvexSession) => ({
      id: session.id,
      title: session.project_name || session.project_path || "Untitled Session",
      lastMessageAt: new Date(session.last_activity),
      createdAt: new Date(session.started_at),
      model: "claude-code", // Indicator that this is from Claude Code
      metadata: {
        messageCount: session.message_count,
        totalCost: session.total_cost,
        status: session.status,
        projectPath: session.project_path
      }
    }))

    debug("Transformed sessions:", transformed)
    return transformed
  } catch (error) {
    console.error("Failed to load conversations from Convex:", error)
    debug("Error details:", {
      error,
      message: error instanceof Error ? error.message : "Unknown error",
      stack: error instanceof Error ? error.stack : undefined
    })
    return []
  }
}

/**
 * Get a session with its messages
 */
export async function getConversationWithMessages(sessionId: string) {
  debug("getConversationWithMessages called for session:", sessionId)

  try {
    // Get session
    debug("Fetching session from Convex")
    const session = await Effect.runPromise(
      ConvexClient.sessions.getById(sessionId)
    ) as ConvexSession | null

    debug("Session received:", session)

    if (!session) {
      throw new Error("Session not found")
    }

    // Get messages
    debug("Fetching messages from Convex")
    const messages = await Effect.runPromise(
      ConvexClient.messages.listBySession(sessionId, 1000) // Get up to 1000 messages
    ) as Array<ConvexMessage>

    debug("Messages received:", { count: messages.length })

    // Log first 5 messages in detail for debugging
    messages.slice(0, 5).forEach((msg, index) => {
      debug(`Message ${index}:`, {
        entry_uuid: msg.entry_uuid,
        entry_type: msg.entry_type,
        role: msg.role,
        content: msg.content ? msg.content.substring(0, 100) + (msg.content.length > 100 ? "..." : "") : "empty",
        thinking: msg.thinking ? "present" : "absent",
        summary: msg.summary ? "present" : "absent",
        tool_name: msg.tool_name,
        tool_output: msg.tool_output ? "present" : "absent",
        timestamp: new Date(msg.timestamp).toISOString()
      })
    })

    // Transform session to match expected format
    const conversation = {
      id: session.id,
      title: session.project_name || session.project_path || "Untitled Session",
      lastMessageAt: new Date(session.last_activity),
      createdAt: new Date(session.started_at),
      model: "claude-code",
      metadata: {
        messageCount: session.message_count,
        totalCost: session.total_cost,
        status: session.status,
        projectPath: session.project_path
      }
    }

    // Transform messages to match expected format
    const transformedMessages = messages
      .filter((msg: ConvexMessage) => {
        // Filter out entries that shouldn't be displayed as messages
        if (msg.entry_type === "summary" && !msg.summary) {
          debug(`Filtering out empty summary entry ${msg.entry_uuid}`)
          return false
        }
        return true
      })
      .map((msg: ConvexMessage) => {
        debug(`Transforming message ${msg.entry_uuid}:`, {
          entry_type: msg.entry_type,
          role: msg.role,
          hasContent: !!msg.content,
          contentType: typeof msg.content
        })

        const parsedContent = parseMessageContent(msg)
        debug(
          `Parsed content for ${msg.entry_uuid}:`,
          parsedContent.substring(0, 100) + (parsedContent.length > 100 ? "..." : "")
        )

        return {
          id: msg.entry_uuid,
          conversationId: msg.session_id,
          role: determineRole(msg),
          content: parsedContent,
          timestamp: new Date(msg.timestamp),
          model: msg.model,
          metadata: {
            entryType: msg.entry_type,
            thinking: msg.thinking,
            summary: msg.summary,
            toolName: msg.tool_name,
            toolInput: msg.tool_input,
            toolUseId: msg.tool_use_id,
            toolOutput: msg.tool_output,
            toolIsError: msg.tool_is_error,
            tokenUsage: msg.token_usage,
            cost: msg.cost,
            turnCount: msg.turn_count
          }
        }
      })

    return { conversation, messages: transformedMessages }
  } catch (error) {
    console.error("Failed to load conversation from Convex:", error)
    throw error
  }
}

/**
 * Determine the role for a message
 */
function determineRole(message: ConvexMessage): "user" | "assistant" | "system" {
  // First check if role is explicitly set
  if (message.role) {
    debug(`Using explicit role for ${message.entry_uuid}: ${message.role}`)
    return message.role as "user" | "assistant" | "system"
  }

  // Then determine based on entry_type
  switch (message.entry_type) {
    case "user":
      debug(`Setting role as 'user' based on entry_type for ${message.entry_uuid}`)
      return "user"
    case "assistant":
      debug(`Setting role as 'assistant' based on entry_type for ${message.entry_uuid}`)
      return "assistant"
    case "summary":
    case "tool_use":
    case "tool_result":
      debug(`Setting role as 'system' for ${message.entry_type} entry ${message.entry_uuid}`)
      return "system"
    default:
      debug(`Unknown entry_type '${message.entry_type}', defaulting to 'system' for ${message.entry_uuid}`)
      return "system"
  }
}

/**
 * Parse message content from stored format
 * Claude Code messages can have complex content structures stored as JSON strings
 */
function parseMessageContent(message: ConvexMessage): string {
  debug(`Parsing content for entry_type: ${message.entry_type}, entry_uuid: ${message.entry_uuid}`)
  // Handle different message types
  switch (message.entry_type) {
    case "user":
      // User messages from Claude Code have content in a specific format
      if (message.content) {
        debug(`User message content type: ${typeof message.content}`)
        try {
          // Check if it's already a string
          if (typeof message.content === "string") {
            // Check if it's HTML (Claude Code format) - we need to strip it
            if (
              message.content.includes("<span") || message.content.includes("→") || message.content.includes("<div")
            ) {
              debug(`User content appears to be HTML from Claude Code`)
              // Strip HTML tags to get plain text
              const plainText = message.content
                .replace(/<[^>]*>/g, "") // Remove HTML tags
                .replace(/&lt;/g, "<")
                .replace(/&gt;/g, ">")
                .replace(/&amp;/g, "&")
                .replace(/&quot;/g, "\"")
                .replace(/&#39;/g, "'")
                .trim()
              return plainText || message.content // Fallback to original if stripping fails
            }
            // Try to parse as JSON first
            try {
              const parsed = JSON.parse(message.content)
              if (parsed.text) {
                debug(`Found text field in parsed user content`)
                return parsed.text
              }
              // If it's an array format
              if (Array.isArray(parsed)) {
                const textParts = parsed
                  .filter((part: any) => part.type === "text")
                  .map((part: any) => part.text || "")
                  .join("\n")
                debug(`Extracted text from array format: ${textParts.substring(0, 50)}...`)
                return textParts
              }
            } catch {
              // Not JSON, return as-is
              debug(`User content is plain text`)
              return message.content
            }
          }
          return String(message.content)
        } catch (error) {
          debug(`Error parsing user content:`, error)
          return String(message.content || "")
        }
      }
      // Show empty message indicator
      debug(`User message has empty content`)
      return "[Empty message]"

    case "assistant":
      // Assistant messages might have complex content
      if (message.content) {
        debug(`Assistant message content type: ${typeof message.content}`)
        // Check if it's HTML (Claude Code format) - we need to strip it
        if (
          typeof message.content === "string" &&
          (message.content.includes("<span") || message.content.includes("→") || message.content.includes("<div"))
        ) {
          debug(`Assistant content appears to be HTML from Claude Code`)
          // Strip HTML tags to get plain text
          const plainText = message.content
            .replace(/<[^>]*>/g, "") // Remove HTML tags
            .replace(/&lt;/g, "<")
            .replace(/&gt;/g, ">")
            .replace(/&amp;/g, "&")
            .replace(/&quot;/g, "\"")
            .replace(/&#39;/g, "'")
            .trim()
          return plainText || message.content // Fallback to original if stripping fails
        }
        try {
          const parsed = JSON.parse(message.content)
          // Handle multi-part messages (text + tool use)
          if (Array.isArray(parsed)) {
            const parts = parsed.map((part: any) => {
              if (part.type === "text") {
                return part.text || ""
              }
              if (part.type === "tool_use") {
                // Tool uses should be separate entries, not inline
                debug(`Skipping inline tool_use in assistant message`)
                return ""
              }
              return ""
            }).filter(Boolean)

            const result = parts.join("\n").trim()
            debug(`Extracted assistant text from array: ${result.substring(0, 50)}...`)
            return result
          }
          // If it's an object with text field
          if (parsed.text) {
            return parsed.text
          }
          // Otherwise return the original content
          return message.content
        } catch {
          // Not JSON, return as-is
          debug(`Assistant content is plain text`)
          return message.content
        }
      }
      // If no content but has thinking, show thinking
      if (message.thinking) {
        debug(`Assistant has no content but has thinking`)
        return `💭 [Thinking]\n${message.thinking}`
      }
      return ""

    case "tool_use": {
      debug(`Formatting tool_use entry: ${message.tool_name}`)
      const toolInputStr = typeof message.tool_input === "string"
        ? message.tool_input
        : JSON.stringify(message.tool_input, null, 2)
      return `🔧 Tool: ${message.tool_name || "Unknown"}\n\nInput:\n\`\`\`json\n${toolInputStr}\n\`\`\``
    }

    case "tool_result": {
      debug(`Formatting tool_result entry`)
      if (!message.tool_output) return "No output"

      // Format tool output nicely
      const output = message.tool_output
      // Return plain text without markdown formatting to avoid breaking the layout
      return `📤 Tool Result:\n${output}`
    }

    case "summary":
      debug(`Formatting summary entry`)
      return `📝 Summary: ${message.summary || "No summary"}`

    default:
      debug(`Unknown entry_type: ${message.entry_type}, returning content as-is`)
      return message.content || ""
  }
}

/**
 * Create a new conversation - for Claude Code sessions this is handled by Overlord
 */
export async function createConversation(_title?: string): Promise<string> {
  throw new Error("Cannot create Claude Code sessions from web UI - use Overlord sync")
}

/**
 * Add a message - for Claude Code sessions this is handled by Overlord
 */
export async function addMessage(
  _conversationId: string,
  _role: "user" | "assistant",
  _content: string
): Promise<void> {
  throw new Error("Cannot add messages to Claude Code sessions from web UI - use Overlord sync")
}

/**
 * Update conversation title - for Claude Code sessions this is handled by Overlord
 */
export async function updateConversationTitle(
  _conversationId: string,
  _title: string
): Promise<void> {
  throw new Error("Cannot update Claude Code sessions from web UI - use Overlord sync")
}

/**
 * Delete a conversation - not supported for Claude Code sessions
 */
export async function deleteConversation(_conversationId: string): Promise<void> {
  throw new Error("Cannot delete Claude Code sessions from web UI")
}

/**
 * Search conversations by content
 */
export async function searchConversations(query: string) {
  debug("searchConversations called with query:", query)
  const allConversations = await getConversations()
  const filtered = allConversations.filter((conv: any) =>
    conv.title?.toLowerCase().includes(query.toLowerCase()) ||
    conv.metadata?.projectPath?.toLowerCase().includes(query.toLowerCase())
  )
  debug("Search results:", { query, totalCount: allConversations.length, filteredCount: filtered.length })
  return filtered
}
