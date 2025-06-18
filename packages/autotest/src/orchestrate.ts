#!/usr/bin/env bun
import { BunContext, BunRuntime } from "@effect/platform-bun"
import { Console, Effect, Layer } from "effect"
import { BrowserServiceLive } from "./Browser/Service.js"
import { ScreenshotServiceLive } from "./Screenshot/Service.js"
import { ServerServiceLive } from "./Server/Service.js"
import { TestOrchestratorLive, TestOrchestrator } from "./Orchestrator/TestOrchestrator.js"
import type { OrchestratorConfig } from "./Orchestrator/types.js"

// Parse command line arguments
const args = process.argv.slice(2)

if (args.length === 0) {
  console.error("Usage: bun run orchestrate <config-json>")
  console.error("Example: bun run orchestrate '{\"project\":{\"root\":\"/path/to/project\",\"startCommand\":\"bun run dev\"}}'")
  process.exit(1)
}

// Default configuration for testing OpenAgents
const defaultConfig: OrchestratorConfig = {
  project: {
    root: process.cwd(),
    startCommand: "bun run dev",
    port: 3003,
    readyPattern: /OpenAgents is running at|listening on|ready/i
  },
  testing: {
    routes: ["/", "/chat", "/docs", "/about", "/components"],
    interactions: [
      {
        route: "/chat",
        actions: ["select-model", "send-message"]
      }
    ],
    timeout: 30000
  },
  monitoring: {
    captureConsole: true,
    captureNetwork: true,
    captureErrors: true,
    screenshotOnError: true
  }
}

const program = Effect.gen(function*() {
  // Parse config from command line or use default
  let config: OrchestratorConfig
  
  if (args[0] === "--default") {
    config = defaultConfig
    yield* Console.log("Using default configuration for OpenAgents.com")
  } else {
    try {
      config = JSON.parse(args[0])
    } catch (error) {
      yield* Console.error(`Invalid JSON: ${error}`)
      return yield* Effect.fail("Invalid configuration format")
    }
  }

  // Run the test orchestration
  const orchestrator = yield* TestOrchestrator
  const report = yield* orchestrator.runFullTest(config)

  // Output report
  yield* Console.log("\n=== Test Report ===")
  yield* Console.log(`Duration: ${report.duration}ms`)
  yield* Console.log(`Routes tested: ${report.summary.totalRoutes}`)
  yield* Console.log(`Passed: ${report.summary.passedRoutes}`)
  yield* Console.log(`Failed: ${report.summary.failedRoutes}`)
  
  if (report.summary.failedRoutes > 0) {
    yield* Console.error("\n=== Failed Routes ===")
    for (const route of report.routes.filter(r => !r.success)) {
      yield* Console.error(`- ${route.route}: ${route.errors.length} errors`)
      for (const error of route.errors) {
        yield* Console.error(`  - [${error.type}] ${error.message}`)
      }
    }
  }

  if (report.suggestedFixes && report.suggestedFixes.length > 0) {
    yield* Console.log("\n=== Suggested Fixes ===")
    for (const fix of report.suggestedFixes) {
      yield* Console.log(`- ${fix.issue}: ${fix.suggestion}`)
    }
  }

  // Save detailed report
  const reportPath = "./test-report.json"
  yield* Effect.tryPromise({
    try: () => Bun.write(reportPath, JSON.stringify(report, null, 2)),
    catch: (error) => new Error(`Failed to write report: ${error}`)
  })
  
  yield* Console.log(`\nDetailed report saved to: ${reportPath}`)

  // Exit with appropriate code
  if (report.summary.failedRoutes > 0) {
    yield* Effect.fail("Tests failed")
  }
})

// Create the layer with all services
const MainLive = Layer.mergeAll(
  ServerServiceLive,
  BrowserServiceLive,
  ScreenshotServiceLive,
  TestOrchestratorLive
).pipe(
  Layer.provide(BunContext.layer)
)

// Run the program
const runnable = program.pipe(
  Effect.provide(MainLive),
  Effect.catchAll((error) =>
    Console.error(`Error: ${error}`).pipe(
      Effect.flatMap(() => Effect.fail(error))
    )
  )
)

BunRuntime.runMain(runnable)