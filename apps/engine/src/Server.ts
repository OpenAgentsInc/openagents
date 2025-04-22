import * as dotenv from "dotenv"
import { Effect } from "effect"
import * as fs from "node:fs"
import * as Http from "node:http"
import * as Https from "node:https"
import * as path from "node:path"

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
    }

    if (!config.githubApiKey) {
      throw new Error("GITHUB_API_KEY is required but not found in environment")
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

  // Only log initial events and errors
  if (event === "connected" || event === "status") {
    log(`Broadcasting ${event} event to ${clients.size} clients`)
  } else if (event === "analysis" && data.includes("error")) {
    log(`Error: ${data.substring(data.indexOf("<span class=\"error\">") + 19, data.indexOf("</span>"))}`)
  }

  for (const client of clients.values()) {
    client.write(message)
  }
}

// Get GitHub issue from GitHub API
const fetchGitHubIssue = (owner: string, repo: string, issueNumber: number): void => {
  Effect.runPromise(loadConfig).then((config) => {
    try {
      log(`Fetching GitHub issue #${issueNumber} from ${owner}/${repo}...`)

      const options = {
        hostname: "api.github.com",
        path: `/repos/${owner}/${repo}/issues/${issueNumber}`,
        method: "GET",
        headers: {
          "Accept": "application/vnd.github.v3+json",
          "Authorization": `token ${config.githubApiKey}`,
          "User-Agent": "OpenAgents-Engine/1.0"
        }
      }

      const request = Https.request(options, (response) => {
        let data = ""

        response.on("data", (chunk) => data += chunk)

        response.on("end", () => {
          try {
            if (data.trim() === "") {
              throw new Error("Empty response received")
            }

            const parsedData = JSON.parse(data)
            if (parsedData) {
              log(`Issue #${issueNumber} received: ${parsedData.title}`)

              // Format the issue data for display
              const issueHtml = `
                <div>
                  <h3>GitHub Issue #${parsedData.number}: ${parsedData.title}</h3>
                  <div class="issue-metadata">
                    <span>Status: <strong>${parsedData.state}</strong></span>
                    <span>Created by: <strong>${parsedData.user.login}</strong></span>
                    <span>Created: <strong>${new Date(parsedData.created_at).toLocaleString()}</strong></span>
                  </div>
                  <div class="issue-body">
                    <h4>Description:</h4>
                    <p>${parsedData.body.replace(/\n/g, "<br>")}</p>
                  </div>
                </div>
              `

              // Show issue details in the analysis div first
              broadcastSSE("analysis", issueHtml)
              analyzeIssueWithClaude(parsedData)
            } else {
              throw new Error("Invalid issue data received")
            }
          } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err)
            log(`Error parsing issue: ${errorMessage}`)
            broadcastSSE("analysis", `<span class="error">Error fetching issue: ${errorMessage}</span>`)
          }
        })
      })

      request.on("error", (err) => {
        log(`Error fetching issue: ${err.message}`)
        broadcastSSE("analysis", `<span class="error">Error fetching issue: ${err.message}</span>`)
      })

      request.end()
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err)
      error(`Exception setting up GitHub request: ${errorMessage}`)
      broadcastSSE("analysis", `<span class="error">Error setting up GitHub request: ${errorMessage}</span>`)
    }
  }).catch((err) => {
    error(`Failed to load configuration: ${err}`)
    broadcastSSE("analysis", `<span class="error">Failed to load API configuration: ${err}</span>`)
  })
}

