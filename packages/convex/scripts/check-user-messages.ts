import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function checkUserMessages() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get messages from the session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 50
  })

  // Find all user messages
  const userMessages = messages.filter((m: any) => m.entry_type === "user")

  console.log(`Found ${userMessages.length} user messages`)

  for (let i = 0; i < Math.min(5, userMessages.length); i++) {
    const msg = userMessages[i]
    console.log(`\n=== User Message ${i + 1} ===`)
    console.log("ID:", msg._id)
    console.log("Entry UUID:", msg.entry_uuid)
    console.log("Has content:", !!msg.content)
    console.log("Content type:", typeof msg.content)
    console.log("Content length:", msg.content ? msg.content.length : 0)
    if (msg.content) {
      console.log("First 200 chars:", msg.content.substring(0, 200))
    } else {
      console.log("Content is:", msg.content)
    }
  }
}

checkUserMessages().catch(console.error)
