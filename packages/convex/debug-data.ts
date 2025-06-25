import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"

const main = Effect.gen(function*() {
  console.log("Checking if specific session exists...")
  
  // Check if this specific session exists
  const testSessionId = "34edb32a-d589-4ece-b9b0-91e5c4b0f718"
  const allSessions = yield* ConvexClient.sessions.listByUser("claude-code-test-new")
  const existingSession = allSessions.find(s => s.id === testSessionId)
  
  if (existingSession) {
    console.log(`Session ${testSessionId} already exists for claude-code-test-new`)
  } else {
    console.log(`Session ${testSessionId} does NOT exist for claude-code-test-new - good for testing`)
  }
  
  // Also check what sessions exist for claude-code-test-new
  console.log(`Total sessions for claude-code-test-new: ${allSessions.length}`)
  allSessions.forEach(session => {
    console.log(`- ${session.id}: ${session.project_name}`)
  })
})

Effect.runPromise(main).catch(console.error)