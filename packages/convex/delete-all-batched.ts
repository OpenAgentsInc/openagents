#!/usr/bin/env bun

/**
 * Delete all data in batches to avoid Convex limits
 */

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")

async function deleteAllInBatches() {
  console.log("🗑️  Deleting all data from Convex in batches...")
  
  let totalMessagesDeleted = 0
  let totalSessionsDeleted = 0
  
  // Delete messages first
  console.log("\nDeleting messages...")
  while (true) {
    const result = await client.mutation(api.deleteBatch.deleteMessageBatch, {})
    totalMessagesDeleted += result.deleted
    console.log(`  Deleted ${result.deleted} messages (total: ${totalMessagesDeleted})`)
    
    if (!result.hasMore) break
    await new Promise(resolve => setTimeout(resolve, 100)) // Small delay
  }
  
  // Delete sessions
  console.log("\nDeleting sessions...")
  while (true) {
    const result = await client.mutation(api.deleteBatch.deleteSessionBatch, {})
    totalSessionsDeleted += result.deleted
    console.log(`  Deleted ${result.deleted} sessions (total: ${totalSessionsDeleted})`)
    
    if (!result.hasMore) break
    await new Promise(resolve => setTimeout(resolve, 100)) // Small delay
  }
  
  console.log(`\n✅ Complete! Deleted ${totalMessagesDeleted} messages and ${totalSessionsDeleted} sessions`)
  
  console.log("\n📥 Now run this to import ONE session:")
  console.log("cd ../overlord && bun run src/index.ts import --user-id=\"claude-code-user\" --api-key=\"test\" --limit=1")
}

deleteAllInBatches().catch(console.error)