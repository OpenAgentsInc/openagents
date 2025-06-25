#!/usr/bin/env bun

/**
 * Delete ALL data from Convex database
 * WARNING: This will permanently delete all sessions and messages!
 */

import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"

const deleteAllData = Effect.gen(function*() {
  console.log("⚠️  WARNING: This will delete ALL data from Convex!")
  console.log("Proceeding in 3 seconds... Press Ctrl+C to cancel")
  
  await new Promise(resolve => setTimeout(resolve, 3000))
  
  console.log("\n🗑️  Starting deletion process...")
  
  // Get all sessions
  const sessions = yield* ConvexClient.sessions.listByUser("claude-code-user")
  console.log(`Found ${sessions.length} sessions to delete`)
  
  // Delete all messages for each session
  for (const session of sessions) {
    console.log(`\nDeleting messages for session: ${session.id}`)
    const messages = yield* ConvexClient.messages.listBySession(session.id, 1000)
    console.log(`  Found ${messages.length} messages`)
    
    // Note: We need to add delete functions to the Convex client
    // For now, we'll document what needs to be deleted
    console.log(`  Would delete ${messages.length} messages from session ${session.id}`)
  }
  
  console.log("\n❌ Delete functions not yet implemented in Convex")
  console.log("To delete all data, you need to:")
  console.log("1. Go to Convex dashboard: https://dashboard.convex.dev")
  console.log("2. Select your project")
  console.log("3. Go to Data tab")
  console.log("4. Delete all records from 'messages' table")
  console.log("5. Delete all records from 'sessions' table")
  
  return {
    sessionsFound: sessions.length,
    status: "Manual deletion required"
  }
})

Effect.runPromise(deleteAllData)
  .then(result => {
    console.log("\n✅ Data analysis complete:", result)
  })
  .catch(console.error)