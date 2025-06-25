#!/usr/bin/env bun

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

// Fix user messages that have empty content
async function fixUserMessages() {
  console.log("Fixing user messages with empty content...")
  
  const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")
  
  try {
    // Get all sessions
    const sessions = await client.query(api.sessions.listByUser, { userId: "test-user" })
    console.log(`Found ${sessions.length} sessions`)
    
    let totalFixed = 0
    
    for (const session of sessions) {
      console.log(`\nChecking session: ${session.id} (${session.project_name || session.project_path})`)
      
      // Get all messages for this session
      const messages = await client.query(api.messages.listBySession, {
        sessionId: session.id,
        limit: 1000
      })
      
      console.log(`  Found ${messages.length} messages`)
      
      // Check for user messages with empty content
      const emptyUserMessages = messages.filter(
        msg => msg.entry_type === "user" && (!msg.content || msg.content === "")
      )
      
      if (emptyUserMessages.length > 0) {
        console.log(`  Found ${emptyUserMessages.length} user messages with empty content`)
        
        // For now, just log them - we would need the original JSONL to fix them
        for (const msg of emptyUserMessages.slice(0, 3)) {
          console.log(`    - ${msg.entry_uuid} at ${new Date(msg.timestamp).toISOString()}`)
        }
        
        totalFixed += emptyUserMessages.length
      }
    }
    
    console.log(`\nTotal user messages with empty content: ${totalFixed}`)
    console.log("\nTo fix these, we need the original JSONL files from Claude Code")
    
  } catch (error) {
    console.error("Error:", error)
  }
}

// Run the fix
fixUserMessages()