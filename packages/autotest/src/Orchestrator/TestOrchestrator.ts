import { Console, Context, Effect, Layer } from "effect"
import { BrowserService } from "../Browser/Service.js"
import type { Page } from "../Browser/types.js"
import { ScreenshotService } from "../Screenshot/Service.js"
import { ServerService } from "../Server/Service.js"
import { navigate, performInteractions } from "../Testing/Interactions.js"
import type { InteractionStep } from "../Testing/types.js"
import type {
  ConsoleMessage,
  NetworkRequest,
  OrchestratorConfig,
  RouteTestResult,
  TestError,
  TestReport,
  TestSummary
} from "./types.js"

export class TestOrchestrator extends Context.Tag("@openagentsinc/autotest/TestOrchestrator")<
  TestOrchestrator,
  {
    readonly runFullTest: (config: OrchestratorConfig) => Effect.Effect<TestReport, Error>
    readonly testRoute: (
      page: Page,
      route: string,
      interactions?: ReadonlyArray<InteractionStep>
    ) => Effect.Effect<RouteTestResult, Error>
  }
>() {}

const setupPageMonitoring = (page: Page, config: OrchestratorConfig) =>
  Effect.sync(() => {
    const consoleMessages: Array<ConsoleMessage> = []
    const networkRequests: Array<NetworkRequest> = []
    const errors: Array<TestError> = []

    if (config.monitoring?.captureConsole) {
      page.instance.on("console", (msg) => {
        consoleMessages.push({
          type: msg.type() as any,
          text: msg.text(),
          timestamp: new Date()
        })

        // Track console errors
        if (msg.type() === "error") {
          errors.push({
            type: "console",
            message: msg.text(),
            timestamp: new Date()
          })
        }
      })
    }

    if (config.monitoring?.captureNetwork) {
      page.instance.on("request", (req) => {
        const request: NetworkRequest = {
          url: req.url(),
          method: req.method()
        }
        networkRequests.push(request)
      })

      page.instance.on("response", (res) => {
        const request = networkRequests.find((r) => r.url === res.url())
        if (request) {
          request.status = res.status()
          if (res.status() >= 400) {
            errors.push({
              type: "network",
              message: `${request.method} ${request.url} failed with status ${res.status()}`,
              timestamp: new Date()
            })
          }
        }
      })

      page.instance.on("requestfailed", (req) => {
        const request = networkRequests.find((r) => r.url === req.url())
        if (request) {
          request.error = req.failure()?.errorText || "Unknown error"
          errors.push({
            type: "network",
            message: `${request.method} ${request.url} failed: ${request.error}`,
            timestamp: new Date()
          })
        }
      })
    }

    if (config.monitoring?.captureErrors) {
      page.instance.on("pageerror", (error) => {
        errors.push({
          type: "assertion",
          message: error.message,
          stack: error.stack || undefined,
          timestamp: new Date()
        })
      })
    }

    return { consoleMessages, networkRequests, errors }
  })

