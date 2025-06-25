import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"

const main = Effect.gen(function*() {
  console.log("Checking latest sessions and user messages...")
  
  // Check the most recent sessions
  const allSessions = yield* ConvexClient.sessions.listByUser("claude-code-user")
  console.log(`Total sessions: ${allSessions.length}`)
  
  // Check the newest session that has been updated with the user content fix
  const recentSession = allSessions[0] // Most recent
  console.log(`\n--- Checking most recent session ${recentSession.id} ---`)
  console.log("Session metadata:", {
    project_path: recentSession.project_path,
    project_name: recentSession.project_name,
    message_count: recentSession.message_count
  })
  
  // Check the first few messages, focusing on user messages
  const messages = yield* ConvexClient.messages.listBySession(recentSession.id, 10)
  console.log("Checking user messages for content:")
  
  messages.forEach((msg, i) => {
    if (msg.role === "user") {
      console.log(`User message ${i}:`, {
        entry_type: msg.entry_type,
        content: msg.content?.substring(0, 150) + (msg.content && msg.content.length > 150 ? "..." : ""),
        contentLength: msg.content?.length || 0,
        hasContent: !!msg.content
      })
    }
  })
  
  // Also check wanix sessions to see if they have user content now
  const wanixSessions = allSessions.filter(s => s.project_path === "wanix")
  if (wanixSessions.length > 0) {
    console.log(`\n--- Checking wanix session ${wanixSessions[0].id} ---`)
    const wanixMessages = yield* ConvexClient.messages.listBySession(wanixSessions[0].id, 5)
    const userMessages = wanixMessages.filter(m => m.role === "user")
    console.log("Wanix user messages:")
    userMessages.forEach((msg, i) => {
      console.log(`User ${i}:`, {
        content: msg.content?.substring(0, 100) + (msg.content && msg.content.length > 100 ? "..." : ""),
        hasContent: !!msg.content
      })
    })
  }
})

Effect.runPromise(main).catch(console.error)