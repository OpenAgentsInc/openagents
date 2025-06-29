import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function findBrokenMessage() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get ALL messages from the session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 1000
  })

  console.log(`Total messages: ${messages.length}`)

  // Find messages with problematic content
  for (const msg of messages) {
    if (msg.content) {
      // Check for escaped HTML entities that might break rendering
      if (msg.content.includes("\\n") && msg.content.includes("const") && msg.content.includes("regex")) {
        console.log("\n=== FOUND PROBLEMATIC MESSAGE ===")
        console.log("ID:", msg._id)
        console.log("Entry UUID:", msg.entry_uuid)
        console.log("Entry type:", msg.entry_type)
        console.log("Content length:", msg.content.length)
        console.log("First 300 chars:", msg.content.substring(0, 300))
        console.log("Contains backslash-n:", msg.content.includes("\\n"))
        console.log("Contains HTML entities:", msg.content.includes("&lt;") || msg.content.includes("&gt;"))

        // Update this message to have empty content
        await client.mutation(api.messages.update, {
          id: msg._id,
          updates: { content: "" }
        })
        console.log("CLEARED problematic message content!")
      }
    }
  }
}

findBrokenMessage().catch(console.error)
