// src/Program.ts

import * as HttpServer from "@effect/platform/HttpServer";
import * as HttpRouter from "@effect/platform/HttpRouter";
import * as HttpServerRequest from "@effect/platform/HttpServerRequest";
import * as HttpServerResponse from "@effect/platform/HttpServerResponse";
import * as NodeContext from "@effect/platform-node/NodeContext";
import * as NodeHttpServer from "@effect/platform-node/NodeHttpServer";
import * as NodeFileSystem from "@effect/platform-node/NodeFileSystem";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Socket from "@effect/platform/Socket";
import * as Headers from "@effect/platform/Headers";
import * as PlatformError from "@effect/platform/Error";
import * as Effect from "effect/Effect";
import * as Layer from "effect/Layer";
import * as Scope from "effect/Scope";
import * as path from "node:path";

// Simple logging functions
const log = (message: string) => console.log(`[${new Date().toISOString()}] ${message}`);
const error = (message: string) => console.error(`[${new Date().toISOString()}] ERROR: ${message}`);

// Use process.cwd() which is generally more reliable
const publicDir = path.join(process.cwd(), "public");
log(`Public directory path: ${publicDir}`);

// --- WebSocket Handler ---

// NOTE: This function requires Scope in its environment because socket.writer requires Scope.
// The Router/Server must handle providing this Scope when the WebSocket connects.
const handleSocket = (socket: Socket.Socket): Effect.Effect<void, Socket.SocketError, Scope.Scope> =>
  Effect.gen(function* (_) {
    log("Client connected via Effect Socket, sending initial status");
    // socket.writer itself returns an Effect<writerFn, _, Scope>
    // We yield it to get the actual writer function within the scope
    const writer = yield* _(socket.writer);

    // Send initial status messages
    const connectionStatusHtml = `<div id="connection-status" hx-swap-oob="true" class="font-medium text-green-500">Connected</div>`;
    // If sending initial messages can fail, we might want to handle errors here
    yield* _(writer(connectionStatusHtml));

    const agentStatusHtml = `<div id="agent-status" hx-swap-oob="true" class="font-medium text-green-500">Ready</div>`;
    yield* _(writer(agentStatusHtml));

    // Process incoming messages (just log them for now)
    // runRaw handles string | Uint8Array and runs indefinitely until socket closes or errors
    // Its error type is SocketError, which we propagate. Its success type is void.
    yield* _(socket.runRaw((message) =>
      Effect.sync(() => {
        log(`Received WebSocket message: ${typeof message === "string" ? message : `<Uint8Array length=${message.length}>`}`);
        // Handle client messages here if needed in the future
      })
    ));
    // When runRaw completes successfully (socket closed by other side cleanly), it yields void.
    // The generator implicitly returns void here.
  }).pipe(
    // Note: We are letting SocketError propagate from runRaw or writer
    // We catch them higher up if needed, or let the server handle them.
    // If we wanted to map errors to `never` here, we'd use catchAll.
    // Effect.catchAll((err) => { ... return Effect.void })
    Effect.scoped // Ensures the scope (and the writer) is handled correctly
  );


// --- HTTP Handlers ---

// Effect to serve index.html
// Requires FileSystem in its environment
const serveIndex: Effect.Effect<
  HttpServerResponse.HttpServerResponse, // Success type
  PlatformError.PlatformError,           // Error type (can fail with PlatformError)
  FileSystem.FileSystem                  // Required environment
> = Effect.gen(function* (_) {
  log("Serving index.html");
  const fs = yield* _(FileSystem.FileSystem); // Access FileSystem service
  const indexPath = path.join(publicDir, "index.html");

  // Attempt to read the file, map success to HTML response, map known errors to HTTP responses
  return yield* _(fs.readFileString(indexPath, "utf8").pipe(
    Effect.map(content => HttpServerResponse.html(content)), // On success, create HTML response
    Effect.catchTag("SystemError", (e) => { // Handle specific filesystem errors
      if (e.reason === "NotFound") {
        error(`Index file not found: ${indexPath}`);
        // Map error to a successful effect yielding a 404 response
        return Effect.succeed(HttpServerResponse.empty({ status: 404 }));
      }
      // For other system errors, return a 500 and log
      error(`Filesystem error (Index): ${e.reason} ${e.message}`);
      // Map error to a successful effect yielding a 500 response
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    })
    // If other PlatformErrors can occur and need specific handling, add catchTag here.
    // Otherwise, they will propagate up.
  ));
});

// Effect to serve static files
// Requires FileSystem and HttpServerRequest in its environment
const serveStaticFile: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  PlatformError.PlatformError, // Can fail with PlatformError
  FileSystem.FileSystem | HttpServerRequest.HttpServerRequest // Required environment
