#!/usr/bin/env bun

/**
 * Debug the import process to find why content is empty
 */

import { parseJSONL, extractSessionMetadata, convertToConvexMessages } from "../overlord/src/services/JSONLParser"
import { Effect } from "effect"
import * as fs from "fs"

const JSONL_FILE_PATH = "/Users/christopherdavid/code/openagents/docs/ingest/claude-code-example.jsonl"

async function main() {
  console.log("🔍 Debugging import process...")
  
  // Read file
  const fileContent = fs.readFileSync(JSONL_FILE_PATH, 'utf-8')
  const lines = fileContent.trim().split("\n").filter(line => line.trim())
  
  console.log(`\n📄 File has ${lines.length} lines`)
  
  // Parse with Effect-based parser
  const parseEffect = parseJSONL(fileContent)
  const entries = await Effect.runPromise(parseEffect)
  
  console.log(`\n✅ Parsed ${entries.length} entries`)
  
  // Check assistant messages
  const assistantEntries = entries.filter(e => e.type === "assistant")
  console.log(`\n📊 Found ${assistantEntries.length} assistant entries`)
  
  // Sample a few assistant messages
  console.log("\n--- First 5 Assistant Messages ---")
  for (let i = 0; i < Math.min(5, assistantEntries.length); i++) {
    const entry = assistantEntries[i]
    console.log(`\n[${i + 1}] UUID: ${entry.uuid}`)
    console.log(`Content: ${entry.content ? `"${entry.content.substring(0, 100)}..."` : "EMPTY"}`)
    console.log(`Content length: ${entry.content?.length || 0}`)
    console.log(`Has thinking: ${!!entry.thinking}`)
    console.log(`Tool uses: ${entry.tool_uses.length}`)
  }
  
  // Now convert to Convex format
  const sessionId = "test-session"
  const convexMessages = convertToConvexMessages(entries, sessionId)
  
  // Check the converted messages
  const convexAssistantMessages = convexMessages.filter(m => m.entry_type === "assistant")
  console.log(`\n📦 Converted to ${convexAssistantMessages.length} Convex assistant messages`)
  
  console.log("\n--- First 5 Converted Assistant Messages ---")
  for (let i = 0; i < Math.min(5, convexAssistantMessages.length); i++) {
    const msg = convexAssistantMessages[i]
    console.log(`\n[${i + 1}] UUID: ${msg.entry_uuid}`)
    console.log(`Content: ${msg.content ? `"${msg.content.substring(0, 100)}..."` : "EMPTY"}`)
    console.log(`Content length: ${msg.content?.length || 0}`)
    console.log(`Has thinking: ${!!msg.thinking}`)
  }
  
  // Check the raw JSONL to see what's in the original
  console.log("\n--- Checking Raw JSONL ---")
  let assistantCount = 0
  for (let i = 0; i < Math.min(100, lines.length) && assistantCount < 5; i++) {
    try {
      const parsed = JSON.parse(lines[i])
      if (parsed.type === "assistant") {
        assistantCount++
        console.log(`\n[Raw ${assistantCount}] Line ${i + 1}`)
        console.log(`UUID: ${parsed.uuid}`)
        console.log(`Has message: ${!!parsed.message}`)
        console.log(`Has content: ${!!parsed.message?.content}`)
        if (parsed.message?.content && Array.isArray(parsed.message.content)) {
          console.log(`Content array length: ${parsed.message.content.length}`)
          const textItems = parsed.message.content.filter((item: any) => item.type === "text")
          console.log(`Text items: ${textItems.length}`)
          if (textItems.length > 0) {
            console.log(`First text: "${textItems[0].text?.substring(0, 100) || "NO TEXT FIELD"}"`)
          }
        }
      }
    } catch (e) {
      // Skip parse errors
    }
  }
}

main().catch(console.error)