// src/Program.ts

import * as HttpServer from "@effect/platform/HttpServer";
import * as HttpRouter from "@effect/platform/HttpRouter";
import * as HttpServerRequest from "@effect/platform/HttpServerRequest";
import * as HttpServerResponse from "@effect/platform/HttpServerResponse";
import { NodeHttpServer, NodeRuntime, NodeContext, NodeFileSystem } from "@effect/platform-node";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Headers from "@effect/platform/Headers";
import { Effect, Layer, Stream, Scope } from "effect";
import * as path from "node:path";
import { createServer } from "node:http";
import type * as Net from "node:net";
import * as Cause from "effect/Cause";

// Simple logging functions
const log = (message: string) => console.log(`[${new Date().toISOString()}] ${message}`);
const error = (message: string) => console.error(`[${new Date().toISOString()}] ERROR: ${message}`);

// Use process.cwd() which is generally more reliable
const publicDir = path.join(process.cwd(), "public");
log(`Public directory path: ${publicDir}`);

// --- HTTP Handlers ---

// Effect to serve index.html
const serveIndex: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Errors are mapped to success
  FileSystem.FileSystem // Requires FileSystem
> = Effect.gen(function* (_) {
  log("Serving index.html");
  const fs = yield* _(FileSystem.FileSystem);
  const indexPath = path.join(publicDir, "index.html");

  // Explicitly return the Effect, satisfying the declared type
  const responseEffect = fs.readFileString(indexPath, "utf8").pipe(
    Effect.map(content => HttpServerResponse.html(content)),
    Effect.catchTag("SystemError", (e) => { // Catch specific file system errors
      if (e.reason === "NotFound") {
        error(`Index file not found: ${indexPath}`);
        return Effect.succeed(HttpServerResponse.empty({ status: 404 }));
      }
      error(`Filesystem error (Index): ${e.reason} ${e.message}`);
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    }),
    // Catch any other potential errors to guarantee E=never
    Effect.catchAll((unhandledError) => {
      error(`Unhandled error reading index: ${JSON.stringify(unhandledError)}`);
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    })
  );
  return yield* _(responseEffect);
});

// Effect to serve static files
const serveStaticFile: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Errors are mapped to success
  FileSystem.FileSystem | HttpServerRequest.HttpServerRequest // Requires FileSystem and Request
> = Effect.gen(function* (_) {
  const request = yield* _(HttpServerRequest.HttpServerRequest);
  const fs = yield* _(FileSystem.FileSystem);

  const urlPath = new URL(request.url).pathname;
  if (urlPath.includes("..")) {
    log(`Directory traversal attempt blocked: ${urlPath}`);
    return Effect.succeed(HttpServerResponse.empty({ status: 400 })); // Return an Effect
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
    Effect.catchTag("SystemError", (e) => { // Catch specific file system errors
      if (e.reason === "NotFound") {
        log(`Static file not found: ${filePath}`);
        return Effect.succeed(HttpServerResponse.empty({ status: 404 }));
      }
      error(`Filesystem error (Static): ${e.reason} ${e.message}`);
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    }),
    // Catch any other potential errors to guarantee E=never
    Effect.catchAll((unhandledError) => {
      error(`Unhandled error reading static file ${filePath}: ${JSON.stringify(unhandledError)}`);
      return Effect.succeed(HttpServerResponse.empty({ status: 500 }));
    })
  );
  return yield* _(responseEffect);
});

// WebSocket Handling Logic (using the example pattern - Needs HttpServerRequest Service)
const handleWebSocketStream: Effect.Effect<
  HttpServerResponse.HttpServerResponse,
  never, // Errors logged within, returns success (empty response)
  Scope.Scope | HttpServerRequest.HttpServerRequest // Required by upgradeChannel & request access
