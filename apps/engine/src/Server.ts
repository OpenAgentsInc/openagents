import { Effect } from "effect"
import * as fs from "node:fs"
import * as Http from "node:http"
import * as Https from "node:https"
import * as path from "node:path"

// Public directory path
const publicDir = path.join(process.cwd(), "public")

// Function to log outside of Effect
const log = (message: string) => console.log(message)
const error = (message: string) => console.error(message)

// Create a map to store SSE clients
const clients = new Map<string, Http.ServerResponse>()
let lastClientId = 0

// Get a dad joke from icanhazdadjoke.com
const fetchDadJoke = Effect.async<string, Error>((resume) => {
  log("Fetching a dad joke...")

  const request = Https.request({
    hostname: "icanhazdadjoke.com",
    path: "/",
    method: "GET",
    headers: {
      "Accept": "application/json",
      "User-Agent": "Effect Dad Joke Streamer (https://effect.website)"
    }
  }, (response) => {
    let data = ""

    response.on("data", (chunk) => data += chunk)

    response.on("end", () => {
      try {
        if (data.trim() === '') {
          throw new Error("Empty response received")
        }

        const parsedData = JSON.parse(data)
        if (parsedData && parsedData.joke) {
          log(`Joke received: ${parsedData.joke}`)
          resume(Effect.succeed(parsedData.joke))
        } else {
          throw new Error("No joke found in response")
        }
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err))
        log(`Error parsing joke: ${error.message}`)
        resume(Effect.fail(error))
      }
    })
  })

  request.on("error", (err) => {
    log(`Error fetching joke: ${err.message}`)
    resume(Effect.fail(err))
  })

  request.end()
})

// Function to send SSE message to all clients
const broadcastSSE = (event: string, data: string) => {
  // Format a standard SSE message
  const message = `event: ${event}\ndata: ${data}\n\n`

  log(`Broadcasting ${event} event to ${clients.size} clients`)
  // Don't truncate log output - no need

  for (const client of clients.values()) {
    client.write(message)
  }
}

// Setup a route to handle the SSE connection
const sseHandler = (req: Http.IncomingMessage, res: Http.ServerResponse) => {
  log("New SSE connection established")
  log("SSE Headers: " + JSON.stringify(req.headers))

  // Set headers for SSE
  res.writeHead(200, {
    'Content-Type': 'text/event-stream',
    'Cache-Control': 'no-cache',
    'Connection': 'keep-alive'
  })

  // Send initial connection message - using the standard format
  broadcastSSE("connected", "SSE connection established")
  log("Sent initial connection message")

  // Wait a moment then send a test message
  setTimeout(() => {
    // Send all test messages via broadcast function for consistency
    broadcastSSE("test", "This is a test event to confirm SSE is working")
    broadcastSSE("test-broadcast", "Test broadcast via broadcastSSE function")
  }, 1000)

  // Add client to the list
  const clientId = (++lastClientId).toString()
  clients.set(clientId, res)

  log(`Client ${clientId} connected, total clients: ${clients.size}`)

  // Handle client disconnect
  req.on("close", () => {
    log(`Client ${clientId} disconnected`)
    clients.delete(clientId)
  })

  // Handle errors
  res.on("error", (err) => {
    error(`SSE client ${clientId} error: ${err.message}`)
    clients.delete(clientId)
  })
}