// Analyze a GitHub issue with Claude
const analyzeIssueWithClaude = (issue: {
  title: string
  user: { login: string }
  created_at: string
  state: string
  body: string
}): void => {
  Effect.runPromise(loadConfig).then((config) => {
    try {
      log("Setting up request to Anthropic API")
      const options = {
        hostname: "api.anthropic.com",
        path: "/v1/messages",
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "anthropic-version": "2023-06-01",
          "x-api-key": config.anthropicApiKey,
          "anthropic-beta": "messages-2023-12-15"
        }
      }

      // Prepare the request payload
      const prompt = `You are an AI assistant tasked with analyzing GitHub issues.
I'll provide you with a GitHub issue, and I'd like you to:

1. Summarize the main points of the issue
2. Identify the type of issue (bug report, feature request, question, etc.)
3. Suggest what additional information might be helpful (if any)
4. Outline potential next steps for addressing this issue

Here's the issue:

Title: ${issue.title}
Author: ${issue.user.login}
Created: ${issue.created_at}
Status: ${issue.state}

Description:
${issue.body}

Please format your response as structured analysis with clear headings.`

      const data = JSON.stringify({
        model: "claude-3-haiku-20240307",
        max_tokens: 1000,
        stream: true,
        messages: [
          { role: "user", content: prompt }
        ]
      })

      // Make the request
      const req = Https.request(options, (apiRes) => {
        if (apiRes.statusCode !== 200) {
          log(`Anthropic API response status: ${apiRes.statusCode} ${apiRes.statusMessage}`)
        }

        // Start fresh with each request
        let analysisContent = ""

        // Begin analysis section after issue display
        broadcastSSE("analysis", `<div><h3>Analysis:</h3><p class='analysis'></p></div>`)

        apiRes.on("data", (chunk) => {
          const lines = chunk.toString().split("\n").filter((line: string) => line.trim() !== "")

          for (const line of lines) {
            try {
              // Check if it's a data line
              if (line.startsWith("data: ")) {
                const jsonPart = line.slice(6) // Remove "data: " prefix

                // Check for [DONE] marker
                if (jsonPart.trim() === "[DONE]") {
                  // Process and preserve line breaks for final message
                  const finalContent = analysisContent.replace(/\n/g, "<br>")
                  log("✓ Analysis completed")

                  // Add timestamp to ensure fresh rendering
                  const timestamp = Date.now()

                  // Send final complete message - add "complete" class to signal it's done
                  broadcastSSE(
                    "analysis",
                    `<div><h3>Analysis:</h3><p class='analysis complete' data-ts="${timestamp}">${finalContent}</p></div>`
                  )
                  continue
                }

                const data = JSON.parse(jsonPart)

                // If it has delta content, send it to the client
                if (data.type === "content_block_delta" && data.delta && data.delta.text) {
                  const text = data.delta.text
                  analysisContent += text

                  // Log deltas but only if they contain meaningful text (not just whitespace)
                  if (text.trim().length > 0) {
                    log(`Delta: ${text.trim()}`)
                  }

                  // Process and preserve line breaks for proper display
                  const processedContent = analysisContent.replace(/\n/g, "<br>")

                  // For debugging, let's add a counter to make each response unique
                  // This ensures browser doesn't cache incorrectly
                  const timestamp = Date.now()

                  // Send complete content each time to refresh the entire div
                  broadcastSSE(
                    "analysis",
                    `<div><h3>Analysis:</h3><p class='analysis' data-ts="${timestamp}">${processedContent}</p></div>`
                  )
                }
              }
            } catch (parseErr) {
              log(`Error parsing line: ${line}`)
              log(`Parse error: ${parseErr}`)
            }
          }
        })

        apiRes.on("end", () => {
          log("Anthropic API stream ended")
        })
      })

      req.on("error", (err) => {
        error(`Error making request to Anthropic API: ${err.message}`)
        broadcastSSE("analysis", `<span class='error'>Error connecting to language model API: ${err.message}</span>`)
      })

      // Send the request
      req.write(data)
      req.end()
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err)
      error(`Exception setting up Anthropic request: ${errorMessage}`)
      broadcastSSE("analysis", `<span class='error'>Error setting up analysis request: ${errorMessage}</span>`)
    }
  }).catch((err) => {
    error(`Failed to load configuration: ${err}`)
    broadcastSSE("analysis", `<span class='error'>Failed to load API configuration: ${err}</span>`)
  })
}

// Get agent state
const getAgentStatus = (): {
  status: string
  last_updated: string
  current_task: null
  progress: {
    steps_completed: number
    total_steps: number
  }
} => {
  return {
    status: "ready",
    last_updated: new Date().toISOString(),
    current_task: null,
    progress: {
      steps_completed: 0,
      total_steps: 0
    }
  }
}

// Setup a route to handle the SSE connection
const sseHandler = (req: Http.IncomingMessage, res: Http.ServerResponse): void => {
  // Set headers for SSE
  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache",
    "Connection": "keep-alive"
  })

  // Send initial connection message
  broadcastSSE("connected", "SSE connection established")

  // Send initial status information
  const statusData = JSON.stringify({
    connection: "connected",
    agent: "ready"
  })
  broadcastSSE("status", statusData)

  // Add client to the list
  const clientId = (++lastClientId).toString()
  clients.set(clientId, res)

  // Handle client disconnect
  req.on("close", () => {
    clients.delete(clientId)
  })

  // Handle errors
  res.on("error", (err) => {
    error(`SSE client ${clientId} error: ${err.message}`)
    clients.delete(clientId)
  })
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
        fetchGitHubIssue(owner, repo, issueNumber)

        res.writeHead(200, { "Content-Type": "text/plain" })
        res.end("Issue fetch initiated")
      })
      return
    }

    // Handle get agent status endpoint
    if (url === "/agent-status" && req.method === "GET") {
      const status = getAgentStatus()
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
