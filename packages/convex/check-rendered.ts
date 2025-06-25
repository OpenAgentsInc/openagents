import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function checkRendered() {
  const client = new ConvexHttpClient(CONVEX_URL)
  
  // Get some messages to check their content
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 20
  })
  
  // Find a message with content that looks like code
  const codeMessage = messages.find((m: any) => 
    m.content && m.content.includes("→") && m.content.includes("span")
  )
  
  if (codeMessage) {
    console.log("=== MESSAGE WITH CODE ===")
    console.log("Entry type:", codeMessage.entry_type)
    console.log("Role:", codeMessage.role)
    console.log("\nContent preview:")
    console.log(codeMessage.content.substring(0, 500))
    console.log("\nContent looks like:", {
      isHTML: codeMessage.content.includes("<span"),
      hasLineNumbers: codeMessage.content.includes("→"),
      length: codeMessage.content.length
    })
  }
  
  // Also check what renderMarkdown is doing
  const userMessage = messages.find((m: any) => m.entry_type === "user" && m.content)
  if (userMessage) {
    console.log("\n=== USER MESSAGE ===")
    console.log("Content:", userMessage.content.substring(0, 200))
  }
}

checkRendered().catch(console.error)