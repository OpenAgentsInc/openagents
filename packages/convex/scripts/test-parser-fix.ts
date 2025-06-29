#!/usr/bin/env bun

/**
 * Quick test of the fixed parser on a smaller subset
 */

import { Effect } from "effect"
import * as fs from "fs"
import { convertToConvexMessages, parseJSONL } from "../overlord/src/services/JSONLParser"

const JSONL_FILE_PATH = "/Users/christopherdavid/code/openagents/docs/ingest/claude-code-example.jsonl"

async function testParserFix() {
  console.log("🧪 Testing parser fix on tool results...")

  const fileContent = fs.readFileSync(JSONL_FILE_PATH, "utf-8")

  // Take just the first 50 lines to test quickly
  const lines = fileContent.trim().split("\n").filter((line) => line.trim())
  const testContent = lines.slice(0, 50).join("\n")

  console.log(`Testing with ${lines.slice(0, 50).length} lines...`)

  try {
    const entries = await Effect.runPromise(parseJSONL(testContent))
    console.log(`✅ Parsed ${entries.length} entries`)

    // Check for tool results
    const userEntries = entries.filter((e) => e.type === "user")
    const toolResultCount = userEntries.reduce((total, entry) => total + entry.tool_results.length, 0)

    console.log(`📊 User entries: ${userEntries.length}`)
    console.log(`📊 Tool results: ${toolResultCount}`)

    if (toolResultCount > 0) {
      console.log("✅ Tool results found - testing Convex conversion...")

      const convexMessages = convertToConvexMessages(entries, "test-session")
      const toolResultMessages = convexMessages.filter((m) => m.entry_type === "tool_result")

      console.log(`📦 Convex tool result messages: ${toolResultMessages.length}`)

      // Check if any have complex tool_output
      const complexOutputs = toolResultMessages.filter((m) => {
        try {
          const parsed = JSON.parse(m.tool_output)
          return typeof parsed === "object"
        } catch {
          return false
        }
      })

      console.log(`🔧 Complex tool outputs (should be stringified): ${complexOutputs.length}`)

      if (complexOutputs.length > 0) {
        console.log("Sample complex output (first 200 chars):")
        console.log(complexOutputs[0].tool_output.substring(0, 200) + "...")
      }
    }

    console.log("🎉 Parser test completed successfully!")
  } catch (error) {
    console.error("❌ Parser test failed:", error)
  }
}

testParserFix().catch(console.error)
