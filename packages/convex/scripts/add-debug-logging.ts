#!/usr/bin/env bun

/**
 * Script to add debug logging to track session and message loading
 */

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")

// Add a mutation to update session project info
// const updateSessionProjectInfo = async (sessionId: string, projectPath: string, projectName: string) => {
//   // Since we don't have a direct update mutation, we would need to add one
//   // For now, let's just document what needs to be done
//   console.log(`Would update session ${sessionId}:`)
//   console.log(`  project_path: ${projectPath}`)
//   console.log(`  project_name: ${projectName}`)
// }

async function analyzeSessionPaths() {
  console.log("=== Analyzing Session Project Paths ===\n")

  const sessions = await client.query(api.sessions.listRecent, { limit: 100 })

  // For each session, try to determine a better project name
  for (const session of sessions || []) {
    // Extract a better project name from the session ID or other metadata
    let betterProjectName = "Unknown Project"

    // If we had access to the original file paths, we could extract better names
    // For now, we can at least use the session ID prefix
    if (session.id) {
      // Use first 8 chars of session ID as a unique identifier
      const shortId = session.id.substring(0, 8)
      betterProjectName = `Project ${shortId}`
    }

    // Check if we have any messages to infer project context
    const messages = await client.query(api.messages.listBySession, {
      sessionId: session.id,
      limit: 5
    })

    if (messages && messages.length > 0) {
      console.log(`Session ${session.id}:`)
      console.log(`  Current project_name: ${session.project_name}`)
      console.log(`  Suggested name: ${betterProjectName}`)
      console.log(`  Has ${messages.length} messages (claimed: ${session.message_count})`)

      // Check first message for context
      const firstMessage = messages.find((m) => m.entry_type === "user" && m.content)
      if (firstMessage && firstMessage.content) {
        console.log(`  First user message: ${firstMessage.content.substring(0, 100)}...`)
      }
      console.log("")
    }
  }
}

async function debugMessageContent() {
  console.log("\n=== Debugging Empty Message Content ===\n")

  // Get a session with messages
  const testSessionId = "7644ecb5-b708-4418-9ed5-a08c87a1e72f"

  const messages = await client.query(api.messages.listBySession, {
    sessionId: testSessionId,
    limit: 20
  })

  console.log(`Found ${messages?.length || 0} messages for session ${testSessionId}`)

  // Analyze message types and content
  const stats = {
    total: messages?.length || 0,
    withContent: 0,
    emptyContent: 0,
    byType: new Map<string, number>()
  }

  messages?.forEach((msg) => {
    if (msg.content && msg.content.trim() !== "") {
      stats.withContent++
    } else {
      stats.emptyContent++
    }

    const count = stats.byType.get(msg.entry_type) || 0
    stats.byType.set(msg.entry_type, count + 1)
  })

  console.log("\nMessage Statistics:")
  console.log(`  Total messages: ${stats.total}`)
  console.log(`  With content: ${stats.withContent}`)
  console.log(`  Empty content: ${stats.emptyContent}`)
  console.log("\nBy type:")
  stats.byType.forEach((count, type) => {
    console.log(`  ${type}: ${count}`)
  })

  // Show a few examples
  console.log("\nSample messages:")
  messages?.slice(0, 5).forEach((msg, i) => {
    console.log(`\nMessage ${i + 1}:`)
    console.log(`  Type: ${msg.entry_type}`)
    console.log(`  Role: ${msg.role || "N/A"}`)
    console.log(`  Has content: ${!!msg.content}`)
    console.log(`  Has thinking: ${!!msg.thinking}`)
    console.log(`  Has summary: ${!!msg.summary}`)
    console.log(`  Timestamp: ${new Date(msg.timestamp).toISOString()}`)
    if (msg.content) {
      console.log(`  Content preview: ${msg.content.substring(0, 100)}...`)
    }
  })
}

// Main execution
async function main() {
  await analyzeSessionPaths()
  await debugMessageContent()

  console.log("\n=== Recommendations for Fixes ===\n")
  console.log("1. Add a Convex mutation to update session project info")
  console.log("2. Modify Overlord's extractProjectPath to handle actual Claude file paths")
  console.log("3. Debug why some messages have empty content - check JSONL parsing")
  console.log("4. Add a script to re-sync sessions with correct message counts")
  console.log("5. Consider adding a 'resync' feature to re-import sessions with better metadata")
}

main().catch(console.error)
