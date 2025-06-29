#!/usr/bin/env bun

/**
 * Inspect a specific raw message to understand its structure
 */

import * as fs from "fs"

const JSONL_FILE_PATH = "/Users/christopherdavid/code/openagents/docs/ingest/claude-code-example.jsonl"

async function main() {
  const fileContent = fs.readFileSync(JSONL_FILE_PATH, "utf-8")
  const lines = fileContent.trim().split("\n").filter((line) => line.trim())

  // Look at line 6 (the first assistant message with empty content)
  console.log("Raw line 6 (first empty assistant message):")
  const line6 = JSON.parse(lines[5]) // 0-indexed
  console.log(JSON.stringify(line6, null, 2))

  console.log("\n\nRaw line 8 (another empty assistant message):")
  const line8 = JSON.parse(lines[7])
  console.log(JSON.stringify(line8, null, 2))
}

main().catch(console.error)
