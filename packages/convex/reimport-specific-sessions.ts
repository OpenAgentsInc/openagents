#!/usr/bin/env bun

/**
 * Re-import specific sessions containing the empty messages
 */

import { exec } from "child_process"
import { promisify } from "util"

const execAsync = promisify(exec)

// Sessions that contain the problematic messages
const SESSION_IDS = [
  "96e1bdb0-541a-4275-a3ca-2a1ec98da6cc",
  "865916aa-87b7-4f29-af69-bf22eef729f7"
]

async function main() {
  console.log("Re-importing specific sessions with empty message issues...")
  console.log("Sessions to re-import:", SESSION_IDS)
  
  try {
    // Use overlord import command to re-import
    // Since we can't target specific sessions, we'll import all and rely on the update logic
    const { stdout, stderr } = await execAsync(
      `cd ../overlord && bun run src/index.ts import --user-id="claude-code-user" --api-key="test" --limit=100`,
      { maxBuffer: 1024 * 1024 * 10 } // 10MB buffer
    )
    
    if (stderr) {
      console.error("Stderr:", stderr)
    }
    
    console.log(stdout)
    console.log("\n✅ Re-import complete. The empty messages should now display properly:")
    console.log("- Assistant messages with only tool_use will show '🔧 Using tool: [toolname]'")
    console.log("- User messages with tool_result will show '📤 Tool Result: [content]'")
    console.log("\n🔗 Check the messages at:")
    console.log("- http://localhost:3003/chat/96e1bdb0-541a-4275-a3ca-2a1ec98da6cc")
    console.log("- http://localhost:3003/chat/865916aa-87b7-4f29-af69-bf22eef729f7")
  } catch (error) {
    console.error("Failed to re-import:", error)
  }
}

main()