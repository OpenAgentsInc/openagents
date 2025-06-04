/**
 * Example usage of Claude Code integration
 * Run with: pnpm tsx packages/ai/examples/claude-code-example.ts
 */

import { NodeCommandExecutor } from "@effect/platform-node"
import { Console, Effect, Stream } from "effect"
import { ClaudeCodeClient, ClaudeCodeClientLive, makeClaudeCodeConfig } from "../src/index.js"

// Example 1: Check if Claude Code is available
const checkAvailability = Effect.gen(function*() {
  yield* Console.log("🔍 Checking Claude Code availability...")

  const client = yield* ClaudeCodeClient
  const isAvailable = yield* client.checkAvailability().pipe(
    Effect.catchAll(() =>
      Effect.gen(function*() {
        yield* Console.error("❌ Error: Claude Code CLI not found")
        return false
      })
    )
  )

  if (isAvailable) {
    yield* Console.log("✅ Claude Code is available!")
  } else {
    yield* Console.log("❌ Claude Code is not available")
    yield* Console.log("Please ensure 'claude' CLI is installed")
  }

  return isAvailable
})

// Example 2: Simple prompt
const simplePrompt = Effect.gen(function*() {
  yield* Console.log("\n📝 Sending a simple prompt...")

  const client = yield* ClaudeCodeClient
  const response = yield* client.prompt("What is 2 + 2?", {
    outputFormat: "json"
  })

  yield* Console.log("Response:", response)

  if ("usage" in response && response.usage) {
    yield* Console.log(`Tokens used: ${response.usage.total_tokens}`)
  }
})

// Example 3: Conversation with session
const conversation = Effect.gen(function*() {
  yield* Console.log("\n💬 Starting a conversation...")

  const client = yield* ClaudeCodeClient

  // First message
  const response1 = yield* client.prompt("My name is Alice. What's yours?", {
    outputFormat: "json"
  })

  yield* Console.log("Claude:", response1.content)

  // Continue conversation if we have a session ID
  if ("session_id" in response1 && response1.session_id) {
    yield* Console.log(`\n🔗 Continuing with session: ${response1.session_id}`)

    const response2 = yield* client.continueSession(
      response1.session_id,
      "What was my name again?",
      { outputFormat: "json" }
    )

    yield* Console.log("Claude:", response2.content)
  }
})

// Example 4: Streaming response
const streamingExample = Effect.gen(function*() {
  yield* Console.log("\n🌊 Streaming response example...")

  const client = yield* ClaudeCodeClient
  const stream = client.streamPrompt("Tell me a short story in 3 sentences.")

  yield* stream.pipe(
    Stream.tap((chunk) => Console.log(`Chunk: ${chunk}`)),
    Stream.runDrain
  )
})

// Main program
const program = Effect.gen(function*() {
  const isAvailable = yield* checkAvailability

  if (!isAvailable) {
    yield* Console.log("\n⚠️  Skipping examples since Claude Code is not available")
    return
  }

  // Run examples
  yield* simplePrompt.pipe(
    Effect.catchAll((error) => Console.error(`Simple prompt error: ${JSON.stringify(error)}`))
  )

  yield* conversation.pipe(
    Effect.catchAll((error) => Console.error(`Conversation error: ${JSON.stringify(error)}`))
  )

  yield* streamingExample.pipe(
    Effect.catchAll((error) => Console.error(`Streaming error: ${JSON.stringify(error)}`))
  )
})

// Run with custom configuration
const customConfig = makeClaudeCodeConfig({
  model: "claude-3-opus-20240229",
  outputFormat: "json",
  defaultTimeout: 30000
})

// Execute the program
program.pipe(
  Effect.provide(ClaudeCodeClientLive),
  Effect.provide(customConfig),
  Effect.provide(NodeCommandExecutor.layer),
  Effect.runPromise
).then(
  () => console.log("\n✨ Examples completed!"),
  (error) => console.error("\n💥 Failed:", error)
)
