import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function verifyFix() {
  const client = new ConvexHttpClient(CONVEX_URL)
  
  // Get the problematic session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 20
  })
  
  console.log("=== CHECKING MESSAGE CONTENT ===")
  
  for (const msg of messages) {
    console.log(`\n${msg.entry_type} message:`)
    if (msg.content) {
      // Check what the content looks like
      const preview = msg.content.substring(0, 200)
      console.log("Content preview:", preview)
      console.log("Contains HTML?", msg.content.includes('<'))
      console.log("Contains line numbers?", msg.content.includes('→'))
    } else {
      console.log("No content")
    }
  }
}

verifyFix().catch(console.error)