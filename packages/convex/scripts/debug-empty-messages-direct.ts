#!/usr/bin/env tsx

import { Effect } from "effect"
import { ConvexClient } from "./src/client"

// Target message IDs
const targetMessageIds = [
  "c7b7ab32-5a6d-438b-bd73-f97b2a75ec42", // Blue box (assistant)
  "75f5d516-754d-4d9a-bb42-a272fa37c30b" // Green box with "[Empty message]" (user)
]

async function debugEmptyMessages() {
  console.log("Searching for empty messages using Effect client...\n")

  try {
    const result = await Effect.runPromise(
      Effect.gen(function*() {
        // Get sessions for the hardcoded user
        const sessions = yield* ConvexClient.sessions.listByUser("claude-code-user")
        console.log(`Found ${sessions.length} sessions for user claude-code-user\n`)

        const foundMessages: Array<any> = []

        // Search through each session
        for (const session of sessions) {
          console.log(`Searching session ${session._id} (${session.project_name || session.project_path})...`)

          const messages = yield* ConvexClient.messages.listBySession(session._id, 1000)
          console.log(`  - Found ${messages.length} messages`)

          // Check if our target messages are in this session
          for (const targetId of targetMessageIds) {
            const foundMessage = messages.find((msg: any) => msg.entry_uuid === targetId)

            if (foundMessage) {
              console.log(`  ✓ Found message ${targetId}!`)
              foundMessages.push({
                ...foundMessage,
                sessionId: session._id,
                sessionTitle: session.project_name || session.project_path || "Unknown"
              })
            }
          }
        }

        return foundMessages
      })
    )

    console.log(`\n${"=".repeat(80)}`)
    console.log(`RESULTS: Found ${result.length} of ${targetMessageIds.length} target messages`)
    console.log(`${"=".repeat(80)}\n`)

    // Analyze each found message
    for (const msg of result) {
      console.log(`\n${"=".repeat(80)}`)
      console.log(`Message: ${msg.entry_uuid}`)
      console.log(`${"=".repeat(80)}`)

      console.log(`Session: ${msg.sessionId} (${msg.sessionTitle})`)
      console.log(`Entry Type: ${msg.entry_type}`)
      console.log(`Role: ${msg.role || "undefined"}`)
      console.log(`Timestamp: ${new Date(msg.timestamp).toISOString()}`)

      console.log(`\nContent Analysis:`)
      console.log(`- Content is null: ${msg.content === null}`)
      console.log(`- Content is undefined: ${msg.content === undefined}`)
      console.log(`- Content is empty string: ${msg.content === ""}`)
      console.log(`- Content type: ${typeof msg.content}`)
      console.log(`- Content length: ${msg.content ? msg.content.length : "N/A"}`)

      if (msg.content) {
        console.log(`\nContent Preview (first 500 chars):`)
        console.log(msg.content.substring(0, 500))

        // Check if it's JSON
        try {
          const parsed = JSON.parse(msg.content)
          console.log(`\n✅ Content is valid JSON`)
          console.log(`JSON structure:`, JSON.stringify(parsed, null, 2).substring(0, 500))
        } catch {
          console.log(`\n❌ Content is not JSON`)
        }

        // Check for HTML
        if (msg.content.includes("<") || msg.content.includes("&")) {
          console.log(`\nHTML/Entity Detection:`)
          console.log(`- Contains <: ${msg.content.includes("<")}`)
          console.log(`- Contains >: ${msg.content.includes(">")}`)
          console.log(`- Contains &lt;: ${msg.content.includes("&lt;")}`)
          console.log(`- Contains &gt;: ${msg.content.includes("&gt;")}`)
          console.log(`- Contains &amp;: ${msg.content.includes("&amp;")}`)
        }
      }

      console.log(`\nOther Fields:`)
      console.log(`- thinking: ${msg.thinking ? "present" : "absent"}`)
      console.log(`- summary: ${msg.summary ? "present" : "absent"}`)
      console.log(`- tool_name: ${msg.tool_name || "undefined"}`)
      console.log(`- tool_output: ${msg.tool_output ? "present" : "absent"}`)

      // Save full message for inspection
      const filename = `message-${msg.entry_uuid}.json`
      await Bun.write(filename, JSON.stringify(msg, null, 2))
      console.log(`\n💾 Full message saved to: ${filename}`)
    }

    if (result.length === 0) {
      console.log("\n❌ No messages found with the specified UUIDs")
      console.log("Target UUIDs:")
      targetMessageIds.forEach((id) => console.log(`  - ${id}`))
    }
  } catch (error) {
    console.error("\n❌ Error during search:", error)
  }
}

// Run the debug script
debugEmptyMessages().then(() => {
  console.log("\n✅ Debug complete")
  process.exit(0)
}).catch((error) => {
  console.error("Fatal error:", error)
  process.exit(1)
})
