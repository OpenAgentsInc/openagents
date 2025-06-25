#!/usr/bin/env bun

/**
 * Delete all Convex data and re-import one session
 */

import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"
import { exec } from "child_process"
import { promisify } from "util"

const execAsync = promisify(exec)

const main = Effect.gen(function*() {
  console.log("🗑️  Step 1: Deleting all data from Convex...")
  
  try {
    const result = yield* ConvexClient.admin.deleteEverything()
    console.log(`✅ Deleted ${result.messagesDeleted} messages and ${result.sessionsDeleted} sessions`)
  } catch (error) {
    console.error("❌ Failed to delete data:", error)
    console.log("Continuing anyway...")
  }
  
  console.log("\n📥 Step 2: Re-importing ONE session...")
  
  // Import just one session using Overlord
  const importResult = yield* Effect.tryPromise({
    try: () => execAsync(
      `cd ../overlord && bun run src/index.ts import --user-id="claude-code-user" --api-key="test" --limit=1`,
      { maxBuffer: 1024 * 1024 * 10 } // 10MB buffer
    ),
    catch: (error) => new Error(`Import failed: ${error}`)
  })
  
  if (importResult.stderr) {
    console.error("Stderr:", importResult.stderr)
  }
  
  console.log(importResult.stdout)
  
  console.log("\n✅ Complete! Database now contains only 1 session.")
  
  // Verify what we have
  console.log("\n📊 Verifying data...")
  const sessions = yield* ConvexClient.sessions.listByUser("claude-code-user")
  console.log(`Found ${sessions.length} session(s) in database`)
  
  if (sessions.length > 0) {
    const session = sessions[0]
    console.log(`\nSession ID: ${session.id}`)
    console.log(`Project: ${session.project_name || session.project_path}`)
    console.log(`Messages: ${session.message_count}`)
    console.log(`\n🔗 View at: http://localhost:3003/chat/${session.id}`)
  }
})

Effect.runPromise(main).catch(console.error)