// Create a simple HTTP server
const createHttpServer = () => {
  const server = Http.createServer((req, res) => {
    const url = req.url || "/"
    log(`Request received: ${req.method} ${url}`)

    // Handle SSE endpoint
    if (url === "/sse") {
      sseHandler(req, res)
      return
    }

    // Handle joke fetch endpoint
    if (url === "/fetch-joke" && req.method === "POST") {
      fetchDadJoke.pipe(
        Effect.match({
          onSuccess: (joke) => {
            // Preserve line breaks using <br> to ensure proper HTML display
            const jokeWithLineBreaks = joke.replace(/\n/g, "<br>");
            broadcastSSE("joke", `<div><h3>Original Dad Joke:</h3><p>${jokeWithLineBreaks}</p></div>`)
            broadcastSSE("expansion", "")
            expandJokeWithClaude(joke)
            res.writeHead(200, { "Content-Type": "text/plain" })
            res.end("Joke fetch initiated")
          },
          onFailure: (err) => {
            error(`Error in fetch-joke: ${err.message}`)
            res.writeHead(500, { "Content-Type": "text/plain" })
            res.end("Error fetching joke")
          }
        }),
        Effect.runPromise
      )
      return
    }

    // Serve static files from public directory
    if (url === "/") {
      const indexPath = path.join(publicDir, "index.html")
      if (fs.existsSync(indexPath)) {
        const content = fs.readFileSync(indexPath, "utf8")
        res.writeHead(200, { "Content-Type": "text/html" })
        res.end(content)
      } else {
        res.writeHead(404, { "Content-Type": "text/plain" })
        res.end("Index file not found")
      }
      return
    }

    // Serve static files (like font files)
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

// Modify expandJokeWithClaude to use SSE
const expandJokeWithClaude = (joke: string) => {
  const apiKey = process.env.ANTHROPIC_API_KEY

  if (!apiKey) {
    error("ANTHROPIC_API_KEY environment variable not set")
    broadcastSSE("expansion", "<span class='error'>API key not configured</span>")
    return
  }

  try {
    log("Setting up request to Anthropic API")
    const options = {
      hostname: "api.anthropic.com",
      path: "/v1/messages",
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "anthropic-version": "2023-06-01",
        "x-api-key": apiKey,
        "anthropic-beta": "messages-2023-12-15"
      }
    }

    // Prepare the request payload
    const prompt = `You are a comedian writer. You've just been given this dad joke:

"${joke}"

Expand this into a 2-paragraph comedic short story that elaborates on the joke while keeping it family-friendly.
Don't repeat the original joke verbatim. Just create a funny short story inspired by the joke.`

    const data = JSON.stringify({
      model: "claude-3-haiku-20240307",
      max_tokens: 1000,
      stream: true,
      messages: [
        { role: "user", content: prompt }
      ]
    })

    // Make the request
    log("Sending request to Anthropic API")
    const req = Https.request(options, (apiRes) => {
      log(`Anthropic API response status: ${apiRes.statusCode} ${apiRes.statusMessage}`)

      // Start fresh with each request
      let storyContent = ""

      // Initialize with blank content
      broadcastSSE("expansion", `<div><h3>Claude's Expansion:</h3><p class='story'></p></div>`)

      apiRes.on("data", (chunk) => {
        const lines = chunk.toString().split("\n").filter((line: string) => line.trim() !== "")

        for (const line of lines) {
          try {
            // Check if it's a data line
            if (line.startsWith("data: ")) {
              const jsonPart = line.slice(6) // Remove "data: " prefix

              // Check for [DONE] marker
              if (jsonPart.trim() === "[DONE]") {
                log("Received [DONE] marker")
                // Process and preserve line breaks for final message
                const finalContent = storyContent.replace(/\n/g, "<br>");
                // Log final content length
                log(`FINAL content length: ${storyContent.length} chars`);
                log(`Final content with line breaks: ${finalContent.substring(0, 100)}...`);

                // Add timestamp to ensure fresh rendering
                const timestamp = Date.now();

                // Send final complete message - add "complete" class to signal it's done
                broadcastSSE("expansion", `<div><h3>Claude's Expansion:</h3><p class='story complete' data-ts="${timestamp}">${finalContent}</p></div>`)
                continue
              }

              const data = JSON.parse(jsonPart)

              // If it has delta content, send it to the client
              if (data.type === "content_block_delta" && data.delta && data.delta.text) {
                const text = data.delta.text
                storyContent += text
                log(`Streaming expansion chunk: ${text}`)

                // Process and preserve line breaks for proper display
                const processedContent = storyContent.replace(/\n/g, "<br>");
                // Log length of current content to verify it's growing
                log(`Current content length: ${storyContent.length} chars`);

                // For debugging, let's add a counter to make each response unique
                // This ensures browser doesn't cache incorrectly
                const timestamp = Date.now();

                // Send complete content each time to refresh the entire div
                broadcastSSE("expansion", `<div><h3>Claude's Expansion:</h3><p class='story' data-ts="${timestamp}">${processedContent}</p></div>`)
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
      broadcastSSE("expansion", "<span class='error'>Error connecting to language model API</span>")
    })

    // Send the request
    req.write(data)
    req.end()

  } catch (err) {
    error(`Exception setting up Anthropic request: ${err}`)
    broadcastSSE("expansion", "<span class='error'>Error setting up expansion request</span>")
  }
}

// Export the function to start the server
export const startServer = () => {
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
