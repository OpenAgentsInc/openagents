// src/Program.ts

import * as HttpServer from "@effect/platform/HttpServer"; // Use main HttpServer import
import * as HttpRouter from "@effect/platform/HttpRouter";
import * as HttpServerRequest from "@effect/platform/HttpServerRequest";
import * as HttpServerResponse from "@effect/platform/HttpServerResponse";
import { NodeHttpServer, NodeRuntime, NodeContext, NodeFileSystem } from "@effect/platform-node"; // Import specific Node modules
import * as FileSystem from "@effect/platform/FileSystem";
// import * as Socket from "@effect/platform/Socket"; // Keep for WebSocket types // Unused
import * as Headers from "@effect/platform/Headers";
// import * as PlatformError from "@effect/platform/Error"; // Unused
import { Effect, Layer, Stream, Scope } from "effect"; // Import needed Effect types // Removed unused: Schedule, Console, Queue, Fiber
import * as path from "node:path";
import { createServer } from "node:http"; // Needed for NodeHttpServer layer
import type * as Net from "node:net"; // Needed for NodeHttpServer layer options type
import * as Cause from "effect/Cause"; // Import Cause for detailed error logging

// Simple logging functions (can be replaced with Effect Console/Logger later)
const log = (message: string) => console.log(`[${new Date().toISOString()}] ${message}`);
const error = (message: string) => console.error(`[${new Date().toISOString()}] ERROR: ${message}`);

// Use process.cwd() which is generally more reliable
const publicDir = path.join(process.cwd(), "public");
log(`Public directory path: ${publicDir}`);

// --- WebSocket Handler (Commented out - unused) ---
/*
// NOTE: This handler is currently unused because HttpRouter.upgrade and
// HttpServerResponse.websocket seem unavailable in this Effect Platform version.
// Keeping the definition for future reference if WebSocket support is re-enabled.
const handleSocket = (socket: Socket.Socket): Effect.Effect<void, Socket.SocketError, Scope.Scope> =>
  Effect.gen(function*(_) {
    log("Client connected via Effect Socket, sending initial status");
    const writer = yield* _(socket.writer);

    const connectionStatusHtml = `<div id="connection-status" hx-swap-oob="true" class="font-medium text-green-500">Connected</div>`;
    yield* _(writer(connectionStatusHtml));

    const agentStatusHtml = `<div id="agent-status" hx-swap-oob="true" class="font-medium text-green-500">Ready</div>`;
    yield* _(writer(agentStatusHtml));

    yield* _(socket.runRaw((message) =>
      Effect.sync(() => {
        log(`Received WebSocket message: ${typeof message === "string" ? message : `<Uint8Array length=${message.length}>`}`);
      })
    ));
    log("WebSocket runRaw completed (socket closed by peer).");
  }).pipe(
    Effect.scoped
  );
*/


// --- HTTP Handlers ---

// Effect to serve index.html
// Maps errors to successful responses, so Error channel is never
const serveIndex: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Error type is never because we map errors to successful responses
  FileSystem.FileSystem // Requires FileSystem
> = Effect.gen(function* (_) {
  log("Serving index.html");
  const fs = yield* _(FileSystem.FileSystem);
  const indexPath = path.join(publicDir, "index.html");

  // Explicitly return the Effect, satisfying the declared type
  const responseEffect = fs.readFileString(indexPath, "utf8").pipe(
    Effect.map(content => HttpServerResponse.html(content)),
    Effect.catchTag("SystemError", (e) => {
      if (e.reason === "NotFound") {
        error(`Index file not found: ${indexPath}`);
        return Effect.succeed(HttpServerResponse.empty({ status: 404 }));
      }
      error(`Filesystem error (Index): ${e.reason} ${e.message}`);
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    })
    // catchAll ensures E = never
    // Effect.catchAll((unhandledError) => {
    //    error(`Unhandled error reading index: ${JSON.stringify(unhandledError)}`);
    //    return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    // })
  );
  return yield* _(responseEffect);
}); // Type check: returns Effect<Response, never, FS>

// Effect to serve static files
// Maps errors to successful responses, so Error channel is never
const serveStaticFile: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Error type is never because we map errors to successful responses
  FileSystem.FileSystem | HttpServerRequest.HttpServerRequest // Requires FileSystem and Request
> = Effect.gen(function* (_) {
  const request = yield* _(HttpServerRequest.HttpServerRequest);
  const fs = yield* _(FileSystem.FileSystem);

  const urlPath = new URL(request.url).pathname;
  if (urlPath.includes("..")) {
    log(`Directory traversal attempt blocked: ${urlPath}`);
    // Return an effect yielding the response
    return Effect.succeed(HttpServerResponse.empty({ status: 400 }));
  }
  const filePath = path.join(publicDir, urlPath);
  log(`Attempting to serve static file: ${filePath}`);

  const ext = path.extname(filePath);
  let contentType = "application/octet-stream";
  if (ext === ".css") contentType = "text/css";
  else if (ext === ".js") contentType = "text/javascript";
  else if (ext === ".woff") contentType = "font/woff";
  else if (ext === ".woff2") contentType = "font/woff2";
  else if (ext === ".html") contentType = "text/html";

  // Explicitly return the Effect, satisfying the declared type
  const responseEffect = fs.readFile(filePath).pipe(
    Effect.map(content =>
      HttpServerResponse.raw(content, {
        headers: Headers.set(Headers.empty, "Content-Type", contentType)
      })
    ),
    Effect.catchTag("SystemError", (e) => {
      if (e.reason === "NotFound") {
        log(`Static file not found: ${filePath}`);
        return Effect.succeed(HttpServerResponse.empty({ status: 404 }));
      }
      error(`Filesystem error (Static): ${e.reason} ${e.message}`);
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    })
    // catchAll ensures E = never
    // Effect.catchAll((unhandledError) => {
    //    error(`Unhandled error reading static file ${filePath}: ${JSON.stringify(unhandledError)}`);
    //    return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    // })
  );
  return yield* _(responseEffect);
}); // Type check: returns Effect<Response, never, FS | Request>

