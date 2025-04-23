import * as dotenv from "dotenv"
import { Effect, Ref } from "effect"
import * as fs from "node:fs"
import * as Http from "node:http"
import * as path from "node:path"
import type { AgentState } from "./github/AgentStateTypes.js"
import { GitHubClient } from "./github/GitHub.js"
import { TaskExecutor } from "./github/TaskExecutor.js"
import { AllLayers } from "./Program.js"

// Load environment variables from .env file
const loadConfig = Effect.try({
  try: () => {
    dotenv.config()
    const config = {
      anthropicApiKey: process.env.ANTHROPIC_API_KEY,
      githubApiKey: process.env.GITHUB_TOKEN,
      githubRepo: process.env.GITHUB_REPO_NAME || "openagents",
      githubRepoOwner: process.env.GITHUB_REPO_OWNER || "OpenAgentsInc"
    }

    if (!config.anthropicApiKey) {
      throw new Error("ANTHROPIC_API_KEY is required but not found in environment")
    } else {
      console.log(
        `DEBUG: ANTHROPIC_API_KEY is set (length: ${config.anthropicApiKey.length}, starts with: ${config.anthropicApiKey.substring(0, 3)
        }...)`
      )
    }

    if (!config.githubApiKey) {
      throw new Error("GITHUB_API_KEY is required but not found in environment")
    } else {
      console.log(
        `DEBUG: GITHUB_TOKEN is set (length: ${config.githubApiKey.length}, starts with: ${config.githubApiKey.substring(0, 3)
        }...)`
      )
    }

    return config
  },
  catch: (error) => new Error(`Failed to load configuration: ${error}`)
})

// Public directory path
const publicDir = path.join(process.cwd(), "public")

// Function to log outside of Effect
const log = (message: string): void => console.log(message)
const error = (message: string): void => console.error(message)

// Create a map to store SSE clients
const clients = new Map<string, Http.ServerResponse>()
let lastClientId = 0

// Function to send SSE message to all clients
const broadcastSSE = (event: string, data: string): void => {
  // Format a standard SSE message
  const message = `event: ${event}\ndata: ${data}\n\n`

  // Log all SSE events during debugging to see what's happening with communication
  log(`DEBUG: Broadcasting SSE event: ${event} to ${clients.size} clients`)

  if (clients.size === 0) {
    log(`DEBUG: WARNING - No SSE clients connected to receive ${event} event!`)
  }

  // Only log detailed data for certain events
  if (event === "connected" || event === "status") {
    log(`Broadcasting ${event} event to ${clients.size} clients`)
  } else if (event === "analysis" && data.includes("error")) {
    log(`Error: ${data.substring(data.indexOf("<span class=\"error\">") + 19, data.indexOf("</span>"))}`)
  } else if (event === "agent_state" || event === "error") {
    // Log a truncated version of the data for important events
    log(`DEBUG: SSE data for ${event}: ${data.substring(0, 100)}...`)
  }

  for (const client of clients.values()) {
    try {
      client.write(message)
      log(`DEBUG: Successfully wrote SSE message to client`)
    } catch (err) {
      error(`DEBUG ERROR: Failed to write SSE message to client: ${err instanceof Error ? err.message : String(err)}`)
    }
  }
}

// Create a function to generate agent state updates for the UI
const createAgentStateUpdate = (state: AgentState) => ({
  instanceId: state.agent_info.instance_id,
  taskStatus: state.current_task.status,
  currentStepDescription: state.plan[state.current_task.current_step_index]?.description ?? "N/A",
  currentStepNumber: state.plan[state.current_task.current_step_index]?.step_number ?? 0,
  stepsCompleted: state.metrics.steps_completed,
  totalSteps: state.metrics.total_steps_in_plan,
  lastError: state.error_state.last_error
    ? { message: state.error_state.last_error.message, type: state.error_state.last_error.type }
    : null
})

// Comments about environment variables and API keys
// Note: We now import AllLayers directly from Program.js to ensure consistent layer composition

