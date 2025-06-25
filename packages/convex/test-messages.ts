#!/usr/bin/env bun

// Use the built ConvexClient from our package
import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

// Test queries
async function testMessageQueries() {
  console.log("Testing Convex message queries...")
  
  const directClient = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")
  
  try {
    // Query a raw list of all messages (we need to create a temporary query function)
    console.log("\n1. Getting sample of messages from database...")
    
    // Try to query messages for specific session IDs we know have messages
    const sessionIds = [
      "f3ba5d5f-cd46-4fd1-af16-dcd6d386c126", // message_count: 2
      "7644ecb5-b708-4418-9ed5-a08c87a1e72f", // message_count: 1023
      "db8779f5-9388-4aac-99da-46c32b591fcb", // message_count: 1994
    ]
    
    for (const sessionId of sessionIds) {
      console.log(`\n2. Querying messages for session: ${sessionId}`)
      const messages = await directClient.query(api.messages.listBySession, {
        sessionId: sessionId,
        limit: 5
      })
      console.log(`Found ${messages?.length || 0} messages for session ${sessionId}`)
      if (messages && messages.length > 0) {
        console.log("First message:", JSON.stringify(messages[0], null, 2))
      }
      
      // Also check with getRecent
      console.log(`\n3. Querying recent messages for session: ${sessionId}`)
      const recentMessages = await directClient.query(api.messages.getRecent, {
        sessionId: sessionId,
        limit: 5
      })
      console.log(`Found ${recentMessages?.length || 0} recent messages`)
    }
    
  } catch (error) {
    console.error("Error querying Convex:", error)
  }
}

// Run the test
testMessageQueries()