// WebSocket Handling Logic (using the example pattern - Needs HttpServerRequest Service)
const handleWebSocketStream: Effect.Effect<
  HttpServerResponse.HttpServerResponse, // Returns an empty response after handling stream
  never, // Errors logged within, returns success (empty response) or fails if upgrade fails
  Scope.Scope | HttpServerRequest.HttpServerRequest // Required by upgradeChannel & request access
> = Effect.gen(function* (_) {
  log("Handling WebSocket stream request");
  const request = yield* _(HttpServerRequest.HttpServerRequest); // Need request context
  const upgradeHeader = request.headers["upgrade"];

  if (upgradeHeader?.toLowerCase() !== "websocket") {
    log("Request to /ws without WebSocket upgrade header");
    // Return failure Effect mapped to a success yielding 400 response
    return Effect.succeed(HttpServerResponse.empty({ status: 400 }));
  }

  log("WebSocket upgrade detected, attempting channel upgrade...");

  // Create a simple outgoing stream for initial messages
  const connectionStatusHtml = `<div id="connection-status" hx-swap-oob="true" class="font-medium text-green-500">Connected</div>`;
  const agentStatusHtml = `<div id="agent-status" hx-swap-oob="true" class="font-medium text-green-500">Ready</div>`;

  const outgoingStream = Stream.make(connectionStatusHtml, agentStatusHtml).pipe(
    Stream.encodeText
  );

  // Process the WebSocket stream using the upgradeChannel accessed via HttpServerRequest
  // Use Effect.absolve to turn Either<E, A> from upgradeChannel into Effect<A, E>
  const streamEffect = outgoingStream.pipe(
    // Use HttpServerRequest.upgradeChannel (assuming this is the correct API location)
    Stream.pipeThroughChannel(HttpServerRequest.upgradeChannel), // Requires Scope & Request
    Stream.runForEach((chunk) => // Process incoming chunks (likely Uint8Array)
      Effect.sync(() => log(`WebSocket Received Chunk (raw): ${chunk}`))
      // Add Stream.decodeText() before runForEach if string is desired
    ),
    // Handle errors during stream processing
    Effect.catchAll((err) =>
      Effect.sync(() => error(`WebSocket stream error: ${JSON.stringify(err)}`))
    )
  );

  // Must return an effect yielding a response
  // We run the stream effect and then return an empty response
  return yield* _(streamEffect.pipe(
    Effect.map(() => {
      log("WebSocket stream processing finished.");
      return HttpServerResponse.empty(); // Return empty response on success
    }),
    // If streamEffect itself fails *after* upgrade, we might log but still return empty
    Effect.catchAll((err) => {
      error(`Error processing WebSocket stream after upgrade: ${JSON.stringify(err)}`);
      return Effect.succeed(HttpServerResponse.empty()); // Or maybe a 500 status? Hard to say.
    })
  ));
}); // Type check: returns Effect<Response, never, Scope | Request>


// --- Router Definition ---

// Create the router, combining routes.
const httpRouter = HttpRouter.empty.pipe(
  HttpRouter.get("/", serveIndex),
  // WebSocket handler needs Scope and Request. Router provides Request. Scope needs to come from server layer / launch.
  HttpRouter.get("/ws", handleWebSocketStream),
  HttpRouter.get("/:path*", serveStaticFile)
);

// --- Combine Layers ---

// Define the main HTTP application layer by serving the router
// The `serve` function itself likely returns the Effect<void, ServeError, HttpServer | RouterEnv>
// where RouterEnv is the combined R of all routes (FS | Request | Scope)
const HttpAppServeEffect = HttpServer.serve(httpRouter);

// Define the final layer stack for the application
// This layer provides the implementations needed by HttpAppServeEffect
const AppLayer = Layer.mergeAll(
  NodeContext.layer, // Provides Node runtime context
  NodeHttpServer.layer( // Provides HttpServer implementation
    () => createServer(),
    { port: 3000 } as Net.ListenOptions
  ),
  NodeFileSystem.layer // Provides FileSystem implementation
);


// --- Main Execution ---

// The main effect launches the server within the provided layers
const main: Effect.Effect<void, Error, never> = HttpAppServeEffect.pipe(
  Effect.provide(AppLayer), // Provide the implementations (FS, HttpServer, Context)
  // The result should now be Effect<void, ServeError, never>
  // Catch server startup errors
  Effect.catchAll((serveErr) => {
    error(`Server failed to start: ${JSON.stringify(serveErr)}`);
    // Map the error to a failure in the Effect's error channel
    return Effect.fail(new Error("Server startup failed"));
  }),
  // Add final logging for runtime errors/defects after layers are provided
  Effect.tapErrorCause((cause) => Effect.sync(() => {
    // Using Cause.pretty provides more detail than JSON.stringify
    if (Cause.isInterruptedOnly(cause)) {
      log(`Server interrupted - Shutting down.`);
    } else {
      error(`Unhandled runtime error/defect:\n${Cause.pretty(cause)}`);
    }
  }))
); // Final type should be Effect<void, Error, never>


// Run the main effect using the NodeRuntime
log("Starting server runtime...");
NodeRuntime.runMain(main);
