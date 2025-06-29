#!/usr/bin/env bun

/**
 * Analyze why message b71910df-a580-4308-8085-33557be098f4 isn't being detected as tool result
 */

import { Effect } from "effect"
import { ConvexClient } from "./src/client.js"

const TARGET_UUID = "b71910df-a580-4308-8085-33557be098f4"

const analyzeMessage = Effect.gen(function*() {
  console.log(`\nAnalyzing message: ${TARGET_UUID}`)

  // Get the message
  const message = yield* ConvexClient.messages.getByUuid("", TARGET_UUID).pipe(
    Effect.catchAll((error) => {
      console.log("Direct lookup failed:", error)
      return Effect.succeed(null)
    })
  )

  if (!message) {
    console.log("Message not found")
    return
  }

  console.log("\n=== Message Details ===")
  console.log("Role:", message.role)
  console.log("Entry type:", message.entry_type)
  console.log("Content length:", message.content?.length || 0)

  if (message.content) {
    console.log("\n=== Content Analysis ===")

    // Check if content starts with [{
    const startsWithBracket = message.content.startsWith("[{")
    console.log("Starts with '[{':", startsWithBracket)

    // Try to parse as JSON
    try {
      const parsed = JSON.parse(message.content)
      console.log("\nJSON parse successful!")
      console.log("Is array:", Array.isArray(parsed))

      if (Array.isArray(parsed) && parsed.length > 0) {
        const firstElement = parsed[0]
        console.log("\nFirst element:")
        console.log("- type:", firstElement.type)
        console.log("- tool_use_id:", firstElement.tool_use_id)
        console.log("- is_error:", firstElement.is_error)
        console.log("- content length:", firstElement.content?.length || 0)

        // Check if it should be detected as tool result
        const isToolResult = firstElement.type === "tool_result"
        console.log("\nShould be detected as tool result:", isToolResult)

        if (isToolResult) {
          console.log("\n✅ This message SHOULD be detected as a tool result")
          console.log("The detection logic in renderChatMessage should work for this")
        }
      }
    } catch (e) {
      console.log("\nJSON parse failed:", e.message)
      console.log("First 200 chars:", message.content.substring(0, 200))
    }

    // Check the rendered field
    console.log("\n=== Rendered Field ===")
    console.log("Has rendered field:", !!message.rendered)
    if (message.rendered) {
      console.log("Rendered length:", message.rendered.length)
      console.log("First 500 chars of rendered:")
      console.log(message.rendered.substring(0, 500))
    }

    // Simulate what renderChatMessage would do
    console.log("\n=== Simulating renderChatMessage Logic ===")
    if (message.role === "user" && message.content && message.content.startsWith("[{")) {
      try {
        const parsed = JSON.parse(message.content)
        if (Array.isArray(parsed) && parsed[0]?.type === "tool_result") {
          console.log("✅ renderChatMessage SHOULD format this as a tool result")

          // Check if there's a rendered field that might be overriding
          if (message.rendered) {
            console.log("\n⚠️ BUT: Message has a 'rendered' field which takes precedence!")
            console.log("The rendered field will be used instead of the tool result formatting")
          }
        }
      } catch (e) {
        console.log("❌ renderChatMessage would fail to parse JSON:", e.message)
      }
    } else {
      console.log("❌ renderChatMessage wouldn't detect this as a tool result")
      console.log("- Role is 'user':", message.role === "user")
      console.log("- Has content:", !!message.content)
      console.log("- Starts with '[{':", message.content?.startsWith("[{"))
    }
  }
})

async function main() {
  await Effect.runPromise(analyzeMessage).catch(console.error)
}

main()
