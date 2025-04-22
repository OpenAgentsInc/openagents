// src/Program.ts

import { NodeContext, NodeFileSystem, NodeHttpServer, NodeRuntime } from "@effect/platform-node"
import * as FileSystem from "@effect/platform/FileSystem"
import * as Headers from "@effect/platform/Headers"
import * as HttpRouter from "@effect/platform/HttpRouter"
import * as HttpServer from "@effect/platform/HttpServer"
import * as HttpServerRequest from "@effect/platform/HttpServerRequest"
import * as HttpServerResponse from "@effect/platform/HttpServerResponse"
import type { Scope } from "effect"
import { Effect, Layer, Stream } from "effect"
import * as Cause from "effect/Cause"
import { createServer } from "node:http"
import type * as Net from "node:net"
import * as path from "node:path"

// Simple logging functions
const log = (message: string) => console.log(`[${new Date().toISOString()}] ${message}`)
const error = (message: string) => console.error(`[${new Date().toISOString()}] ERROR: ${message}`)

// Use process.cwd() which is generally more reliable
const publicDir = path.join(process.cwd(), "public")
log(`Public directory path: ${publicDir}`)

// --- HTTP Handlers ---

// Effect to serve index.html
const serveIndex: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Errors are mapped to success
  FileSystem.FileSystem // Requires FileSystem
> = Effect.gen(function*(_) {
  log("Serving index.html")
  const fs = yield* _(FileSystem.FileSystem)
  const indexPath = path.join(publicDir, "index.html")

  // Use Effect.tryPromise to have effect-based error handling
  return yield* _(
    fs.readFileString(indexPath, "utf8").pipe(
      Effect.map((content) => HttpServerResponse.html(content)),
      Effect.catchAll((e) => {
        if (e._tag === "SystemError" && e.reason === "NotFound") {
          error(`Index file not found: ${indexPath}`)
          return Effect.succeed(HttpServerResponse.empty({ status: 404 }))
        }
        error(`Filesystem error (Index): ${e._tag === "SystemError" ? e.reason + " " + e.message : JSON.stringify(e)}`)
        return Effect.succeed(HttpServerResponse.empty({ status: 500 }))
      })
    )
  )
})

// Effect to serve static files
const serveStaticFile: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Errors are mapped to success
  FileSystem.FileSystem | HttpServerRequest.HttpServerRequest // Requires FileSystem and Request
> = Effect.gen(function*(_) {
  const request = yield* _(HttpServerRequest.HttpServerRequest)
  const fs = yield* _(FileSystem.FileSystem)

  const urlPath = new URL(request.url).pathname
  if (urlPath.includes("..")) {
    log(`Directory traversal attempt blocked: ${urlPath}`)
    return HttpServerResponse.empty({ status: 400 })
  }
  const filePath = path.join(publicDir, urlPath)
  log(`Attempting to serve static file: ${filePath}`)

  const ext = path.extname(filePath)
  let contentType = "application/octet-stream"
  if (ext === ".css") contentType = "text/css"
  else if (ext === ".js") contentType = "text/javascript"
  else if (ext === ".woff") contentType = "font/woff"
  else if (ext === ".woff2") contentType = "font/woff2"
  else if (ext === ".html") contentType = "text/html"

  return yield* _(
    fs.readFile(filePath).pipe(
      Effect.map((content) =>
        HttpServerResponse.raw(content, {
          headers: Headers.set(Headers.empty, "Content-Type", contentType)
        })
      ),
      Effect.catchAll((e) => {
        if (e._tag === "SystemError" && e.reason === "NotFound") {
          log(`Static file not found: ${filePath}`)
          return Effect.succeed(HttpServerResponse.empty({ status: 404 }))
        }
        error(`Filesystem error (Static): ${e._tag === "SystemError" ? e.reason + " " + e.message : JSON.stringify(e)}`)
        return Effect.succeed(HttpServerResponse.empty({ status: 500 }))
      })
    )
  )
})