// Setup a route to handle the SSE connection
const sseHandler = (req: Http.IncomingMessage, res: Http.ServerResponse): void => {
  log(`DEBUG: SSE connection requested, setting up headers`)

  // Set headers for SSE
  try {
    res.writeHead(200, {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      "Connection": "keep-alive"
    })
    log(`DEBUG: SSE headers successfully set`)
  } catch (err) {
    error(`DEBUG ERROR: Failed to set SSE headers: ${err instanceof Error ? err.message : String(err)}`)
    return
  }

  // Add client to the list first to ensure all messages are received
  const clientId = (++lastClientId).toString()
  clients.set(clientId, res)
  log(`DEBUG: Added new SSE client with ID ${clientId}, total clients: ${clients.size}`)

  // Send initial connection message
  log(`DEBUG: Sending initial connection message to client ${clientId}`)
  try {
    // Write directly to this client first
    const connMessage = `event: connected\ndata: SSE connection established\n\n`
    res.write(connMessage)
    log(`DEBUG: Successfully sent direct connection message to client ${clientId}`)
  } catch (err) {
    error(`DEBUG ERROR: Failed to send direct connection message: ${err instanceof Error ? err.message : String(err)}`)
  }

  // Send via broadcast (for consistency in logs)
  broadcastSSE("connected", "SSE connection established")

  // Send initial status information
  log(`DEBUG: Sending initial status information to client ${clientId}`)
  const statusData = JSON.stringify({
    connection: "connected",
    agent: "ready"
  })
  broadcastSSE("status", statusData)

  // Handle client disconnect
  req.on("close", () => {
    log(`DEBUG: SSE client ${clientId} disconnected`)
    clients.delete(clientId)
    log(`DEBUG: Removed client ${clientId}, remaining clients: ${clients.size}`)
  })

  // Handle errors
  res.on("error", (err) => {
    error(`DEBUG ERROR: SSE client ${clientId} error: ${err.message}`)
    clients.delete(clientId)
    log(`DEBUG: Removed client ${clientId} due to error, remaining clients: ${clients.size}`)
  })

  // Send a test message every 10 seconds to keep connection alive and verify it's working
  const keepAliveTimer = setInterval(() => {
    try {
      if (!clients.has(clientId)) {
        log(`DEBUG: Client ${clientId} no longer exists, clearing keepalive timer`)
        clearInterval(keepAliveTimer)
        return
      }

      const keepAliveMessage = `event: keepalive\ndata: ${new Date().toISOString()}\n\n`
      res.write(keepAliveMessage)
      log(`DEBUG: Sent keepalive to client ${clientId}`)
    } catch (err) {
      error(
        `DEBUG ERROR: Failed to send keepalive to client ${clientId}: ${err instanceof Error ? err.message : String(err)
        }`
      )
      clients.delete(clientId)
      clearInterval(keepAliveTimer)
    }
  }, 10000)
}