> = Effect.gen(function* (_) {
  log("Handling WebSocket stream request");
  const request = yield* _(HttpServerRequest.HttpServerRequest);
  const upgradeHeader = request.headers["upgrade"];

  if (upgradeHeader?.toLowerCase() !== "websocket") {
    log("Request to /ws without WebSocket upgrade header");
    return Effect.succeed(HttpServerResponse.empty({ status: 400 })); // Return an Effect
  }

  log("WebSocket upgrade detected, attempting channel upgrade...");

  const connectionStatusHtml = `<div id="connection-status" hx-swap-oob="true" class="font-medium text-green-500">Connected</div>`;
  const agentStatusHtml = `<div id="agent-status" hx-swap-oob="true" class="font-medium text-green-500">Ready</div>`;

  const outgoingStream = Stream.make(connectionStatusHtml, agentStatusHtml).pipe(
    Stream.encodeText
  );

  // Use Effect.absolve to turn Either<E, A> from upgradeChannel into Effect<A, E>
  const streamEffect = outgoingStream.pipe(
    Stream.pipeThroughChannel(HttpServerRequest.upgradeChannel()), // Requires Scope & Request
    Stream.runForEach((chunk) =>
      Effect.sync(() => log(`WebSocket Received Chunk (raw): ${chunk}`))
    ),
    // Handle errors during stream processing gracefully to ensure E=never
    Effect.catchAll((err) =>
      Effect.sync(() => error(`WebSocket stream processing error: ${JSON.stringify(err)}`))
    )
  );

  // Must return an effect yielding a response
  // We run the stream effect and then return an empty response
  // Use catchAll again on the wrapper effect to guarantee E=never
  const responseEffect = streamEffect.pipe(
    Effect.map(() => {
      log("WebSocket stream processing finished.");
      return HttpServerResponse.empty(); // Return empty response on success
    }),
    Effect.catchAll((err) => {
      error(`Error running WebSocket stream effect: ${JSON.stringify(err)}`);
      // Still return a successful response (empty 500?) or just empty?
      // Let's return empty for now as the upgrade likely already happened.
      return Effect.succeed(HttpServerResponse.empty());
    })
  );
  return yield* _(responseEffect);
}); // Type check: returns Effect<Response, never, Scope | Request>


// --- Router Definition ---

// Create the router, combining routes.
const httpRouter = HttpRouter.empty.pipe(
  HttpRouter.get("/", serveIndex),
  // WebSocket handler maps errors to success, so its E is never.
  HttpRouter.get("/ws", handleWebSocketStream),
  HttpRouter.get("/:path*", serveStaticFile)
);

// --- Combine Layers ---

// Define the main HTTP application layer by serving the router
// HttpServer.serve returns the Effect<void, ServeError, HttpServer | RouterEnv>
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
// Correctly apply Effect.provide: provide AppLayer TO HttpAppServeEffect
const main: Effect.Effect<void, Error, never> = Effect.provide(
  HttpAppServeEffect, // The effect needing the layer
  AppLayer           // The layer providing implementations
).pipe(
  // The result should now be Effect<void, ServeError, never>
  // Catch server startup errors
  Effect.catchAll((serveErr) => {
    error(`Server failed to start: ${JSON.stringify(serveErr)}`);
    // Map the error to a failure in the Effect's error channel
    // Use Effect.die to represent fatal startup errors if preferred
    return Effect.fail(new Error("Server startup failed"));
  }),
  // Add final logging for runtime errors/defects after layers are provided
  Effect.tapErrorCause((cause) => Effect.sync(() => {
    if (Cause.isInterruptedOnly(cause)) {
      log(`Server interrupted - Shutting down.`);
    } else {
      error(`Unhandled runtime error/defect:\n${Cause.pretty(cause)}`);
    }
  }))
); // Final type should be Effect<void, Error, never>


// Run the main effect using the NodeRuntime
log("Starting server runtime...");
// The 'main' effect now has E = Error, R = never, satisfying runMain's input constraint
NodeRuntime.runMain(main);
