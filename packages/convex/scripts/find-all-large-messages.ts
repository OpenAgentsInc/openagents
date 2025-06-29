import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function findAllLargeMessages() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get ALL messages from the session - with higher limit
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 2000
  })

  console.log(`Total messages: ${messages.length}`)

  // Find all large messages
  const largeMessages = messages.filter((msg) => msg.content && msg.content.length > 500)

  console.log(`Found ${largeMessages.length} messages with content > 500 chars`)

  for (const msg of largeMessages) {
    console.log(`\n=== Message ${msg.entry_uuid} ===`)
    console.log(`Type: ${msg.entry_type}`)
    console.log(`Length: ${msg.content.length}`)
    console.log(`First 100 chars: ${msg.content.substring(0, 100)}`)

    // Look for the specific problematic pattern from the screenshot
    if (
      msg.content.includes("routeMatch") ||
      msg.content.includes("pathname.match(regex)") ||
      msg.content.includes("const regex = new RegExp")
    ) {
      console.log("\n🚨 FOUND THE PROBLEMATIC MESSAGE!")
      console.log("Clearing this message...")

      await client.mutation(api.messages.update, {
        id: msg._id,
        updates: { content: "" }
      })
      console.log("✅ Cleared!")
    }
  }
}

findAllLargeMessages().catch(console.error)
