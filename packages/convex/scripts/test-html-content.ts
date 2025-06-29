import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function testHtmlContent() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get messages from the session showing HTML
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 10
  })

  // Find a user message with content
  const userMsg = messages.find((m: any) => m.entry_type === "user" && m.content)

  if (userMsg) {
    console.log("=== USER MESSAGE ===")
    console.log("Entry type:", userMsg.entry_type)
    console.log("Content type:", typeof userMsg.content)
    console.log("Content length:", userMsg.content.length)
    console.log("\nFirst 500 chars:")
    console.log(userMsg.content.substring(0, 500))
    console.log("\nContains HTML tags?", {
      hasSpan: userMsg.content.includes("<span"),
      hasDiv: userMsg.content.includes("<div"),
      hasArrow: userMsg.content.includes("→")
    })
  }

  // Find an assistant message
  const assistantMsg = messages.find((m: any) => m.entry_type === "assistant" && m.content)

  if (assistantMsg) {
    console.log("\n=== ASSISTANT MESSAGE ===")
    console.log("Entry type:", assistantMsg.entry_type)
    console.log("Content type:", typeof assistantMsg.content)
    console.log("Content length:", assistantMsg.content.length)
    console.log("\nFirst 500 chars:")
    console.log(assistantMsg.content.substring(0, 500))
  }
}

testHtmlContent().catch(console.error)
