#!/usr/bin/env bun

/**
 * Find message with ID b71910df-a580-4308-8085-33557be098f4
 */

import { Effect } from "effect"
import { ConvexClient } from "./src/client.js"

const TARGET_UUID = "b71910df-a580-4308-8085-33557be098f4"

const findMessage = Effect.gen(function*() {
  console.log(`\nSearching for message: ${TARGET_UUID}`)

  // Try to get the message directly
  const message = yield* ConvexClient.messages.getByUuid("", TARGET_UUID).pipe(
    Effect.catchAll((error) => {
      console.log("Direct lookup failed:", error)
      return Effect.succeed(null)
    })
  )

  if (message) {
    console.log("\n" + "=".repeat(80))
    console.log("FOUND MESSAGE IN CONVEX")
    console.log("=".repeat(80))
    console.log("UUID:", message.uuid)
    console.log("Role:", message.role)
    console.log("Entry type:", message.entry_type)
    console.log("Content length:", message.content?.length || 0, "characters")
    console.log("Created at:", message.created_at)
    console.log("Session ID:", message.session_id)

    // Show first part of content
    if (message.content) {
      console.log("\nFirst 1000 characters of content:")
      console.log(message.content.substring(0, 1000))
      console.log("\n...")

      // Check if it contains tool results
      const hasToolResults = message.content.includes("tool_results") ||
        message.content.includes("tool_calls") ||
        message.content.includes("<function_calls>") ||
        message.content.includes("<function_results>")

      console.log("\nContains tool results indicators:", hasToolResults)

      // Check for specific patterns
      console.log("\nContent analysis:")
      console.log("- Contains 'tool_results':", message.content.includes("tool_results"))
      console.log("- Contains 'tool_calls':", message.content.includes("tool_calls"))
      console.log("- Contains '<function_calls>':", message.content.includes("<function_calls>"))
      console.log("- Contains '<function_results>':", message.content.includes("<function_results>"))
      console.log("- Contains 'function' (case-insensitive):", message.content.toLowerCase().includes("function"))

      // Show the last 1000 characters too
      console.log("\nLast 1000 characters of content:")
      console.log("...")
      console.log(message.content.substring(message.content.length - 1000))
    }

    // Show all fields
    console.log("\n" + "=".repeat(80))
    console.log("FULL MESSAGE OBJECT:")
    console.log("=".repeat(80))
    console.log(JSON.stringify(message, null, 2))
  } else {
    console.log("\nMessage NOT found in Convex database")

    // Let's try to search through all messages
    console.log("\nAttempting to search through all messages...")
    const sessions = yield* ConvexClient.sessions.list().pipe(
      Effect.catchAll(() => Effect.succeed([]))
    )

    let found = false
    for (const session of sessions) {
      const messages = yield* ConvexClient.messages.listBySession(session._id).pipe(
        Effect.catchAll(() => Effect.succeed([]))
      )

      for (const msg of messages) {
        if (msg.uuid === TARGET_UUID) {
          console.log("\nFOUND in session:", session._id)
          console.log("Message:", msg)
          found = true
          break
        }
      }

      if (found) break
    }

    if (!found) {
      console.log("\nMessage not found in any session")
    }
  }
})

async function main() {
  await Effect.runPromise(findMessage).catch(console.error)
}

main()
