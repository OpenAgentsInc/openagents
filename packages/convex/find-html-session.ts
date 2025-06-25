import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function findHtmlSession() {
  const client = new ConvexHttpClient(CONVEX_URL)
  
  // Get all sessions
  const sessions = await client.query(api.sessions.listByUser, {
    userId: "claude-code-user",
    limit: 10
  })
  
  console.log("Looking for sessions with HTML content...")
  
  for (const session of sessions) {
    // Get messages from this session
    const messages = await client.query(api.messages.listBySession, {
      sessionId: session._id,
      limit: 5
    })
    
    // Check if any message has HTML-like content
    const hasHtml = messages.some((m: any) => {
      if (!m.content) return false
      return m.content.includes('<span') || 
             m.content.includes('<div') || 
             m.content.includes('→') ||
             m.content.includes('class=')
    })
    
    if (hasHtml) {
      console.log(`\nFound session with HTML: ${session._id}`)
      console.log(`Project: ${session.project_path}`)
      
      // Show first message with HTML
      const htmlMsg = messages.find((m: any) => 
        m.content && (m.content.includes('<span') || m.content.includes('→'))
      )
      
      if (htmlMsg) {
        console.log(`Message type: ${htmlMsg.entry_type}`)
        console.log(`First 300 chars: ${htmlMsg.content.substring(0, 300)}...`)
      }
    }
  }
}

findHtmlSession().catch(console.error)