// WebSocket Handling Logic (using the example pattern - Needs HttpServerRequest Service)
const handleWebSocketStream: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Errors logged within, returns success (empty response)
  Scope.Scope | HttpServerRequest.HttpServerRequest // Required by upgradeChannel & request access
> = Effect.gen(function*(_) {
  log("Handling WebSocket stream request")
  const request = yield* _(HttpServerRequest.HttpServerRequest)
  const upgradeHeader = request.headers["upgrade"]

  if (upgradeHeader?.toLowerCase() !== "websocket") {
    log("Request to /ws without WebSocket upgrade header")
    return HttpServerResponse.empty({ status: 400 })
  }

  log("WebSocket upgrade detected, attempting channel upgrade...")

  const connectionStatusHtml =
    `<div id="connection-status" hx-swap-oob="true" class="font-medium text-green-500">Connected</div>`
  const agentStatusHtml = `<div id="agent-status" hx-swap-oob="true" class="font-medium text-green-500">Ready</div>`

  const outgoingStream = Stream.make(connectionStatusHtml, agentStatusHtml).pipe(
    Stream.encodeText
  )

  return yield* _(
    outgoingStream.pipe(
      Stream.pipeThroughChannel(HttpServerRequest.upgradeChannel()),
      Stream.runForEach((chunk) => Effect.sync(() => log(`WebSocket Received Chunk (raw): ${chunk}`))),
      Effect.map(() => {
        log("WebSocket stream processing finished.")
        return HttpServerResponse.empty()
      }),
      // Catch all possible errors to make the error type 'never'
      Effect.catchAll((err) => {
        error(`WebSocket stream processing error: ${JSON.stringify(err)}`)
        // Return an empty response - the upgrade has likely already happened
        return Effect.succeed(HttpServerResponse.empty())
      })
    )
  )
})

// --- Router Definition ---

// Create the router, combining routes.
const httpRouter = HttpRouter.empty.pipe(
  HttpRouter.get("/", serveIndex),
  // WebSocket handler maps errors to success, so its E is never.
  HttpRouter.get("/ws", handleWebSocketStream),
  HttpRouter.get("/:path*", serveStaticFile)
)

// --- Combine Layers ---

// Define implementations
const ServerLive = NodeHttpServer.layer(
  () => createServer(),
  { port: 3000 } as Net.ListenOptions
)

const FileSystemLive = NodeFileSystem.layer

// Define the main HTTP application layer by serving the router
// HttpServer.serve() returns a function that takes the router and returns a Layer
const HttpAppLive = HttpServer.serve()(httpRouter).pipe(
  Layer.provide(ServerLive)
)

// Define the final layer stack for the application
const AppLayer = Layer.mergeAll(
  FileSystemLive, // Provide FileSystem
  NodeContext.layer // Provide base Node context
).pipe(
  Layer.provide(HttpAppLive) // Provide HTTP server with router
)

// --- Main Execution ---

// The main effect launches the server by running the entire layer stack
// The `: void` type annotation forces the return type to be void
const main: Effect.Effect<void, Error, never> = Effect.gen(function*(_) {
  yield* _(Layer.launch(AppLayer))
  // This will never be reached as the server runs forever,
  // but it ensures the return type is void
}).pipe(
  // Catch server startup errors
  Effect.catchAll((serveErr) => {
    error(`Server failed to start: ${JSON.stringify(serveErr)}`)
    // Map the error to a failure in the Effect's error channel
    return Effect.fail(new Error("Server startup failed"))
  }),
  // Add final logging for runtime errors/defects after layers are provided
  Effect.tapErrorCause((cause) =>
    Effect.sync(() => {
      if (Cause.isInterruptedOnly(cause)) {
        log(`Server interrupted - Shutting down.`)
      } else {
        error(`Unhandled runtime error/defect:\n${Cause.pretty(cause)}`)
      }
    })
  )
) // Final type should be Effect<void, Error, never>

// Run the main effect using the NodeRuntime
log("Starting server runtime...")
// The 'main' effect now has E = Error, R = never, satisfying runMain's input constraint
NodeRuntime.runMain(main)
