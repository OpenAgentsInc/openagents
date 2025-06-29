import { ConvexHttpClient } from "convex/browser"
import fs from "fs"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function find405Message() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get messages from the session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 1000
  })

  console.log(`Searching through ${messages.length} messages...`)

  // Find the message containing the specific text
  const searchText = "405→            context.params[param] = routeMatch[index + 1]"

  messages.forEach((msg, index) => {
    // Check all text fields
    const fields = [
      { name: "content", value: msg.content },
      { name: "tool_output", value: msg.tool_output },
      { name: "thinking", value: msg.thinking },
      { name: "summary", value: msg.summary }
    ]

    fields.forEach((field) => {
      if (field.value && field.value.includes(searchText)) {
        console.log(`\n🎯 FOUND IT! Message ${index + 1} (index ${index})`)
        console.log(`Field: ${field.name}`)
        console.log(`Entry type: ${msg.entry_type}`)
        console.log(`Entry UUID: ${msg.entry_uuid}`)
        console.log(`Timestamp: ${new Date(msg.timestamp).toISOString()}`)
        console.log(`\nFull ${field.name} content (${field.value.length} chars):`)
        console.log("---START---")
        console.log(field.value)
        console.log("---END---")

        // Save to file for easier inspection
        fs.writeFileSync("message-405.txt", field.value)
        console.log("\nSaved to message-405.txt")
      }
    })
  })
}

find405Message().catch(console.error)
