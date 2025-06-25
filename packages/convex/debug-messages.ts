import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function debugMessages() {
  const client = new ConvexHttpClient(CONVEX_URL)
  
  // Get specific types of messages
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776551",  // Different session
    limit: 50
  })
  
  // Find messages with different characteristics
  const assistantWithContent = messages.find((m: any) => m.entry_type === "assistant" && m.content)
  const assistantWithThinking = messages.find((m: any) => m.entry_type === "assistant" && m.thinking && !m.content)
  const toolUse = messages.find((m: any) => m.entry_type === "tool_use")
  const toolResult = messages.find((m: any) => m.entry_type === "tool_result")
  
  console.log("=== ASSISTANT WITH CONTENT ===")
  if (assistantWithContent) {
    console.log("Content:", assistantWithContent.content)
    console.log("Content length:", assistantWithContent.content?.length)
  }
  
  console.log("\n=== ASSISTANT WITH THINKING ===")
  if (assistantWithThinking) {
    console.log("Thinking:", assistantWithThinking.thinking?.substring(0, 200) + "...")
    console.log("Content:", assistantWithThinking.content || "[EMPTY]")
  }
  
  console.log("\n=== TOOL USE ===")
  if (toolUse) {
    console.log("Tool name:", toolUse.tool_name)
    console.log("Tool input:", JSON.stringify(toolUse.tool_input, null, 2))
    console.log("Content:", toolUse.content || "[EMPTY]")
  }
  
  console.log("\n=== TOOL RESULT ===")
  if (toolResult) {
    console.log("Tool output:", toolResult.tool_output?.substring(0, 200) + "...")
    console.log("Content:", toolResult.content || "[EMPTY]")
  }
}

debugMessages().catch(console.error)