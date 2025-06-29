import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function findEscapedHtml() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get ALL messages from the session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 1000
  })

  console.log(`Checking ${messages.length} messages...`)

  // Find messages with escaped HTML or problematic content
  for (const msg of messages) {
    if (msg.content && msg.content.length > 1000) {
      // Check for patterns that indicate escaped HTML or code
      const hasEscapedNewlines = msg.content.includes("\\n")
      const hasConst = msg.content.includes("const")
      const hasRegex = msg.content.includes("regex")
      const hasRoute = msg.content.includes("routeMatch")

      if (hasEscapedNewlines && hasConst && (hasRegex || hasRoute)) {
        console.log("\n=== FOUND LARGE MESSAGE WITH CODE ===")
        console.log("ID:", msg._id)
        console.log("Entry type:", msg.entry_type)
        console.log("Content length:", msg.content.length)
        console.log("Has escaped newlines:", hasEscapedNewlines)
        console.log("Preview:", msg.content.substring(0, 200).replace(/\\n/g, "\n"))

        // Clear this message's content
        await client.mutation(api.messages.update, {
          id: msg._id,
          updates: { content: "[Content removed - was causing rendering issues]" }
        })
        console.log("✅ Cleared problematic content!")
      }
    }
  }

  console.log("\nDone checking messages.")
}

findEscapedHtml().catch(console.error)
