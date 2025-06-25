#!/usr/bin/env bun

/**
 * Delete all Convex data and re-import with improved parser
 * 
 * This script:
 * 1. Deletes all sessions and messages from Convex
 * 2. Uses the improved JSONL parser to extract ALL content
 * 3. Re-imports everything with comprehensive logging
 * 4. Verifies the import was successful
 */

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api"
import { parseJSONL, extractSessionMetadata, convertToConvexMessages } from "../overlord/src/services/JSONLParser"
import { Effect, pipe } from "effect"
import * as fs from "fs"
import * as path from "path"

// Configuration
const CONVEX_URL = process.env.CONVEX_URL || "https://proficient-panther-764.convex.cloud"
const JSONL_FILE_PATH = "/Users/christopherdavid/code/openagents/docs/ingest/claude-code-example.jsonl"

async function main() {
  console.log("🚀 Starting delete and re-import process...")
  
  // Initialize Convex client
  const client = new ConvexHttpClient(CONVEX_URL)
  
  try {
    // Step 1: Delete all existing data
    console.log("\n🗑️ Deleting all existing data...")
    
    const sessionDeleteResult = await client.mutation(api.sessions.deleteAll, {})
    console.log(`✓ Deleted sessions: ${sessionDeleteResult.deletedCount}`)
    
    const messageDeleteResult = await client.mutation(api.messages.deleteAll, {})
    console.log(`✓ Deleted messages: ${messageDeleteResult.deletedCount}`)
    
    // Step 2: Read and parse JSONL file
    console.log("\n📖 Reading and parsing JSONL file...")
    
    if (!fs.existsSync(JSONL_FILE_PATH)) {
      throw new Error(`JSONL file not found: ${JSONL_FILE_PATH}`)
    }
    
    const fileContent = fs.readFileSync(JSONL_FILE_PATH, 'utf-8')
    console.log(`📄 File size: ${(fileContent.length / 1024 / 1024).toFixed(2)} MB`)
    
    // Parse with Effect-based parser
    const parseEffect = parseJSONL(fileContent)
    
    const entries = await Effect.runPromise(parseEffect)
    
    console.log(`\n✅ Parsed ${entries.length} entries successfully`)
    
    // Step 3: Extract metadata and create session
    console.log("\n📊 Extracting session metadata...")
    
    const metadata = extractSessionMetadata(entries)
    console.log("Session metadata:", {
      ...metadata,
      models: metadata.models.slice(0, 3), // Truncate for display
      sessionIds: metadata.sessionIds.slice(0, 3) // Truncate for display
    })
    
    // Create session
    const sessionId = `claude-code-session-${Date.now()}`
    const projectPath = "/Users/christopherdavid/code/openagents"
    const projectName = "OpenAgents Effect Monorepo"
    
    console.log(`\n💾 Creating session: ${sessionId}`)
    
    await client.mutation(api.sessions.create, {
      id: sessionId,
      user_id: "claude-code-user",
      project_path: projectPath,
      project_name: projectName,
      status: "completed",
      started_at: metadata.firstMessage ? new Date(metadata.firstMessage).getTime() : Date.now(),
      last_activity: metadata.lastMessage ? new Date(metadata.lastMessage).getTime() : Date.now(),
      message_count: metadata.messageCount,
      total_cost: metadata.totalCost
    })
    
    console.log("✓ Session created successfully")
    
    // Step 4: Convert and import messages
    console.log("\n📝 Converting entries to Convex format...")
    
    const convexMessages = convertToConvexMessages(entries, sessionId)
    console.log(`📦 Generated ${convexMessages.length} Convex messages`)
    
    // Import messages in batches to avoid overwhelming Convex
    const batchSize = 50
    let importedCount = 0
    
    console.log(`\n⬆️ Importing ${convexMessages.length} messages in batches of ${batchSize}...`)
    
    for (let i = 0; i < convexMessages.length; i += batchSize) {
      const batch = convexMessages.slice(i, i + batchSize)
      
      // Import batch
      for (const message of batch) {
        try {
          await client.mutation(api.messages.create, message)
          importedCount++
        } catch (error) {
          console.error(`❌ Failed to import message ${message.entry_uuid}:`, error)
        }
      }
      
      console.log(`📈 Progress: ${importedCount}/${convexMessages.length} messages imported`)
      
      // Small delay to be nice to Convex
      await new Promise(resolve => setTimeout(resolve, 100))
    }
    
    console.log(`\n✅ Import completed! ${importedCount} messages imported successfully`)
    
    // Step 5: Verify import
    console.log("\n🔍 Verifying import...")
    
    const importedSession = await client.query(api.sessions.getById, { sessionId })
    if (!importedSession) {
      throw new Error("Session not found after import")
    }
    
    const importedMessages = await client.query(api.messages.listBySession, { 
      sessionId, 
      limit: 1000 
    })
    
    console.log("✓ Verification results:")
    console.log(`  - Session exists: ${!!importedSession}`)
    console.log(`  - Messages imported: ${importedMessages.length}`)
    console.log(`  - Expected messages: ${convexMessages.length}`)
    
    // Message type breakdown
    const messageTypes = importedMessages.reduce((acc, msg) => {
      acc[msg.entry_type] = (acc[msg.entry_type] || 0) + 1
      return acc
    }, {} as Record<string, number>)
    
    console.log("  - Message types:", messageTypes)
    
    // Check for thinking content
    const messagesWithThinking = importedMessages.filter(msg => msg.thinking && msg.thinking.length > 0)
    console.log(`  - Messages with thinking: ${messagesWithThinking.length}`)
    
    // Check for tool usage
    const toolUseMessages = importedMessages.filter(msg => msg.entry_type === "tool_use")
    const toolResultMessages = importedMessages.filter(msg => msg.entry_type === "tool_result")
    console.log(`  - Tool uses: ${toolUseMessages.length}`)
    console.log(`  - Tool results: ${toolResultMessages.length}`)
    
    console.log("\n🎉 Delete and re-import process completed successfully!")
    console.log(`\n📊 Final statistics:`)
    console.log(`   - Session ID: ${sessionId}`)
    console.log(`   - Total entries parsed: ${entries.length}`)
    console.log(`   - Total messages imported: ${importedCount}`)
    console.log(`   - Token usage: ${metadata.totalTokens.toLocaleString()}`)
    console.log(`   - Estimated cost: $${metadata.totalCost.toFixed(4)}`)
    console.log(`   - Models used: ${metadata.models.join(", ")}`)
    console.log(`   - Thinking characters: ${metadata.totalThinkingChars.toLocaleString()}`)
    
  } catch (error) {
    console.error("\n❌ Error during delete and re-import:", error)
    process.exit(1)
  }
}

// Run the script
main().catch(console.error)