> = Effect.gen(function* (_) {
  const request = yield* _(HttpServerRequest.HttpServerRequest); // Access the request
  const fs = yield* _(FileSystem.FileSystem); // Access FileSystem service

  const urlPath = new URL(request.url).pathname;
  // Basic security: prevent directory traversal
  if (urlPath.includes("..")) {
    log(`Directory traversal attempt blocked: ${urlPath}`);
    return HttpServerResponse.empty({ status: 400 });
  }
  const filePath = path.join(publicDir, urlPath);
  log(`Attempting to serve static file: ${filePath}`);

  // Determine content type based on extension
  const ext = path.extname(filePath);
  let contentType = "application/octet-stream"; // Default
  if (ext === ".css") contentType = "text/css";
  else if (ext === ".js") contentType = "text/javascript";
  else if (ext === ".woff") contentType = "font/woff";
  else if (ext === ".woff2") contentType = "font/woff2";
  else if (ext === ".html") contentType = "text/html"; // Serve other HTML files too

  // Attempt to read the file, map success to raw response, map known errors to HTTP responses
  return yield* _(fs.readFile(filePath).pipe( // Read as Uint8Array for raw serving
    Effect.map(content =>
      HttpServerResponse.raw(content, {
        headers: Headers.set(Headers.empty, "Content-Type", contentType)
      })
    ),
    Effect.catchTag("SystemError", (e) => {
      if (e.reason === "NotFound") {
        log(`Static file not found: ${filePath}`);
        // Map error to a successful effect yielding a 404 response
        return Effect.succeed(HttpServerResponse.empty({ status: 404 }));
      }
      error(`Filesystem error (Static): ${e.reason} ${e.message}`);
      // Map error to a successful effect yielding a 500 response
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    })
    // If other PlatformErrors can occur and need specific handling, add catchTag here.
    // Otherwise, they will propagate up.
  ));
});

// --- Router Definition ---

// Create the router, combining routes.
// The resulting router requires the union of requirements from its handlers.
// NOTE: HttpRouter.upgrade does not seem to exist in this version based on errors.
// WebSocket handling needs an alternative approach, perhaps via HttpServerResponse.websocket
// or specific server configuration if available. Temporarily commenting out WS route.
const httpRouter = HttpRouter.empty.pipe(
  HttpRouter.get("/", serveIndex),                    // Handle index route
  HttpRouter.get("/:path*", serveStaticFile)         // Handle static files (must come after specific routes)
  // HttpRouter.upgrade("/ws", (socket: Socket.Socket) => handleSocket) // Requires `upgrade` on HttpRouter
  // --- Alternative WebSocket Handling (if HttpServerResponse.websocket exists) ---
  /*
  HttpRouter.get("/ws", Effect.gen(function*(_) { // Match /ws path
      const request = yield* _(HttpServerRequest.HttpServerRequest);
      const upgradeHeader = request.headers["upgrade"];
      if (upgradeHeader?.toLowerCase() === "websocket") {
          log("WebSocket upgrade requested via GET handler");
          // This function needs to exist and handle the upgrade protocol
          return HttpServerResponse.websocket(handleSocket);
      }
      // Not a websocket upgrade request for this path
      return HttpServerResponse.empty({ status: 400 });
  }))
  */
);

// --- Server and Main Program ---

// The main application effect
const program = Effect.gen(function* (_) {
  yield* _(Effect.log("Server starting..."));

  // Create the server effect using the router
  // Use HttpServer.serve and pipe it into HttpServer.listen
  const serverEffect = HttpServer.serve(httpRouter).pipe(
    HttpServer.listen({ port: 3000 }) // Apply listen options here
    // HttpServer.withLogAddress might not exist or work this way, handle logging manually if needed
  );

  yield* _(Effect.log("HTTP server configured, starting listener..."));

  // Run the server listener effect
  // This Effect conceptually runs forever or until an error occurs during startup
  yield* _(serverEffect);

  // If listen returns the server instance instead of running forever, we might need Effect.never here
  // yield* _(Effect.log("Server listening. Running forever..."));
  // yield* _(Effect.never);

}).pipe(
  // Provide all necessary layers for the server and handlers to run
  // The environment R becomes never after providing layers
  Effect.provide(
    Layer.mergeAll(
      NodeContext.layer,        // Provides Node runtime context
      NodeHttpServer.layer,     // Provides HttpServer implementation for Node
      NodeFileSystem.layer      // Provides FileSystem implementation for Node
    )
    // Add other layers like Logging, Config, etc. if needed
  ),
  // Add final logging for top-level errors
  Effect.tapErrorCause((cause) => Effect.sync(() => {
    error(`Unhandled top-level error: ${JSON.stringify(cause, null, 2)}`);
  }))
);

// --- Run the Application ---
log("Starting server runtime...");
Effect.runPromise(program)
  .then(() => log("Server runtime finished cleanly (this usually indicates an issue if server should run forever)."))
  .catch((err) => {
    // runPromise catches defects and logs them by default
    // This catch is for errors deliberately propagated in the error channel ('E')
    error(`Server runtime failed with error: ${JSON.stringify(err)}`);
    process.exit(1); // Exit with error code
  });
