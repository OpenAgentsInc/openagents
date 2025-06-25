#!/usr/bin/env bun

/**
 * Debug the exact rendering of message b71910df-a580-4308-8085-33557be098f4
 */

import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"
import { renderChatMessage } from "../../apps/openagents.com/src/lib/chat-utils"

const TARGET_UUID = "b71910df-a580-4308-8085-33557be098f4"

const debugRendering = Effect.gen(function*() {
  console.log(`\nDebugging rendering for message: ${TARGET_UUID}`)
  
  // Get the message from Convex
  const message = yield* ConvexClient.messages.getByUuid("", TARGET_UUID).pipe(
    Effect.catchAll(() => Effect.succeed(null))
  )
  
  if (!message) {
    console.log("Message not found")
    return
  }

  console.log("\n=== Raw Message from Convex ===")
  console.log("Role:", message.role)
  console.log("Content length:", message.content?.length || 0)
  console.log("Content starts with '[{':", message.content?.startsWith('[{'))
  console.log("Has rendered field:", !!message.rendered)
  
  // Test rendering with the exact message structure
  console.log("\n=== Testing renderChatMessage ===")
  
  // First, render without any modifications
  console.log("\n1. Direct rendering (as stored in Convex):")
  try {
    const html1 = renderChatMessage(message as any)
    console.log("Success!")
    console.log("Contains tool-result-section:", html1.includes("tool-result-section"))
    console.log("Contains Tool Result label:", html1.includes("Tool Result"))
    console.log("HTML length:", html1.length)
    
    // Save to file for inspection
    const fs = require('fs')
    fs.writeFileSync("debug-rendered-message.html", html1)
    console.log("Saved rendered HTML to debug-rendered-message.html")
  } catch (error) {
    console.log("Error:", error)
  }
  
  // Test with simulated markdown rendering
  console.log("\n2. With simulated markdown rendering:")
  const messageWithMarkdown = {
    ...message,
    rendered: `<p>${message.content?.replace(/</g, '&lt;').replace(/>/g, '&gt;')}</p>`
  }
  
  try {
    const html2 = renderChatMessage(messageWithMarkdown as any)
    console.log("Success!")
    console.log("Contains tool-result-section:", html2.includes("tool-result-section"))
    console.log("Contains Tool Result label:", html2.includes("Tool Result"))
  } catch (error) {
    console.log("Error:", error)
  }
  
  // Check what the actual content parse looks like
  console.log("\n=== Parsing Content ===")
  if (message.content) {
    try {
      const parsed = JSON.parse(message.content)
      console.log("Parse successful!")
      console.log("Is array:", Array.isArray(parsed))
      if (Array.isArray(parsed) && parsed[0]) {
        console.log("First element type:", parsed[0].type)
        console.log("Tool use ID:", parsed[0].tool_use_id)
        console.log("Content preview:", parsed[0].content?.substring(0, 100) + "...")
      }
    } catch (e) {
      console.log("Parse failed:", e.message)
    }
  }
})

async function main() {
  await Effect.runPromise(debugRendering).catch(console.error)
}

main()