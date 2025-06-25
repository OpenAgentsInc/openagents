#!/usr/bin/env bun

/**
 * Find specific messages in JSONL files
 */

import * as fs from "fs/promises"
import * as path from "path"
import * as os from "os"

const TARGET_UUIDS = [
  "c7b7ab32-5a6d-438b-bd73-f97b2a75ec42", // Empty blue box (assistant)
  "75f5d516-754d-4d9a-bb42-a272fa37c30b"  // Green box with "[Empty message]" (user)
]

async function findMessagesInJSONL() {
  const claudePath = path.join(os.homedir(), ".claude", "projects")
  
  try {
    // Get all project directories
    const projectDirs = await fs.readdir(claudePath)
    
    for (const projectDir of projectDirs) {
      const projectPath = path.join(claudePath, projectDir)
      const stat = await fs.stat(projectPath)
      
      if (stat.isDirectory()) {
        // Get all JSONL files in this project
        const files = await fs.readdir(projectPath)
        const jsonlFiles = files.filter(f => f.endsWith('.jsonl'))
        
        for (const jsonlFile of jsonlFiles) {
          const filePath = path.join(projectPath, jsonlFile)
          const content = await fs.readFile(filePath, 'utf-8')
          const lines = content.trim().split('\n')
          
          for (const line of lines) {
            if (!line) continue
            
            try {
              const entry = JSON.parse(line)
              
              // Check if this entry's UUID matches our targets
              if (TARGET_UUIDS.includes(entry.uuid)) {
                console.log("\n" + "=".repeat(80))
                console.log(`FOUND MESSAGE: ${entry.uuid}`)
                console.log(`File: ${filePath}`)
                console.log(`Type: ${entry.type}`)
                console.log("\nFull entry:")
                console.log(JSON.stringify(entry, null, 2))
                console.log("=".repeat(80))
              }
            } catch (e) {
              // Skip invalid JSON lines
            }
          }
        }
      }
    }
    
    console.log("\nSearch complete.")
  } catch (error) {
    console.error("Error searching JSONL files:", error)
  }
}

// Also check if these messages exist in Convex
import { ConvexClient } from "./src/client.js"
import { Effect } from "effect"

const checkConvex = Effect.gen(function*() {
  console.log("\nChecking Convex database for these UUIDs...")
  
  for (const uuid of TARGET_UUIDS) {
    const message = yield* ConvexClient.messages.getByUuid("", uuid).pipe(
      Effect.catchAll(() => Effect.succeed(null))
    )
    
    if (message) {
      console.log(`\nFound in Convex: ${uuid}`)
      console.log("Content:", message.content?.substring(0, 100))
      console.log("Entry type:", message.entry_type)
      console.log("Role:", message.role)
    } else {
      console.log(`\nNOT found in Convex: ${uuid}`)
    }
  }
})

async function main() {
  console.log("Searching for messages with UUIDs:")
  TARGET_UUIDS.forEach(uuid => console.log(`- ${uuid}`))
  
  // First search JSONL files
  await findMessagesInJSONL()
  
  // Then check Convex
  await Effect.runPromise(checkConvex).catch(console.error)
}

main()