// Create a simple HTTP server
const createHttpServer = (): Http.Server => {
  const server = Http.createServer((req, res) => {
    const url = req.url || "/"
    log(`Request received: ${req.method} ${url}`)

    // Handle SSE endpoint
    if (url === "/sse") {
      sseHandler(req, res)
      return
    }

    // Handle fetch GitHub issue endpoint
    if (url.startsWith("/fetch-issue") && req.method === "POST") {
      // Parse form data from the request body
      let body = ""
      req.on("data", (chunk) => {
        body += chunk.toString()
      })

      req.on("end", () => {
        // Parse the form data
        const formData = new URLSearchParams(body)
        const issueNumber = parseInt(formData.get("issue") || "1", 10)
        const owner = formData.get("owner") || process.env.GITHUB_REPO_OWNER || "OpenAgentsInc"
        const repo = formData.get("repo") || process.env.GITHUB_REPO_NAME || "openagents"

        log(`Analyzing GitHub issue #${issueNumber} from ${owner}/${repo}`)

        // Define the execution pipeline
        const executionPipeline = Effect.gen(function* () {
          // Get services from context
          log(`DEBUG: Getting services from context`)
          const githubClient = yield* GitHubClient
          const taskExecutor = yield* TaskExecutor
          log(`DEBUG: Successfully got services (GitHubClient and TaskExecutor) from context`)

          // Generate unique instance ID
          const instanceId = `solver-${owner}-${repo}-${issueNumber}-${Date.now()}`
          log(`DEBUG: Generated instanceId: ${instanceId}`)

          // Try to load or create state
          log(`DEBUG: About to load or create state for ${instanceId}`)
          let initialState
          try {
            initialState = yield* githubClient.loadAgentState(instanceId).pipe(
              Effect.catchAll((err) => {
                log(`DEBUG: State not found (${err.message}), creating new state for ${owner}/${repo}#${issueNumber}`)
                return githubClient.createAgentStateForIssue(owner, repo, issueNumber)
              })
            )
            log(`DEBUG: Successfully loaded or created state: ${initialState.agent_info.instance_id}`)
          } catch (err) {
            error(`DEBUG ERROR: Failed to load or create state: ${err instanceof Error ? err.stack : String(err)}`)
            throw err
          }

          // Create a Ref to hold the current state
          log(`DEBUG: Creating stateRef`)
          const stateRef = yield* Ref.make(initialState)
          log(`DEBUG: Successfully created stateRef`)

          // Send initial state update to UI
          log(`DEBUG: Broadcasting initial state to UI`)
          broadcastSSE("agent_state", JSON.stringify(createAgentStateUpdate(initialState)))
          log(`DEBUG: Successfully broadcast initial state`)

          // Function to check if we should continue execution
          const shouldContinue = (state: AgentState): boolean => {
            const terminalStatuses = ["completed", "error", "blocked"]
            if (terminalStatuses.includes(state.current_task.status)) {
              return false
            }
            return true
          }

          // Define the execution loop with recursion
          // @ts-expect-error TypeScript struggles with recursive Effect typing
          const executeLoop: Effect.Effect<AgentState> = Effect.suspend(() =>
            Effect.gen(function* (): Generator<any, AgentState> {
              // Get current state
              log(`DEBUG: executeLoop - Getting current state from stateRef`)
              const currentState = yield* Ref.get(stateRef)
              log(
                `DEBUG: executeLoop - Got current state, step index: ${currentState.current_task.current_step_index}, status: ${currentState.current_task.status}`
              )

              // Check if we should continue
              if (!shouldContinue(currentState)) {
                log(`DEBUG: executeLoop - Task ended with terminal status: ${currentState.current_task.status}`)
                return currentState
              }

              try {
                // Execute next step
                const stepNumber = currentState.current_task.current_step_index + 1
                log(`DEBUG: executeLoop - About to execute step ${stepNumber}`)

                // This is where we call TaskExecutor.executeNextStep - this might be where it hangs
                log(`DEBUG: executeLoop - Calling taskExecutor.executeNextStep(currentState)`)
                const updatedState = yield* taskExecutor.executeNextStep(currentState)
                log(`DEBUG: executeLoop - Successfully returned from taskExecutor.executeNextStep`)

                // Update the state reference
                log(`DEBUG: executeLoop - Updating stateRef with new state`)
                yield* Ref.set(stateRef, updatedState)
                log(`DEBUG: executeLoop - Successfully updated stateRef`)

                // Broadcast updated state to UI
                log(`DEBUG: executeLoop - Broadcasting updated state to UI`)
                broadcastSSE("agent_state", JSON.stringify(createAgentStateUpdate(updatedState)))
                log(`DEBUG: executeLoop - Successfully broadcast updated state`)

                // Continue execution recursively
                log(`DEBUG: executeLoop - Continuing execution recursively`)
                return yield* executeLoop
              } catch (err) {
                const errorMsg = err instanceof Error ? err.message : String(err)
                const stackTrace = err instanceof Error ? err.stack : "No stack trace"
                error(`DEBUG ERROR: executeLoop - Error executing step: ${errorMsg}`)
                error(`DEBUG ERROR: executeLoop - Stack trace: ${stackTrace}`)

                // Get current state again to update with error
                log(`DEBUG: executeLoop (error handler) - Getting failed state from stateRef`)
                const failedState = yield* Ref.get(stateRef)
                log(`DEBUG: executeLoop (error handler) - Got failed state`)

                // Update task status to error if not already set
                if (failedState.current_task.status !== "error") {
                  log(`DEBUG: executeLoop (error handler) - Updating state with error status`)
                  const errorState = {
                    ...failedState,
                    current_task: {
                      ...failedState.current_task,
                      status: "error"
                    }
                  }

                  // Update state reference with error state
                  yield* Ref.set(stateRef, errorState)
                  log(`DEBUG: executeLoop (error handler) - Updated stateRef with error state`)

                  // Broadcast error state to UI
                  log(`DEBUG: executeLoop (error handler) - Broadcasting error state to UI`)
                  broadcastSSE("agent_state", JSON.stringify(createAgentStateUpdate(errorState)))
                  log(`DEBUG: executeLoop (error handler) - Successfully broadcast error state`)
                }

                // End execution
                log(`DEBUG: executeLoop (error handler) - Ending execution due to error`)
                return yield* Ref.get(stateRef)
              }
            })
          )

          // Start the execution loop
          log(`DEBUG: About to start the execution loop`)
          try {
            yield* executeLoop
            log(`DEBUG: Execution loop completed successfully`)
          } catch (err) {
            error(`DEBUG ERROR: Uncaught error in executeLoop: ${err instanceof Error ? err.stack : String(err)}`)
            throw err
          }

          // Return final state
          return yield* Ref.get(stateRef)
        })

        // Run the execution pipeline with all required services
        log(`DEBUG: About to run pipeline with Effect.runFork`)

        // Create the pipeline with error handling
        const pipelineWithErrorHandling = executionPipeline.pipe(
          // Add error logging for the entire pipeline
          Effect.catchAll((err) => {
            // Log the error thoroughly
            const errorMsg = err instanceof Error ? err.message : String(err)
            const stackTrace = err instanceof Error ? err.stack : "No stack trace"
            error(`DEBUG FATAL PIPELINE ERROR: ${errorMsg}`)
            error(`DEBUG FATAL PIPELINE ERROR STACK: ${stackTrace}`)

            // Broadcast an SSE error
            broadcastSSE("error", `FATAL PIPELINE ERROR: ${errorMsg}`)

            // Re-throw the error to propagate it
            return Effect.fail(err)
          })
        )

        // Provide the AppLayer and run the fork
        // Use as any to work around type issues with complex Effect.js pipelines
        try {
          log(`DEBUG: CRITICAL - About to call Effect.runFork on pipeline`)

          // Add a direct console.log inside the pipeline to see if it's being executed at all
          // This bypasses Effect logging to see if there's an issue with Effect itself
          const pipelineWithDebugging = Effect.gen(function* () {
            console.log("DEBUG: CRITICAL - Inside pipeline generator function - BASELINE")

            try {
              // Simplified approach: run the pipeline directly without individual service tests
              // This avoids potential reference equality issues with the service Tags
              console.log("DEBUG: CRITICAL - Running pipeline with AllLayers from Program.js")
              return yield* pipelineWithErrorHandling
            } catch (err) {
              // Log any errors directly to console
              console.error("DEBUG: CRITICAL ERROR - Failed in pipeline debugging:", err instanceof Error ? err.stack : String(err))
              throw err
            }
          })

          // Use the AllLayers imported from Program.js
          log(`DEBUG: CRITICAL - Using AllLayers from Program.js`)

          // Add direct tracking of the fork's execution
          // Use 'as any' to handle complex Effect.js type inference issues
          const fork = Effect.runFork(
            Effect.provide(pipelineWithDebugging, AllLayers).pipe(
              Effect.catchAll((err) => {
                // Log the error thoroughly
                const errorMsg = err instanceof Error ? err.stack : String(err)
                console.error(`FATAL PIPELINE ERROR: ${errorMsg}`)
                broadcastSSE("error", `FATAL PIPELINE ERROR: ${err instanceof Error ? err.message : String(err)}`)
                return Effect.void // End the effect gracefully after logging
              })
            ) as any
          )
          log(`DEBUG: CRITICAL - Effect.runFork returned fork object`)

          // Add observer
          fork.addObserver((exit) => {
            if (exit._tag === "Failure") {
              console.error("DEBUG: CRITICAL - Fork failed with cause:", exit.cause)
            } else if (exit._tag === "Success") {
              console.log("DEBUG: CRITICAL - Fork completed successfully with value:", exit.value)
            }
          })
        } catch (err) {
          // This catches errors in the setup of the fork itself, not in the pipeline
          error(`DEBUG: CRITICAL ERROR - Exception thrown from Effect.runFork setup: ${err instanceof Error ? err.stack : String(err)}`)
        }

        // Immediately respond to the HTTP request
        res.writeHead(200, { "Content-Type": "text/plain" })
        res.end("Issue processing initiated")
        log(`DEBUG: HTTP response sent, async processing continues in background`)
      })
      return
    }

    // Handle get agent status endpoint
    if (url === "/agent-status" && req.method === "GET") {
      const status = {
        status: "ready",
        last_updated: new Date().toISOString(),
        current_task: null,
        progress: {
          steps_completed: 0,
          total_steps: 0
        }
      }
      res.writeHead(200, { "Content-Type": "application/json" })
      res.end(JSON.stringify(status))
      return
    }

    // Serve static files from public directory
    if (url === "/") {
      const indexPath = path.join(publicDir, "index.html")
      if (fs.existsSync(indexPath)) {
        let content = fs.readFileSync(indexPath, "utf8")

        // Get the config values to inject into the HTML
        try {
          const config = Effect.runSync(loadConfig)
          // Replace placeholders with actual values
          content = content
            .replace("value=\"GITHUB_REPO_OWNER\"", `value="${config.githubRepoOwner}"`)
            .replace("value=\"GITHUB_REPO_NAME\"", `value="${config.githubRepo}"`)
        } catch (error) {
          const err = error instanceof Error ? error.message : String(error)
          log(`Warning: Failed to load config for HTML template: ${err}`)
          // Use fallback values if config loading fails
          content = content
            .replace("value=\"GITHUB_REPO_OWNER\"", "value=\"OpenAgentsInc\"")
            .replace("value=\"GITHUB_REPO_NAME\"", "value=\"openagents\"")
        }

        res.writeHead(200, { "Content-Type": "text/html" })
        res.end(content)
      } else {
        res.writeHead(404, { "Content-Type": "text/plain" })
        res.end("Index file not found")
      }
      return
    }

    // Serve static files (like CSS, JavaScript, and fonts)
    const filePath = path.join(publicDir, url)
    if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
      const ext = path.extname(filePath)
      let contentType = "text/plain"

      if (ext === ".html") contentType = "text/html"
      else if (ext === ".css") contentType = "text/css"
      else if (ext === ".js") contentType = "text/javascript"
      else if (ext === ".woff") contentType = "font/woff"
      else if (ext === ".woff2") contentType = "font/woff2"

      const content = fs.readFileSync(filePath)
      res.writeHead(200, { "Content-Type": contentType })
      res.end(content)
      return
    }

    // Handle 404
    res.writeHead(404, { "Content-Type": "text/plain" })
    res.end("Not found")
  })

  return server
}

// Export the function to start the server
export const startServer = (): void => {
  const server = createHttpServer()
  const port = 3000

  server.listen(port, () => {
    log(`Server started on http://localhost:${port}`)
  })

  server.on("error", (err) => {
    error(`Server error: ${err.message}`)
  })
}

// Start the server if this file is run directly
if (import.meta.url === `file://${process.argv[1]}`) {
  startServer()
}
