#!/usr/bin/env bun

/**
 * Script to run the session fixes
 */

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")

async function runFixes() {
  console.log("=== Running Session Fixes ===\n")

  try {
    // 1. Fix project names
    console.log("1. Fixing project names...")
    const projectResult = await client.mutation(api.sessionFixes.fixAllProjectNames)
    console.log(`   Updated ${projectResult.updated} out of ${projectResult.total} sessions\n`)

    // 2. Fix message counts
    console.log("2. Fixing message counts...")
    const countResult = await client.mutation(api.sessionFixes.fixMessageCounts)
    console.log(`   Fixed ${countResult.fixed} out of ${countResult.total} sessions\n`)

    // 3. Fix message content (do a sample first)
    console.log("3. Fixing message content (sample of 100)...")
    const contentResult = await client.mutation(api.sessionFixes.fixMessageContent, { limit: 100 })
    console.log(`   Fixed ${contentResult.fixed} out of ${contentResult.total} messages\n`)

    // 4. Get diagnostics for a sample session
    console.log("4. Getting diagnostics for a sample session...")
    const sampleSessionId = "7644ecb5-b708-4418-9ed5-a08c87a1e72f"
    const diagnostics = await client.query(api.sessionFixes.getSessionDiagnostics, {
      sessionId: sampleSessionId
    })

    if (diagnostics) {
      console.log(`   Session: ${diagnostics.session.id}`)
      console.log(`   Project: ${diagnostics.session.project_name} (${diagnostics.session.project_path})`)
      console.log(
        `   Messages: ${diagnostics.session.actual_messages} actual vs ${diagnostics.session.claimed_messages} claimed`
      )
      console.log(
        `   Content issues: ${diagnostics.messages.empty_content} empty, ${diagnostics.messages.json_content} JSON`
      )
      console.log(`   Message types:`, diagnostics.messages.by_type)
    }

    console.log("\n✅ Fixes completed!")
  } catch (error) {
    console.error("Error running fixes:", error)
  }
}

runFixes()
