import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function checkSpecific() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get messages that might contain the problematic code
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 2000
  })

  console.log(`Checking ${messages.length} messages...`)

  // Look for ANY message that might contain escaped code
  let foundCount = 0
  for (const msg of messages) {
    if (msg.content && msg.content.includes("\\n")) {
      foundCount++
      console.log(`\nFound message with escaped newlines #${foundCount}`)
      console.log("ID:", msg._id)
      console.log("Type:", msg.entry_type)
      console.log("Length:", msg.content.length)
      console.log("Preview:", msg.content.substring(0, 150))

      // Clear ALL messages with escaped newlines
      await client.mutation(api.messages.update, {
        id: msg._id,
        updates: { content: "" }
      })
      console.log("Cleared this message!")
    }
  }

  console.log(`\nTotal messages with escaped newlines cleared: ${foundCount}`)
}

checkSpecific().catch(console.error)
