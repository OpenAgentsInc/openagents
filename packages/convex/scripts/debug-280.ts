import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function debug280() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get messages from the session
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 1000
  })

  // Get message 280 (index 279)
  const msg280 = messages[279]

  if (!msg280) {
    console.log("Message 280 not found!")
    return
  }

  console.log("\n=== MESSAGE 280 FULL DEBUG ===")
  console.log("Database ID:", msg280._id)
  console.log("Entry UUID:", msg280.entry_uuid)
  console.log("Entry type:", msg280.entry_type)
  console.log("Role:", msg280.role)
  console.log("\nContent analysis:")
  console.log("- Type:", typeof msg280.content)
  console.log("- Length:", msg280.content ? msg280.content.length : 0)
  console.log("- Is null:", msg280.content === null)
  console.log("- Is undefined:", msg280.content === undefined)
  console.log("- Is empty string:", msg280.content === "")

  if (msg280.content) {
    // Check for problematic characters
    console.log("\nCharacter analysis:")
    console.log("- Contains newlines:", msg280.content.includes("\n"))
    console.log("- Contains escaped newlines:", msg280.content.includes("\\n"))
    console.log("- Contains tabs:", msg280.content.includes("\t"))
    console.log("- Contains escaped tabs:", msg280.content.includes("\\t"))
    console.log("- Contains <:", msg280.content.includes("<"))
    console.log("- Contains >:", msg280.content.includes(">"))
    console.log("- Contains &:", msg280.content.includes("&"))
    console.log("- Contains quotes:", msg280.content.includes("\""))
    console.log("- Contains backticks:", msg280.content.includes("`"))

    // Show first and last parts
    console.log("\nContent preview:")
    console.log("First 100 chars:", JSON.stringify(msg280.content.substring(0, 100)))
    console.log("Last 100 chars:", JSON.stringify(msg280.content.substring(msg280.content.length - 100)))

    // Check if it's the problematic tool result
    if (msg280.content.includes("Applied 3 edits")) {
      console.log("\n⚠️  THIS IS THE PROBLEMATIC TOOL RESULT MESSAGE!")

      // Extract the actual content
      const lines = msg280.content.split("\n")
      console.log("Number of lines:", lines.length)
      console.log("Line lengths:", lines.map((l) => l.length))

      // Show each line
      lines.forEach((line, i) => {
        console.log(`Line ${i + 1} (${line.length} chars):`, JSON.stringify(line.substring(0, 80)))
      })
    }
  }

  // Show all fields
  console.log("\nAll message fields:")
  Object.keys(msg280).forEach((key) => {
    const value = msg280[key]
    if (typeof value === "string" && value.length > 100) {
      console.log(`${key}: [string, ${value.length} chars]`)
    } else {
      console.log(`${key}:`, value)
    }
  })

  // Check tool_output specifically
  if (msg280.tool_output) {
    console.log("\n=== TOOL OUTPUT CONTENT ===")
    console.log("Length:", msg280.tool_output.length)
    console.log("Full content:")
    console.log(msg280.tool_output)
  }
}

debug280().catch(console.error)
