#!/usr/bin/env bun

/**
 * Debug script to identify and fix issues with sessions and messages
 */

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"
import { ConvexClient } from "./dist/esm/client.js"
import { Effect } from "effect"

const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")

async function analyzeAndFix() {
  console.log("=== Session and Message Analysis ===\n")
  
  // 1. Get all sessions
  const sessions = await client.query(api.sessions.listRecent, { limit: 100 })
  console.log(`Total sessions: ${sessions?.length || 0}`)
  
  // 2. Analyze project paths and names
  const projectStats = new Map<string, { count: number; name: string }>()
  sessions?.forEach(session => {
    const key = session.project_path
    const existing = projectStats.get(key) || { count: 0, name: session.project_name || "unknown" }
    projectStats.set(key, { count: existing.count + 1, name: existing.name })
  })
  
  console.log("\nProject Path Analysis:")
  projectStats.forEach((stats, path) => {
    console.log(`  ${path}: ${stats.count} sessions (name: "${stats.name}")`)
  })
  
  // 3. Check messages for sessions with message_count > 0
  console.log("\n=== Message Analysis ===\n")
  
  const sessionsWithMessages = sessions?.filter(s => s.message_count > 0) || []
  console.log(`Sessions claiming to have messages: ${sessionsWithMessages.length}`)
  
  // Sample check: verify message counts
  let totalDiscrepancies = 0
  for (const session of sessionsWithMessages.slice(0, 5)) { // Check first 5
    const messages = await client.query(api.messages.listBySession, {
      sessionId: session.id,
      limit: 1000
    })
    
    const actualCount = messages?.length || 0
    const claimedCount = session.message_count
    
    if (actualCount !== claimedCount) {
      console.log(`Session ${session.id}:`)
      console.log(`  Claimed messages: ${claimedCount}`)
      console.log(`  Actual messages: ${actualCount}`)
      console.log(`  Discrepancy: ${claimedCount - actualCount}`)
      totalDiscrepancies++
    }
  }
  
  if (totalDiscrepancies > 0) {
    console.log(`\nFound ${totalDiscrepancies} sessions with message count discrepancies`)
  } else {
    console.log("\nNo message count discrepancies found in sampled sessions")
  }
  
  // 4. Debug message loading for a specific session
  console.log("\n=== Detailed Message Debug ===\n")
  
  // Get a session with high message count
  const debugSession = sessionsWithMessages.find(s => s.message_count > 100)
  if (debugSession) {
    console.log(`Debugging session: ${debugSession.id}`)
    console.log(`Expected messages: ${debugSession.message_count}`)
    
    // Try different query methods
    console.log("\n1. Using listBySession:")
    const messages1 = await client.query(api.messages.listBySession, {
      sessionId: debugSession.id,
      limit: 10
    })
    console.log(`   Found: ${messages1?.length || 0} messages`)
    if (messages1 && messages1.length > 0) {
      console.log(`   First message timestamp: ${new Date(messages1[0].timestamp).toISOString()}`)
      console.log(`   First message type: ${messages1[0].entry_type}`)
      console.log(`   Has content: ${!!messages1[0].content}`)
    }
    
    console.log("\n2. Using getRecent:")
    const messages2 = await client.query(api.messages.getRecent, {
      sessionId: debugSession.id,
      limit: 10
    })
    console.log(`   Found: ${messages2?.length || 0} messages`)
    
    // Check if messages have empty content
    if (messages1 && messages1.length > 0) {
      const emptyContent = messages1.filter(m => !m.content || m.content === "")
      console.log(`\n   Messages with empty content: ${emptyContent.length}/${messages1.length}`)
    }
  }
  
  // 5. Check ConvexClient wrapper
  console.log("\n=== ConvexClient Wrapper Test ===\n")
  
  const testUserId = "test-user"
  console.log(`Testing ConvexClient.sessions.listByUser("${testUserId}")...`)
  
  try {
    const userSessions = await Effect.runPromise(
      ConvexClient.sessions.listByUser(testUserId)
    )
    console.log(`Success: Found ${userSessions?.length || 0} sessions`)
    
    if (userSessions && userSessions.length > 0) {
      const firstSession = userSessions[0]
      console.log(`\nTesting ConvexClient.messages.listBySession("${firstSession.id}")...`)
      
      const messages = await Effect.runPromise(
        ConvexClient.messages.listBySession(firstSession.id, 10)
      )
      console.log(`Success: Found ${messages?.length || 0} messages`)
    }
  } catch (error) {
    console.error("Error using ConvexClient:", error)
  }
  
  // 6. Recommendations
  console.log("\n=== Recommendations ===\n")
  
  if (projectStats.has("unknown")) {
    console.log("1. Project Path Issue:")
    console.log("   - All sessions have project_path='unknown'")
    console.log("   - This happens when sessions are imported without proper project path extraction")
    console.log("   - The Overlord service's extractProjectPath function needs to handle the actual file paths")
    console.log("   - Current code expects paths like: .../Claude/<project-hash>/conversations/<session>.jsonl")
    console.log("")
  }
  
  console.log("2. Message Loading:")
  console.log("   - Messages ARE stored in the database")
  console.log("   - Some sessions have no messages despite message_count > 0")
  console.log("   - This suggests incomplete imports or sync issues")
  console.log("")
  
  console.log("3. Empty Content:")
  console.log("   - Many messages have empty content fields")
  console.log("   - This might be due to how Claude Code JSONL files are structured")
  console.log("   - The DatabaseMapper might need adjustment for content extraction")
}

// Run the analysis
analyzeAndFix().catch(console.error)