import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const CONVEX_URL = "https://proficient-panther-764.convex.cloud"

async function findChatSession() {
  const client = new ConvexHttpClient(CONVEX_URL)

  // Get all sessions
  const sessions = await client.query(api.sessions.listByUser, {
    userId: "claude-code-user",
    limit: 20
  })

  console.log("=== ALL SESSIONS ===")
  for (const session of sessions) {
    console.log(`\nSession: ${session._id}`)
    console.log(`Project: ${session.project_path}`)
    console.log(`Message count: ${session.message_count}`)

    // Get first few messages to see content type
    const messages = await client.query(api.messages.listBySession, {
      sessionId: session._id,
      limit: 3
    })

    const firstUserMsg = messages.find((m: any) => m.entry_type === "user")
    if (firstUserMsg) {
      console.log(`First user message: ${firstUserMsg.content.substring(0, 100)}...`)
    }
  }
}

findChatSession().catch(console.error)
