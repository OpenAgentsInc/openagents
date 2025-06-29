#!/usr/bin/env tsx

import { Effect } from "effect"
import { ConvexClient } from "./src/client"

async function checkSessions() {
  console.log("Checking sessions with messages...\n")

  try {
    const result = await Effect.runPromise(
      Effect.gen(function*() {
        // Get sessions for the hardcoded user
        const sessions = yield* ConvexClient.sessions.listByUser("claude-code-user")
        console.log(`Found ${sessions.length} sessions for user claude-code-user\n`)

        const sessionsWithMessages: Array<any> = []

        // Check each session
        for (const session of sessions) {
          const messages = yield* ConvexClient.messages.listBySession(session._id, 10)

          if (messages.length > 0) {
            console.log(`✓ Session ${session._id} (${session.project_name || session.project_path || "Unknown"})`)
            console.log(`  - ${messages.length} messages`)
            console.log(`  - First message: ${messages[0].entry_type} - ${messages[0].entry_uuid}`)
            console.log(`  - Last activity: ${new Date(session.last_activity).toISOString()}`)

            sessionsWithMessages.push({
              session,
              messageCount: messages.length,
              firstMessage: messages[0]
            })
          }
        }

        return sessionsWithMessages
      })
    )

    console.log(`\n${"=".repeat(80)}`)
    console.log(`SUMMARY: ${result.length} sessions have messages`)
    console.log(`${"=".repeat(80)}\n`)

    // Look for sessions with the most recent activity
    if (result.length > 0) {
      result.sort((a, b) => b.session.last_activity - a.session.last_activity)

      console.log("Most recent session with messages:")
      const recent = result[0]
      console.log(`- Session ID: ${recent.session._id}`)
      console.log(`- Project: ${recent.session.project_name || recent.session.project_path || "Unknown"}`)
      console.log(`- Last activity: ${new Date(recent.session.last_activity).toISOString()}`)
      console.log(`- Message count: ${recent.session.message_count}`)

      // Get more messages from this session
      const messages = await Effect.runPromise(
        ConvexClient.messages.listBySession(recent.session._id, 100)
      )

      console.log(`\nFirst 10 messages from this session:`)
      messages.slice(0, 10).forEach((msg: any, index: number) => {
        console.log(
          `${index + 1}. ${msg.entry_uuid} - ${msg.entry_type} - ${msg.role || "no role"} - content: ${
            msg.content ? "present" : "empty"
          }`
        )
      })
    }
  } catch (error) {
    console.error("\n❌ Error:", error)
  }
}

// Run the check
checkSessions().then(() => {
  console.log("\n✅ Check complete")
  process.exit(0)
}).catch((error) => {
  console.error("Fatal error:", error)
  process.exit(1)
})
