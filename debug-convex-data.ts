import { client } from "@openagentsinc/convex"
import { Effect } from "effect"

const main = Effect.gen(function*() {
  console.log("Fetching session data...")
  
  // Get sessions
  const sessions = yield* client.ConvexClient.sessions.listByUser("claude-code-user")
  console.log("Sessions found:", sessions.length)
  
  if (sessions.length > 0) {
    const session = sessions[0]
    console.log("First session:", {
      id: session.id,
      project_name: session.project_name,
      project_path: session.project_path,
      message_count: session.message_count
    })
    
    // Get messages for this session
    const messages = yield* client.ConvexClient.messages.listBySession(session.id, 5)
    console.log("First 5 messages:")
    
    messages.forEach((msg, i) => {
      console.log(`Message ${i}:`, {
        entry_type: msg.entry_type,
        role: msg.role,
        content: msg.content?.substring(0, 100) + "...",
        contentType: typeof msg.content,
        hasContent: !!msg.content
      })
    })
  }
})

Effect.runPromise(main).catch(console.error)