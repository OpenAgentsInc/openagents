// Test proper import paths for CLI
import { Effect } from "effect"

// The correct way to import from the CLI package after build
console.log("Testing CLI import paths...")

// Check what's exported from the CLI package
import * as CLI from "./packages/cli/dist/src/index.js"

console.log("CLI exports:", Object.keys(CLI))

// Now test the container commands
if (CLI.container) {
  console.log("\n✅ Container commands found in CLI exports")
  console.log("Container exports:", Object.keys(CLI.container))
  
  // Run the test command
  await Effect.runPromise(
    CLI.container.containerTest().pipe(
      Effect.catchAll((error) => 
        Effect.gen(function* () {
          console.error("❌ Error:", error)
        })
      )
    )
  )
} else {
  console.log("\n❌ Container commands not found in CLI exports")
}