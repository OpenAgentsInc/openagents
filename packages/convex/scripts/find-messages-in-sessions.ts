#!/usr/bin/env tsx

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api"

const CONVEX_URL = process.env.NEXT_PUBLIC_CONVEX_URL || "https://humorous-marten-231.convex.cloud"

async function findMessagesInSessions() {
  const client = new ConvexHttpClient(CONVEX_URL)

  const targetMessageIds = [
    "c7b7ab32-5a6d-438b-bd73-f97b2a75ec42", // Blue box (assistant)
    "75f5d516-754d-4d9a-bb42-a272fa37c30b" // Green box with "[Empty message]" (user)
  ]

  console.log("Searching for messages in all sessions...\n")

  try {
    // Get all sessions
    const sessions = await client.query(api.sessions.list, { limit: 100 })
    console.log(`Found ${sessions.length} sessions to search through\n`)

    for (const session of sessions) {
      // Get messages for this session
      const messages = await client.query(api.messages.listBySession, {
        sessionId: session._id,
        limit: 1000 // Get more messages to ensure we find them
      })

      // Check if any of our target messages are in this session
      const foundMessages = messages.filter((msg: any) => targetMessageIds.includes(msg.entry_uuid))

      if (foundMessages.length > 0) {
        console.log(`\n${"=".repeat(80)}`)
        console.log(`Found ${foundMessages.length} message(s) in session: ${session._id}`)
        console.log(`Session Title: ${session.title || "Untitled"}`)
        console.log(`${"=".repeat(80)}`)

        for (const msg of foundMessages) {
          console.log(`\n📄 Message Found:`)
          console.log(`- Entry UUID: ${msg.entry_uuid}`)
          console.log(`- Entry Type: ${msg.entry_type}`)
          console.log(`- Role: ${msg.role || "N/A"}`)
          console.log(`- Content Type: ${typeof msg.content}`)
          console.log(`- Content Length: ${msg.content ? msg.content.length : "null/undefined"}`)
          console.log(`- Has Tool Name: ${!!msg.tool_name}`)
          console.log(`- Has Tool Output: ${!!msg.tool_output}`)
          console.log(`- Timestamp: ${new Date(msg.timestamp).toISOString()}`)

          if (msg.content) {
            console.log(`\n📝 Content Preview (first 1000 chars):`)
            console.log(msg.content.substring(0, 1000))

            // Check content patterns
            console.log(`\n🔎 Content Analysis:`)
            console.log(`- Only whitespace: ${/^\s*$/.test(msg.content)}`)
            console.log(`- Contains tool_result: ${msg.content.includes("tool_result")}`)
            console.log(`- Contains thinking: ${msg.content.includes("thinking")}`)
            console.log(`- Starts with <: ${msg.content.startsWith("<")}`)
            console.log(`- Contains HTML entities: ${/&[a-z]+;/.test(msg.content)}`)
          } else {
            console.log(`\n⚠️  Content is null/undefined`)
          }

          if (msg.tool_name || msg.tool_output) {
            console.log(`\n🔧 Tool Information:`)
            console.log(`- Tool Name: ${msg.tool_name || "N/A"}`)
            console.log(`- Tool Use ID: ${msg.tool_use_id || "N/A"}`)
            console.log(`- Tool Output Length: ${msg.tool_output ? msg.tool_output.length : "N/A"}`)
            if (msg.tool_output) {
              console.log(`- Tool Output Preview (first 500 chars):`)
              console.log(msg.tool_output.substring(0, 500))
            }
          }

          // Save full message to file for inspection
          const filename = `message-${msg.entry_uuid}.json`
          console.log(`\n💾 Saving full message to: ${filename}`)
          await Bun.write(filename, JSON.stringify(msg, null, 2))
        }
      }
    }

    console.log("\n✅ Search complete")
  } catch (error) {
    console.error("❌ Error during search:", error)
  }
}

// Run the search
findMessagesInSessions().then(() => {
  process.exit(0)
}).catch((error) => {
  console.error("Fatal error:", error)
  process.exit(1)
})
