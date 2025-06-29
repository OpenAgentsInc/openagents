#!/usr/bin/env tsx

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api"

const CONVEX_URL = process.env.NEXT_PUBLIC_CONVEX_URL || "https://humorous-marten-231.convex.cloud"

async function investigateMessages() {
  const client = new ConvexHttpClient(CONVEX_URL)

  const messageIds = [
    "c7b7ab32-5a6d-438b-bd73-f97b2a75ec42", // Blue box (assistant)
    "75f5d516-754d-4d9a-bb42-a272fa37c30b" // Green box with "[Empty message]" (user)
  ]

  console.log("Investigating empty messages via direct database query...\n")

  // First, let's try to find these messages by searching through recent sessions
  try {
    // Get recent sessions for the hardcoded user
    const sessions = await client.query(api.sessions.listByUser, { userId: "claude-code-user" })
    console.log(`Found ${sessions.length} sessions for user claude-code-user\n`)

    // Search through each session's messages
    for (const session of sessions) {
      const messages = await client.query(api.messages.listBySession, {
        sessionId: session._id,
        limit: 1000
      })

      // Check if our target messages are in this session
      for (const targetId of messageIds) {
        const foundMessage = messages.find((msg: any) => msg.entry_uuid === targetId)

        if (foundMessage) {
          console.log(`\n${"=".repeat(80)}`)
          console.log(`Found message ${targetId} in session ${session._id}`)
          console.log(`Session: ${session.project_name || session.project_path || "Unknown"}`)
          console.log(`${"=".repeat(80)}`)

          console.log("\n📄 Complete Message Object:")
          console.log(JSON.stringify(foundMessage, null, 2))

          // Analyze the content
          console.log("\n🔍 Content Analysis:")
          console.log(`- entry_uuid: ${foundMessage.entry_uuid}`)
          console.log(`- entry_type: ${foundMessage.entry_type}`)
          console.log(`- role: ${foundMessage.role || "undefined"}`)
          console.log(`- content type: ${typeof foundMessage.content}`)
          console.log(`- content is null: ${foundMessage.content === null}`)
          console.log(`- content is undefined: ${foundMessage.content === undefined}`)
          console.log(`- content is empty string: ${foundMessage.content === ""}`)
          console.log(`- content length: ${foundMessage.content ? foundMessage.content.length : "N/A"}`)

          if (foundMessage.content) {
            console.log(`\n📝 Raw Content (first 2000 chars):`)
            console.log(foundMessage.content.substring(0, 2000))

            // Check if it might be JSON
            try {
              const parsed = JSON.parse(foundMessage.content)
              console.log(`\n✅ Content is valid JSON:`)
              console.log(JSON.stringify(parsed, null, 2).substring(0, 1000))
            } catch {
              console.log(`\n❌ Content is not JSON`)
            }

            // Check for HTML patterns
            if (foundMessage.content.includes("<") || foundMessage.content.includes("&")) {
              console.log(`\n🌐 Content contains HTML-like characters`)
              console.log(`- Contains < : ${foundMessage.content.includes("<")}`)
              console.log(`- Contains > : ${foundMessage.content.includes(">")}`)
              console.log(`- Contains &lt; : ${foundMessage.content.includes("&lt;")}`)
              console.log(`- Contains &gt; : ${foundMessage.content.includes("&gt;")}`)
              console.log(`- Contains &amp; : ${foundMessage.content.includes("&amp;")}`)
            }
          } else {
            console.log(`\n⚠️  Content is ${foundMessage.content === null ? "null" : "undefined"}`)
          }

          // Check other fields that might contain the actual content
          console.log("\n🔎 Other Fields:")
          console.log(`- thinking: ${foundMessage.thinking ? "present" : "absent"}`)
          console.log(`- summary: ${foundMessage.summary ? "present" : "absent"}`)
          console.log(`- tool_name: ${foundMessage.tool_name || "undefined"}`)
          console.log(`- tool_output: ${foundMessage.tool_output ? "present" : "absent"}`)

          if (foundMessage.thinking) {
            console.log(`\n💭 Thinking content (first 500 chars):`)
            console.log(foundMessage.thinking.substring(0, 500))
          }

          if (foundMessage.tool_output) {
            console.log(`\n🔧 Tool output (first 500 chars):`)
            console.log(foundMessage.tool_output.substring(0, 500))
          }
        }
      }
    }

    console.log("\n✅ Search complete")
  } catch (error) {
    console.error("❌ Error during investigation:", error)
  }
}

// Run the investigation
investigateMessages().then(() => {
  process.exit(0)
}).catch((error) => {
  console.error("Fatal error:", error)
  process.exit(1)
})
