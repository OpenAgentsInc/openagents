import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function showMessages() {
  const client = new ConvexHttpClient(CONVEX_URL)
  
  // Get messages
  const messages = await client.query(api.messages.listBySession, {
    sessionId: "claude-code-session-1750816776552",
    limit: 10
  })
  
  console.log(`Found ${messages.length} messages\n`)
  
  messages.forEach((msg: any, index: number) => {
    console.log(`\n--- Message ${index + 1} ---`)
    console.log(`Type: ${msg.entry_type}`)
    console.log(`Role: ${msg.role || 'N/A'}`)
    
    if (msg.content) {
      console.log(`Content: ${msg.content.substring(0, 200)}${msg.content.length > 200 ? '...' : ''}`)
    } else {
      console.log(`Content: [empty]`)
    }
    
    if (msg.thinking) {
      console.log(`Thinking: ${msg.thinking.substring(0, 100)}...`)
    }
    
    if (msg.tool_name) {
      console.log(`Tool: ${msg.tool_name}`)
    }
    
    if (msg.summary) {
      console.log(`Summary: ${msg.summary}`)
    }
  })
}

showMessages().catch(console.error)