#!/usr/bin/env bun

import { ConvexHttpClient } from "./node_modules/convex/dist/esm/browser/index.js"

// Initialize Convex client
const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")

// Test queries
async function testConvexQueries() {
  console.log("Testing Convex queries...")
  
  try {
    // Query all sessions
    console.log("\n1. Querying all sessions...")
    const allSessions = await client.query((await import("./packages/convex/convex/_generated/api.js")).api.sessions.listRecent, { limit: 100 })
    console.log(`Found ${allSessions?.length || 0} total sessions`)
    if (allSessions && allSessions.length > 0) {
      console.log("Sample session:", allSessions[0])
    }
    
    // Query sessions for specific user
    const userId = "cdoingthis"
    console.log(`\n2. Querying sessions for user: ${userId}`)
    const userSessions = await client.query((await import("./packages/convex/convex/_generated/api.js")).api.sessions.listByUser, { 
      userId: userId,
      limit: 100 
    })
    console.log(`Found ${userSessions?.length || 0} sessions for user ${userId}`)
    if (userSessions && userSessions.length > 0) {
      console.log("Sample user session:", userSessions[0])
    }
    
    // List all unique user IDs
    console.log("\n3. Listing unique user IDs from sessions...")
    if (allSessions && allSessions.length > 0) {
      const uniqueUserIds = [...new Set(allSessions.map(s => s.user_id))]
      console.log("Unique user IDs found:", uniqueUserIds)
    }
    
  } catch (error) {
    console.error("Error querying Convex:", error)
  }
}

// Run the test
testConvexQueries()