#!/usr/bin/env node

import { Effect, Layer, Console } from "effect"
import { NodeCommandExecutor } from "@effect/platform-node"
import { 
  ClaudeCodeClient, 
  ClaudeCodeClientLive, 
  ClaudeCodeConfigDefault 
} from "./packages/ai/build/esm/internal.js"

const program = Effect.gen(function*() {
  yield* Console.log("🔍 Testing Claude Code integration...")
  
  const client = yield* ClaudeCodeClient
  
  // Check availability
  yield* Console.log("\n1. Checking Claude Code availability...")
  const isAvailable = yield* client.checkAvailability().pipe(
    Effect.catchTag("ClaudeCodeNotFoundError", () => {
      return Console.log("❌ Claude Code CLI not found").pipe(
        Effect.map(() => false)
      )
    })
  )
  
  if (isAvailable) {
    yield* Console.log("✅ Claude Code CLI is available!")
    
    // Test a simple prompt
    yield* Console.log("\n2. Testing simple prompt...")
    const response = yield* client.prompt("Say hello!", {
      outputFormat: "text"
    }).pipe(
      Effect.catchAll((error) => {
        return Console.log(`❌ Error: ${error._tag} - ${JSON.stringify(error)}`).pipe(
          Effect.map(() => ({ content: "Error occurred" }))
        )
      })
    )
    
    yield* Console.log(`📝 Response: ${response.content}`)
  }
  
  yield* Console.log("\n✅ Test complete!")
})

// Run the program
program.pipe(
  Effect.provide(ClaudeCodeClientLive),
  Effect.provide(ClaudeCodeConfigDefault),
  Effect.provide(NodeCommandExecutor.layer),
  Effect.runPromise
).catch(console.error)