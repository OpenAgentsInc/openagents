#!/usr/bin/env bun

// Use the built ConvexClient from our package
import { ConvexClient } from "./dist/esm/client.js"
import { Effect } from "effect"

// Import the API to access the listRecent function
import { api } from "./convex/_generated/api.js"
import { ConvexHttpClient } from "convex/browser"

// Test queries
async function testConvexQueries() {
  console.log("Testing Convex queries...")
  
  // Also create a direct client for additional queries
  const directClient = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")
  
  try {
    // First, query ALL sessions to see what's in the database
    console.log("\n0. Querying ALL recent sessions...")
    const allSessions = await directClient.query(api.sessions.listRecent, { limit: 100 })
    console.log(`Found ${allSessions?.length || 0} total sessions in database`)
    if (allSessions && allSessions.length > 0) {
      console.log("\nUnique user IDs in database:", [...new Set(allSessions.map(s => s.user_id))])
      console.log("\nFirst session:", JSON.stringify(allSessions[0], null, 2))
    }
    
    // Query sessions for specific user
    const userId = "test-user"
    console.log(`\n1. Querying sessions for user: ${userId}`)
    const userSessions = await Effect.runPromise(
      ConvexClient.sessions.listByUser(userId)
    )
    console.log(`Found ${userSessions?.length || 0} sessions for user ${userId}`)
    if (userSessions && userSessions.length > 0) {
      console.log("\nFirst session:", JSON.stringify(userSessions[0], null, 2))
      console.log("\nAll session IDs:", userSessions.map(s => s.id))
    }
    
    // If we found sessions, get messages for the first one
    if (userSessions && userSessions.length > 0) {
      const sessionId = userSessions[0].id
      console.log(`\n2. Querying messages for session: ${sessionId}`)
      const messages = await Effect.runPromise(
        ConvexClient.messages.listBySession(sessionId, 10)
      )
      console.log(`Found ${messages?.length || 0} messages`)
      if (messages && messages.length > 0) {
        console.log("\nFirst message:", JSON.stringify(messages[0], null, 2))
      }
    }
    
  } catch (error) {
    console.error("Error querying Convex:", error)
  }
}

// Run the test
testConvexQueries()