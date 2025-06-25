#!/usr/bin/env bun

/**
 * Check how messages are transformed by chat-client-convex
 */

import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"

const testSessionId = "466d695f-2808-42f3-97d3-2465cfb138a7"

// Copy the parseMessageContent logic from chat-client-convex
function parseMessageContent(message: any): { content: string, toolMetadata?: any } {
  console.log(`Parsing content for entry_type: ${message.entry_type}, entry_uuid: ${message.entry_uuid}`)
  
  switch (message.entry_type) {
    case "assistant":
      if (message.content) {
        // Check if it's HTML (Claude Code format) - we need to strip it
        if (
          typeof message.content === "string" &&
          (message.content.includes("<span") || message.content.includes("→") || message.content.includes("<div"))
        ) {
          console.log(`Assistant content appears to be HTML from Claude Code`)
          // Strip HTML tags to get plain text
          const plainText = message.content
            .replace(/<[^>]*>/g, "") // Remove HTML tags
            .replace(/&lt;/g, "<")
            .replace(/&gt;/g, ">")
            .replace(/&amp;/g, "&")
            .replace(/&quot;/g, "\"")
            .replace(/&#39;/g, "'")
            .trim()
          return { content: plainText || message.content }
        }
        
        try {
          const parsed = JSON.parse(message.content)
          
          // Check for embedded tool uses
          if (Array.isArray(parsed)) {
            const toolUses = parsed.filter((item: any) => item.type === "tool_use")
            const textParts = parsed
              .filter((item: any) => item.type === "text")
              .map((item: any) => item.text || "")
            
            const toolMetadata = toolUses.length > 0 ? {
              toolName: toolUses[0].name,
              toolUseId: toolUses[0].id,
              toolInput: toolUses[0].input,
              hasEmbeddedTool: true
            } : undefined
            
            return {
              content: textParts.join("\n").trim(),
              toolMetadata
            }
          }
          
          // If it's an object with text field
          if (parsed.text) {
            return { content: parsed.text }
          }
          
          // Otherwise return the original content
          return { content: message.content }
        } catch {
          // Not JSON, return as-is
          console.log(`Assistant content is plain text`)
          return { content: message.content }
        }
      }
      
      // If no content but has thinking, show thinking
      if (message.thinking) {
        console.log(`Assistant has no content but has thinking`)
        return { content: `💭 [Thinking]\n${message.thinking}` }
      }
      return { content: "" }
      
    default:
      return { content: message.content || "" }
  }
}

const main = Effect.gen(function*() {
  console.log("Checking message transformation for session:", testSessionId)
  
  // Get some messages from the session
  const messages = yield* ConvexClient.messages.listBySession(testSessionId, 10)
  
  console.log(`\nFound ${messages.length} messages\n`)
  
  // Process each message
  for (const msg of messages) {
    if (msg.entry_type === "assistant") {
      console.log("\n" + "=".repeat(60))
      console.log("Entry UUID:", msg.entry_uuid)
      console.log("Entry Type:", msg.entry_type)
      console.log("Has tool_name field:", !!msg.tool_name)
      console.log("Has tool_use_id field:", !!msg.tool_use_id)
      
      const { content, toolMetadata } = parseMessageContent(msg)
      
      if (toolMetadata) {
        console.log("\n✅ EXTRACTED TOOL METADATA:")
        console.log("  Tool Name:", toolMetadata.toolName)
        console.log("  Tool ID:", toolMetadata.toolUseId)
        console.log("  Has Input:", !!toolMetadata.toolInput)
      } else {
        console.log("\n❌ No tool metadata extracted")
      }
      
      console.log("\nText content:", content.substring(0, 100) + (content.length > 100 ? "..." : ""))
    }
  }
})

Effect.runPromise(main).catch(console.error)