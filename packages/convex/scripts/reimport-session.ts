#!/usr/bin/env bun

/**
 * Re-import a specific session to apply parsing fixes
 */

import { exec } from "child_process"
import { promisify } from "util"

const execAsync = promisify(exec)
const SESSION_ID = "466d695f-2808-42f3-97d3-2465cfb138a7"
const USER_ID = "claude-code-user"

async function main() {
  console.log(`Re-importing sessions to apply tool parsing fixes...`)

  try {
    // Use overlord import command to re-import sessions
    const { stderr, stdout } = await execAsync(
      `cd ../overlord && bun run src/index.ts import --user-id="${USER_ID}" --api-key="test" --limit=50`,
      { maxBuffer: 1024 * 1024 * 10 } // 10MB buffer
    )

    if (stderr) {
      console.error("Stderr:", stderr)
    }

    console.log(stdout)
    console.log("✅ Re-import complete. Tool data should now be visible in debug JSON.")
    console.log(`\n🔗 Check the conversation at: http://localhost:3003/chat/${SESSION_ID}`)
  } catch (error) {
    console.error("Failed to re-import:", error)
  }
}

main()