export const TestOrchestratorLive = Layer.effect(
  TestOrchestrator,
  Effect.gen(function*() {
    const serverService = yield* ServerService
    const browserService = yield* BrowserService
    const screenshotService = yield* ScreenshotService

    return TestOrchestrator.of({
      testRoute: (page, route, interactions) =>
        Effect.gen(function*() {
          const startTime = Date.now()
          const screenshots: Array<string> = []
          let success = true

          try {
            // Navigate to route
            yield* navigate(page, route).pipe(
              Effect.tapError(() =>
                Effect.sync(() => {
                  success = false
                })
              )
            )

            // Wait for page to settle
            yield* Effect.sleep("1 second")

            // Perform interactions if provided
            if (interactions && interactions.length > 0) {
              yield* performInteractions(page, interactions).pipe(
                Effect.tapError(() =>
                  Effect.sync(() => {
                    success = false
                  })
                )
              )
            }

            // Take screenshot
            const screenshot = yield* screenshotService.capture({
              page,
              fullPage: true
            })
            screenshots.push(screenshot.filename)
          } catch {
            success = false

            // Take error screenshot if configured
            try {
              const errorScreenshot = yield* screenshotService.capture({
                page,
                fullPage: true
              })
              screenshots.push(errorScreenshot.filename)
            } catch {
              // Ignore screenshot errors
            }
          }

          const duration = Date.now() - startTime

          // Get monitoring data from page context
          const monitoring = (page as any).__monitoring || {
            consoleMessages: [],
            networkRequests: [],
            errors: []
          }

          return {
            route,
            success,
            duration,
            errors: monitoring.errors,
            screenshots,
            console: monitoring.consoleMessages,
            network: monitoring.networkRequests
          }
        }),

      runFullTest: (config) =>
        Effect.gen(function*() {
          const startedAt = new Date()

          yield* Console.log("Starting test orchestration...")

          // Start server
          yield* Console.log(`Starting server with command: ${config.project.startCommand}`)
          const serverProcess = yield* serverService.start({
            command: config.project.startCommand,
            cwd: config.project.root,
            port: config.project.port || 3000,
            env: config.project.env || {},
            readyPattern: config.project.readyPattern
          })

          yield* Console.log(`Server started with PID: ${serverProcess.pid}`)

          // Wait for server to be ready
          yield* Console.log("Waiting for server to be ready...")
          yield* serverService.waitForReady(serverProcess, {
            timeout: config.testing.timeout ?? 30000,
            ...(config.project.readyPattern && { pattern: config.project.readyPattern })
          })

          yield* Console.log("Server is ready!")

          // Launch browser
          const browser = yield* browserService.launch({ headless: true })
          const routeResults: Array<RouteTestResult> = []

          try {
            // Test each route
            for (const route of config.testing.routes) {
              yield* Console.log(`Testing route: ${route}`)

              const page = yield* browserService.newPage(browser)

              try {
                // Set up monitoring
                const monitoring = yield* setupPageMonitoring(page, config)
                ;(page as any).__monitoring = monitoring

                // Build full URL
                const baseUrl = config.testing.baseUrl || serverProcess.url
                const fullUrl = `${baseUrl}${route}`

                // Find interactions for this route
                const routeInteractions = config.testing.interactions?.find(
                  (i) => i.route === route
                )

                // Convert string actions to interaction steps
                const interactions = routeInteractions?.actions.map((action) => {
                  if (typeof action === "string") {
                    // Simple action mapping
                    if (action === "select-model") {
                      return {
                        action: "select" as const,
                        selector: "#chat-model-select",
                        value: "qwen2.5:latest"
                      }
                    }
                    if (action === "send-message") {
                      return {
                        action: "fill" as const,
                        selector: "#chat-input",
                        value: "Hello, test message"
                      }
                    }
                    // Default click action
                    return {
                      action: "click" as const,
                      selector: action
                    }
                  }
                  return action
                })

                // Test the route
                const testRouteImpl = yield* TestOrchestrator
                const result = yield* testRouteImpl.testRoute(page, fullUrl, interactions)
                routeResults.push(result)

                yield* Console.log(
                  `Route ${route}: ${result.success ? "✓ PASSED" : "✗ FAILED"} (${result.duration}ms)`
                )
              } finally {
                yield* browserService.closePage(page)
              }
            }
          } finally {
            // Clean up
            yield* browserService.close(browser)
            yield* serverService.stop(serverProcess)
          }

          // Get final server state for logs
          const serverState = yield* serverService.getState(serverProcess)
          const completedAt = new Date()

          // Calculate summary
          const summary: TestSummary = {
            totalRoutes: routeResults.length,
            passedRoutes: routeResults.filter((r) => r.success).length,
            failedRoutes: routeResults.filter((r) => !r.success).length,
            totalErrors: routeResults.reduce((sum, r) => sum + r.errors.length, 0),
            errorsByType: routeResults.reduce((acc, r) => {
              r.errors.forEach((e) => {
                acc[e.type] = (acc[e.type] || 0) + 1
              })
              return acc
            }, {} as Record<string, number>)
          }

          yield* Console.log(`
Test Summary:
- Total routes: ${summary.totalRoutes}
- Passed: ${summary.passedRoutes}
- Failed: ${summary.failedRoutes}
- Total errors: ${summary.totalErrors}
          `)

          return {
            startedAt,
            completedAt,
            duration: completedAt.getTime() - startedAt.getTime(),
            serverLogs: serverState.process?.logs || [],
            routes: routeResults,
            summary,
            suggestedFixes: analyzeFixes(routeResults)
          }
        })
    })
  })
)

// Helper to analyze errors and suggest fixes
function analyzeFixes(results: ReadonlyArray<RouteTestResult>) {
  const fixes = []

  // Check for common patterns
  for (const result of results) {
    for (const error of result.errors) {
      if (error.type === "network" && error.message.includes("404")) {
        fixes.push({
          issue: "Missing route",
          description: error.message,
          suggestion: "Add the missing route handler or check the URL path"
        })
      }

      if (error.type === "console" && error.message.includes("import")) {
        fixes.push({
          issue: "Missing import",
          description: error.message,
          suggestion: "Add the missing import statement"
        })
      }

      if (error.type === "console" && error.message.includes("TypeError")) {
        fixes.push({
          issue: "Type error",
          description: error.message,
          suggestion: "Check for undefined values or incorrect types"
        })
      }
    }
  }

  return fixes
}
