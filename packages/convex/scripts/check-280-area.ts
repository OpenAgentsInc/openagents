import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function check280Area() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get messages from the session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 1000
  })

  // Check messages 275-285
  for (let i = 275; i < 285 && i < messages.length; i++) {
    const msg = messages[i]
    console.log(`\n=== Message ${i + 1} ===`)
    console.log(`Type: ${msg.entry_type}`)
    console.log(`UUID: ${msg.entry_uuid}`)

    // Check all text fields for backticks
    const fields = {
      content: msg.content,
      tool_output: msg.tool_output,
      thinking: msg.thinking,
      summary: msg.summary
    }

    Object.entries(fields).forEach(([fieldName, value]) => {
      if (value) {
        const backtickCount = (value.match(/`/g) || []).length
        if (backtickCount > 0) {
          console.log(`\n${fieldName} contains ${backtickCount} backticks`)
          // Find triple backticks
          if (value.includes("```")) {
            console.log(`  WARNING: Contains triple backticks!`)
            const idx = value.indexOf("```")
            console.log(`  Preview: ...${value.substring(Math.max(0, idx - 30), idx + 50)}...`)
          }
        }
      }
    })
  }
}

check280Area().catch(console.error)
