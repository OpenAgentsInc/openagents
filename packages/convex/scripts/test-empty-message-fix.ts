#!/usr/bin/env bun

/**
 * Test if the empty message fixes are working
 */

// Import the parseMessageContent logic from chat-client-convex
const parseMessageContent = (message: any): string => {
  const debug = (msg: string, data?: any) => console.log(`[DEBUG] ${msg}`, data || "")

  switch (message.entry_type) {
    case "user":
      if (message.content) {
        try {
          if (typeof message.content === "string") {
            try {
              const parsed = JSON.parse(message.content)

              // Check for tool_result format
              if (Array.isArray(parsed) && parsed.length > 0 && parsed[0].type === "tool_result") {
                debug(`User message is a tool_result`)
                const toolResult = parsed[0]
                const content = toolResult.content || ""
                // Format tool result nicely
                if (content.includes("→")) {
                  // It's file content with line numbers
                  return `📤 Tool Result:\n\`\`\`\n${content}\n\`\`\``
                }
                return `📤 Tool Result: ${content}`
              }

              if (parsed.text) {
                return parsed.text
              }
              // If it's an array format
              if (Array.isArray(parsed)) {
                const textParts = parsed
                  .filter((part: any) => part.type === "text")
                  .map((part: any) => part.text || "")
                  .join("\n")
                return textParts
              }
            } catch {
              // Not JSON, return as-is
              return message.content
            }
          }
          return String(message.content)
        } catch (error) {
          debug(`Error parsing user content:`, error)
          return String(message.content || "")
        }
      }
      return "[Empty message]"

    case "assistant":
      if (message.content) {
        try {
          const parsed = JSON.parse(message.content)
          // Handle multi-part messages (text + tool use)
          if (Array.isArray(parsed)) {
            const textParts = parsed.filter((part: any) => part.type === "text")
            const toolParts = parsed.filter((part: any) => part.type === "tool_use")

            // If there's no text but there are tools, show tool invocation
            if (textParts.length === 0 && toolParts.length > 0) {
              debug(`Assistant message contains only tool_use, no text`)
              const tool = toolParts[0]
              return `🔧 Using tool: ${tool.name}`
            }

            const parts = parsed.map((part: any) => {
              if (part.type === "text") {
                return part.text || ""
              }
              return ""
            }).filter(Boolean)

            return parts.join("\n").trim()
          }
          // If it's an object with text field
          if (parsed.text) {
            return parsed.text
          }
          // Otherwise return the original content
          return message.content
        } catch {
          // Not JSON, return as-is
          return message.content
        }
      }
      return ""

    default:
      return message.content || ""
  }
}

// Test the problematic messages
const testMessages = [
  {
    entry_uuid: "c7b7ab32-5a6d-438b-bd73-f97b2a75ec42",
    entry_type: "assistant",
    role: "assistant",
    content:
      "[{\"type\":\"tool_use\",\"id\":\"toolu_01NBF6ryfwphMM2RiWESkkr5\",\"name\":\"Read\",\"input\":{\"file_path\":\"/Users/christopherdavid/code/effectexamples/examples/http-server/src/People/Http.ts\"}}]"
  },
  {
    entry_uuid: "75f5d516-754d-4d9a-bb42-a272fa37c30b",
    entry_type: "user",
    role: "user",
    content:
      "[{\"tool_use_id\":\"toolu_01NBF6ryfwphMM2RiWESkkr5\",\"type\":\"tool_result\",\"content\":\"     1→import { HttpApiBuilder } from \\\"@effect/platform\\\"\\n     2→import { Effect, Layer, pipe } from \\\"effect\\\"\\n...\"}]"
  }
]

console.log("Testing message parsing fixes:\n")

for (const msg of testMessages) {
  console.log(`Message: ${msg.entry_uuid}`)
  console.log(`Type: ${msg.entry_type}`)
  console.log(`Original content: ${msg.content.substring(0, 100)}...`)

  const parsedContent = parseMessageContent(msg)
  console.log(`Parsed content: ${parsedContent}`)
  console.log("-".repeat(80) + "\n")
}

console.log("\n✅ Test complete. The messages should now display:")
console.log("1. Assistant message: '🔧 Using tool: Read'")
console.log("2. User message: '📤 Tool Result: [file content]'")
