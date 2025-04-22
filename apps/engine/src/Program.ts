import * as NodeContext from "@effect/platform-node/NodeContext"
import * as crypto from "crypto"
import * as Effect from "effect/Effect"
import * as fs from "node:fs"
import { createServer } from "node:http"
import * as path from "node:path"

// Simple logging functions
const log = (message: string) => console.log(`[${new Date().toISOString()}] ${message}`)
const error = (message: string) => console.error(`[${new Date().toISOString()}] ERROR: ${message}`)

// Use process.cwd() to get the current working directory
// This is more reliable than __dirname when dealing with compiled code
const publicDir = path.join(process.cwd(), "public")

// Log the resolved public directory path for debugging
log(`Public directory path: ${publicDir}`)

// Use Effect-style functions with Node.js primitives
const startServer = Effect.gen(function*(_) {
  yield* _(Effect.log("Starting server on http://localhost:3000"))

  // Create a Node.js HTTP server
  const server = createServer((req, res) => {
    // Handle HTTP request
    const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`)
    const urlPath = url.pathname

    if (urlPath === "/") {
      // Serve the index.html file
      const indexPath = path.join(publicDir, "index.html")
      log(`Attempting to serve index.html from: ${indexPath}`)

      if (fs.existsSync(indexPath)) {
        log(`Found index.html, serving content`)
        const content = fs.readFileSync(indexPath, "utf8")
        res.writeHead(200, { "Content-Type": "text/html" })
        res.end(content)
      } else {
        error(`Index file not found at path: ${indexPath}`)
        res.writeHead(404, { "Content-Type": "text/plain" })
        res.end("Index file not found")
      }
    } else {
      // Check if the requested file exists in the public directory
      const filePath = path.join(publicDir, urlPath)
      if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
        const ext = path.extname(filePath)
        let contentType = "text/plain"

        if (ext === ".html") contentType = "text/html"
        else if (ext === ".css") contentType = "text/css"
        else if (ext === ".js") contentType = "text/javascript"

        const content = fs.readFileSync(filePath)
        res.writeHead(200, { "Content-Type": contentType })
        res.end(content)
      } else {
        // 404 for all other HTTP paths
        res.writeHead(404, { "Content-Type": "text/plain" })
        res.end("Not Found")
      }
    }
  })

  // WebSocket handling using basic Node.js
  server.on("upgrade", (req, socket, _head) => {
    const wsUrl = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`)
    const wsPath = wsUrl.pathname

    log(`WebSocket upgrade request for path: ${wsPath}`)

    if (wsPath === "/ws") {
      // This is a WebSocket upgrade request for /ws path
      const key = req.headers["sec-websocket-key"]
      log(`WebSocket connection with key: ${key || "none"}`)

      if (!key) {
        error("Missing Sec-WebSocket-Key header")
        socket.write("HTTP/1.1 400 Bad Request\r\n\r\n")
        socket.destroy()
        return
      }

      // Compute the WebSocket accept key
      const acceptKey = crypto
        .createHash("sha1")
        .update(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11", "binary")
        .digest("base64")

      // Send the handshake response
      socket.write(
        "HTTP/1.1 101 Switching Protocols\r\n" +
          "Upgrade: websocket\r\n" +
          "Connection: Upgrade\r\n" +
          `Sec-WebSocket-Accept: ${acceptKey}\r\n\r\n`
      )

      // Function to create a simple text WebSocket frame
      const createTextFrame = (text: string) => {
        const textBuffer = Buffer.from(text)
        const frame = Buffer.alloc(textBuffer.length + 2)
        frame[0] = 0x81 // Text frame
        frame[1] = textBuffer.length // Unmasked, length
        textBuffer.copy(frame, 2)
        return frame
      }

      // Send welcome message
      log("Sending welcome message to WebSocket client")
      socket.write(createTextFrame("Connected to WebSocket"))

      // Handle data from client
      socket.on("data", (buffer) => {
        log(`Received WebSocket data of length: ${buffer.length} bytes`)
        // Simple WebSocket frame parsing - handle ping message
        if (buffer.length >= 6 && (buffer[0] & 0x0f) === 0x01) { // Text frame
          const secondByte = buffer[1]
          const isMasked = (secondByte & 0x80) !== 0
          const length = secondByte & 0x7f

          if (isMasked && length <= 125) {
            const maskingKey = buffer.slice(2, 6)
            const data = Buffer.alloc(length)

            // Unmask the data
            for (let i = 0; i < length; i++) {
              data[i] = buffer[i + 6] ^ maskingKey[i % 4]
            }

            // Check for ping message
            const message = data.toString()
            log(`Decoded WebSocket message: "${message}"`)

            if (message === "ping") {
              log("Received ping message, sending pong response")
              socket.write(createTextFrame("pong"))
            }
          }
        }
      })

      // Handle connection close
      socket.on("end", () => {
        log("WebSocket connection closed")
      })

      // Handle socket errors
      socket.on("error", (err) => {
        error(`WebSocket error: ${err.message}`)
      })
    } else {
      // Not a WebSocket upgrade for our path
      socket.destroy()
    }
  })

  // Start server
  yield* _(Effect.promise(() =>
    new Promise<void>((resolve) => {
      server.listen(3000, () => {
        log("Server started listening on port 3000")
        resolve()
      })
    })
  ))

  // Replace Effect.log with our custom log function
  log("HTTP server started on http://localhost:3000")
  log("WebSocket endpoint available at ws://localhost:3000/ws")

  // Keep server running
  return yield* _(Effect.never)
})

// Run the program
Effect.runPromise(
  Effect.provide(
    startServer,
    NodeContext.layer
  )
)
