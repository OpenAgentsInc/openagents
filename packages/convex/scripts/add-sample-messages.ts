#!/usr/bin/env bun

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

// Add sample messages for testing
async function addSampleMessages() {
  console.log("Adding sample messages for testing...")

  const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")

  try {
    // Get a session to test with
    const sessions = await client.query(api.sessions.listByUser, { userId: "test-user" })

    if (sessions.length === 0) {
      console.log("No sessions found. Creating a test session...")

      // We'd need to create mutation endpoints for this
      console.log("Cannot create sessions from client - they come from Overlord sync")
      return
    }

    // Use the first session
    const testSession = sessions[0]
    console.log(`Using session: ${testSession.id} (${testSession.project_name || testSession.project_path})`)

    // Check if we have a mutation to update messages
    console.log("\nNote: To fix user messages, we need to:")
    console.log("1. Create a Convex mutation to update message content")
    console.log("2. Re-import JSONL files with proper content extraction")
    console.log("3. Or manually update messages in the Convex dashboard")

    // For now, let's create sample conversations using the web API
    console.log("\nCreating a test conversation through the web API...")
  } catch (error) {
    console.error("Error:", error)
  }
}

// Run the script
addSampleMessages()
