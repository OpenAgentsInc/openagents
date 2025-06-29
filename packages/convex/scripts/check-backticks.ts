import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function checkBackticks() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get messages from the session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 1000
  })

  console.log(`Total messages: ${messages.length}`)

  // Check each message for backticks
  messages.forEach((msg, index) => {
    const hasBackticks = [
      msg.content,
      msg.tool_output,
      msg.thinking,
      msg.summary
    ].some((field) => field && field.includes("`"))

    if (hasBackticks) {
      console.log(`\nMessage ${index + 1} (${msg.entry_type}) contains backticks:`)
      if (msg.content?.includes("`")) {
        console.log("  - In content field")
        // Show first occurrence
        const idx = msg.content.indexOf("`")
        console.log(`    Preview: ...${msg.content.substring(Math.max(0, idx - 20), idx + 20)}...`)
      }
      if (msg.tool_output?.includes("`")) {
        console.log("  - In tool_output field")
        const idx = msg.tool_output.indexOf("`")
        console.log(`    Preview: ...${msg.tool_output.substring(Math.max(0, idx - 20), idx + 20)}...`)
      }
      if (msg.thinking?.includes("`")) {
        console.log("  - In thinking field")
      }
      if (msg.summary?.includes("`")) {
        console.log("  - In summary field")
      }
    }
  })
}

checkBackticks().catch(console.error)
