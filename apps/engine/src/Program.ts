import * as NodeContext from "@effect/platform-node/NodeContext"
import * as crypto from "crypto"
import * as Effect from "effect/Effect"
import { createServer } from "http"

// Create the HTML content
const htmlContent = `<!DOCTYPE html>
<html>
<head>
  <title>OpenAgents Engine</title>
  <style>
    body {
      background-color: black;
      color: white;
      font-family: monospace;
      display: flex;
      justify-content: center;
      align-items: center;
      height: 100vh;
      margin: 0;
    }
  </style>
</head>
<body>
  <h1>Hello world</h1>
</body>
</html>`

// Use Effect-style functions with Node.js primitives
const startServer = Effect.gen(function*(_) {
  yield* _(Effect.log("Starting server on http://localhost:3000"))

  // Create a Node.js HTTP server
  const server = createServer((req, res) => {
    // Handle HTTP request
    const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`)
    const path = url.pathname

    if (path === "/") {
      // Serve HTML for root path
      res.writeHead(200, { "Content-Type": "text/html" })
      res.end(htmlContent)
    } else {
      // 404 for all other HTTP paths
      res.writeHead(404)
      res.end("Not Found")
    }
  })

  // WebSocket handling using basic Node.js
  server.on("upgrade", (req, socket, _head) => {
    const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`)

    if (url.pathname === "/ws") {
      // This is a WebSocket upgrade request for /ws path
      const key = req.headers["sec-websocket-key"]

      if (!key) {
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
      socket.write(createTextFrame("Connected to WebSocket"))

      // Handle data from client
      socket.on("data", (buffer) => {
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
            if (data.toString() === "ping") {
              socket.write(createTextFrame("pong"))
            }
          }
        }
      })

      // Handle connection close
      socket.on("end", () => {
        // Connection closed
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
        resolve()
      })
    })
  ))

  yield* _(Effect.log("HTTP server started on http://localhost:3000"))
  yield* _(Effect.log("WebSocket endpoint available at ws://localhost:3000/ws"))

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
