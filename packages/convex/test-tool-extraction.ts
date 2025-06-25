#!/usr/bin/env bun

/**
 * Test tool extraction from Claude Code messages
 */

import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"

const testSessionId = "466d695f-2808-42f3-97d3-2465cfb138a7"

const main = Effect.gen(function*() {
  console.log("Testing tool extraction for session:", testSessionId)
  
  // Get some messages from the session
  const messages = yield* ConvexClient.messages.listBySession(testSessionId, 20)
  
  console.log(`Found ${messages.length} messages`)
  
  // Look for assistant messages with potential tool content
  for (const msg of messages) {
    if (msg.entry_type === "assistant" && msg.content) {
      // Check if content contains tool_use
      if (msg.content.includes('"type":"tool_use"') || msg.content.includes('tool_use')) {
        console.log("\n=== Found message with tool_use content ===")
        console.log("Entry UUID:", msg.entry_uuid)
        console.log("Content preview:", msg.content.substring(0, 200) + "...")
        
        // Try to parse and extract tool info
        try {
          const parsed = JSON.parse(msg.content)
          if (Array.isArray(parsed)) {
            const toolUses = parsed.filter((item: any) => item.type === "tool_use")
            if (toolUses.length > 0) {
              console.log("\nExtracted tool uses:")
              toolUses.forEach((tool: any) => {
                console.log(`- Tool: ${tool.name}, ID: ${tool.id}`)
                console.log(`  Input:`, JSON.stringify(tool.input).substring(0, 100) + "...")
              })
            }
          }
        } catch (e) {
          console.log("Failed to parse as JSON")
        }
      }
    }
    
    // Also check existing tool fields
    if (msg.tool_name || msg.tool_use_id) {
      console.log("\n=== Message with tool fields ===")
      console.log("Entry UUID:", msg.entry_uuid)
      console.log("Tool name:", msg.tool_name)
      console.log("Tool ID:", msg.tool_use_id)
    }
  }
})

Effect.runPromise(main).catch(console.error)