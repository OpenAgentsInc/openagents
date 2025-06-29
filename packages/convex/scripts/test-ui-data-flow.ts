#!/usr/bin/env bun

/**
 * Test the complete data flow from Convex to UI
 */

import { ConvexHttpClient } from "convex/browser"
import { Effect } from "effect"
import { api } from "./convex/_generated/api"
import { ConvexClient } from "./src/client"

const CONVEX_URL = process.env.CONVEX_URL || "https://proficient-panther-764.convex.cloud"

async function main() {
  console.log("🧪 Testing data flow from Convex to UI format...")

  // Method 1: Direct ConvexHttpClient (what check-messages.ts uses)
  const directClient = new ConvexHttpClient(CONVEX_URL)

  // Method 2: ConvexClient wrapper (what the UI uses)

  // Test 1: Get sessions via direct client
  console.log("\n📋 Test 1: Direct client sessions")
  const directSessions = await directClient.query(api.sessions.listRecent, { limit: 1 })
  console.log(`Found ${directSessions.length} sessions`)

  if (directSessions.length > 0) {
    const sessionId = directSessions[0].id
    console.log(`Using session: ${sessionId}`)

    // Test 2: Get messages via direct client
    console.log("\n📋 Test 2: Direct client messages")
    const directMessages = await directClient.query(api.messages.listBySession, {
      sessionId,
      limit: 5
    })

    console.log(`Found ${directMessages.length} messages`)
    directMessages.forEach((msg, idx) => {
      console.log(`\n[Direct ${idx + 1}] ${msg.entry_uuid}`)
      console.log(`  Type: ${msg.entry_type}`)
      console.log(`  Content: ${msg.content ? `"${msg.content.substring(0, 50)}..."` : "EMPTY"}`)
      console.log(`  Content length: ${msg.content?.length || 0}`)
    })

    // Test 3: Get same data via ConvexClient wrapper
    console.log("\n📋 Test 3: ConvexClient wrapper messages")
    const wrappedMessages = await Effect.runPromise(
      ConvexClient.messages.listBySession(sessionId, 5)
    )

    console.log(`Found ${wrappedMessages.length} messages via wrapper`)
    wrappedMessages.forEach((msg, idx) => {
      console.log(`\n[Wrapped ${idx + 1}] ${msg.entry_uuid}`)
      console.log(`  Type: ${msg.entry_type}`)
      console.log(`  Content: ${msg.content ? `"${msg.content.substring(0, 50)}..."` : "EMPTY"}`)
      console.log(`  Content length: ${msg.content?.length || 0}`)
    })

    // Test 4: Compare direct vs wrapped
    console.log("\n📋 Test 4: Comparing data")
    if (directMessages.length === wrappedMessages.length) {
      console.log("✅ Same number of messages")

      for (let i = 0; i < directMessages.length; i++) {
        const direct = directMessages[i]
        const wrapped = wrappedMessages[i]

        if (direct.entry_uuid !== wrapped.entry_uuid) {
          console.log(`❌ UUID mismatch at index ${i}`)
        }

        if (direct.content !== wrapped.content) {
          console.log(`❌ Content mismatch at index ${i}`)
          console.log(`   Direct: ${direct.content?.substring(0, 30) || "EMPTY"}`)
          console.log(`   Wrapped: ${wrapped.content?.substring(0, 30) || "EMPTY"}`)
        }
      }
    } else {
      console.log("❌ Different number of messages!")
    }
  }
}

main().catch(console.error)
