#!/usr/bin/env bun
import { Console, Effect } from "effect"
import { BrowserServiceLive } from "./Browser/Service.js"
import { captureScreenshot, validateRequest } from "./Claude/Integration.js"
import { ScreenshotServiceLive } from "./Screenshot/Service.js"

// Parse command line arguments
const args = process.argv.slice(2)

if (args.length === 0) {
  console.error("Usage: bun run capture <request-json>")
  console.error("Example: bun run capture '{\"url\":\"http://localhost:3000\",\"fullPage\":true}'")
  process.exit(1)
}

const program = Effect.gen(function*() {
  // Parse request from command line
  const requestJson = args[0]
  let request: unknown

  try {
    request = JSON.parse(requestJson)
  } catch (error) {
    yield* Console.error(`Invalid JSON: ${error}`)
    yield* Effect.fail("Invalid request format")
  }

  // Validate and process request
  const validatedRequest = yield* validateRequest(request)
  yield* Console.log(`Capturing screenshot for: ${validatedRequest.url}`)

  const result = yield* captureScreenshot(validatedRequest)

  yield* Console.log(`Screenshot saved to: ${result.path}`)

  // Output result as JSON for Claude Code integration
  yield* Console.log(JSON.stringify(result, null, 2))
})

// Run the program
const runnable = program.pipe(
  Effect.provide(BrowserServiceLive),
  Effect.provide(ScreenshotServiceLive),
  Effect.catchAll((error) =>
    Console.error(`Error: ${error}`).pipe(
      Effect.flatMap(() => Effect.fail(error))
    )
  )
)

Effect.runPromise(runnable).catch((error) => {
  console.error("Failed to capture screenshot:", error)
  process.exit(1)
})
