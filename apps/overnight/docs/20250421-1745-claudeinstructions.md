Okay, agent, we need to integrate an AI service (specifically Anthropic Claude) into the `overnight` codebase. This service will act as the "brain" of the coding agent and will use the GitHub tools (`GitHubFileClient`, `GitHubIssueClient`) you previously refactored.

We will base this implementation on patterns observed in a similar project (`oaeffect`), but you **do not** have access to that codebase directly. I will provide the necessary structures, patterns, and code snippets based on that reference project. Follow these instructions *meticulously*.

**Overall Goal:** Create an `AiService` using Effect and `@effect/ai-anthropic`, define tools for the existing GitHub clients, implement a basic streaming HTTP server with SSE to interact with the AI, and build a React frontend to display the interaction.

**Location of Relevant `overnight` Files (Your Current Codebase):**

*   `src/github/Client.ts`
*   `src/github/Errors.ts`
*   `src/github/FileClient.ts`
*   `src/github/IssueClient.ts`
*   `src/Program.ts`
*   `test/github/FileClient.test.ts`
*   `test/github/IssueClient.test.ts`
*   `package.json`
*   `README.md`
*   `CLAUDE.md`

---

**Instructions for Agent:**

**Phase 1: Backend AI Service Setup**

1.  **Add Dependencies:**
    *   Run `pnpm add @effect/ai @effect/ai-anthropic @effect/platform-node`
    *   Run `pnpm add -D @types/node nodemon concurrently` (if not already present, for server development).

2.  **Configure Anthropic Client:**
    *   Modify `src/Program.ts`.
    *   Import necessary modules: `import { AnthropicClient } from "@effect/ai-anthropic"; import { NodeHttpClient } from "@effect/platform-node"; import { Config, Layer } from "effect";`
    *   Define the Anthropic Layer using an API key from environment variables. Add this near the top:
        ```typescript
        // Configure Anthropic with API key from environment variables
        const Anthropic = AnthropicClient.layerConfig({
          apiKey: Config.redacted("ANTHROPIC_API_KEY")
        });

        // Provide HTTP client to Anthropic layer
        const AnthropicWithHttp = Layer.provide(Anthropic, NodeHttpClient.layerUndici);
        ```
    *   Ensure you have an `.env` file in the project root (`apps/overnight/.env`) and add your Anthropic API key:
        ```dotenv
        ANTHROPIC_API_KEY=your_anthropic_api_key_here
        GITHUB_TOKEN=your_github_token_here # Should already exist
        ```
    *   Ensure your `.gitignore` file includes `.env`.

3.  **Define AI Tool Schemas (`src/Tools.ts`):**
    *   Create a new file: `src/Tools.ts`.
    *   Import `Schema` from `effect`.
    *   Define `Schema.TaggedRequest` schemas for *both* existing GitHub operations (fetch file, fetch issue). Use the existing interfaces (`FetchFilePayload`, `GitHubFileContent`, `FetchIssuePayload`, `GitHubIssue`) as a basis for the payload and success types. The `failure` type should probably be `Schema.String` for simplicity in the tool definition, even though our clients return structured errors. We'll handle mapping the structured errors to strings in the tool implementation.
    *   Add descriptive annotations.

        ```typescript
        // src/Tools.ts
        import * as Schema from "@effect/schema/Schema";
        import type { GitHubFileContent } from "./github/FileClient.js"; // Adjust path if needed
        import type { GitHubIssue } from "./github/IssueClient.js"; // Adjust path if needed

        /**
         * Tool Schema: Get GitHub File Content
         */
        export class GetGitHubFileContent extends Schema.TaggedRequest<GetGitHubFileContent>()(
          "GetGitHubFileContent",
          {
            payload: {
              owner: Schema.String.annotations({ description: "The owner of the GitHub repository (user or organization)" }),
              repo: Schema.String.annotations({ description: "The name of the GitHub repository" }),
              path: Schema.String.annotations({ description: "The full path to the file within the repository" }),
              ref: Schema.optional(Schema.String).annotations({ description: "Optional branch, tag, or commit SHA (defaults to default branch)" })
            },
            // Use Schema.Void temporarily if GitHubFileContent is complex; refine later if needed
            // Or define a simplified Schema version of GitHubFileContent here. Let's use String for now.
            success: Schema.String.annotations({ description: "The fetched content of the file as a UTF-8 string." }),
            failure: Schema.String.annotations({ description: "A string describing the reason for failure." })
          },
          {
            description: "Fetches the UTF-8 text content of a specified file from a GitHub repository."
          }
        ) {}

        /**
         * Tool Schema: Get GitHub Issue Details
         */
        export class GetGitHubIssue extends Schema.TaggedRequest<GetGitHubIssue>()(
          "GetGitHubIssue",
          {
            payload: {
              owner: Schema.String.annotations({ description: "The owner of the GitHub repository (user or organization)" }),
              repo: Schema.String.annotations({ description: "The name of the GitHub repository" }),
              issueNumber: Schema.Number.annotations({ description: "The number of the issue to fetch" })
            },
            // Use Schema.Void temporarily if GitHubIssue is complex; refine later if needed
            // Or define a simplified Schema version of GitHubIssue here. Let's use String for now.
            success: Schema.String.annotations({ description: "A summary of the fetched issue details (title, state, body snippet)." }),
            failure: Schema.String.annotations({ description: "A string describing the reason for failure." })
          },
          {
            description: "Fetches details (title, state, body) of a specific issue from a GitHub repository."
          }
        ) {}
        ```

4.  **Implement AI Toolkit (`src/AiService.ts`):**
    *   Create a new file: `src/AiService.ts`.
    *   Import `AiToolkit` from `@effect/ai`, `Effect`, `Layer`, `Console`.
    *   Import the tool schemas (`GetGitHubFileContent`, `GetGitHubIssue`) from `src/Tools.ts`.
    *   Import the *client tags* (`GitHubFileClient`, `GitHubIssueClient`) from `src/github/FileClient.ts` and `src/github/IssueClient.ts`.
    *   Import the *error types* (`FileNotFoundError`, `IssueNotFoundError`, `GitHubApiError`, `RateLimitExceededError`, `HttpError`) from the relevant `src/github/` files.
    *   Define the toolkit by adding both tools:
        ```typescript
        import { AiToolkit } from "@effect/ai";
        import { GetGitHubFileContent, GetGitHubIssue } from "./Tools.js";

        export const GitHubToolkit = AiToolkit.empty
          .add(GetGitHubFileContent)
          .add(GetGitHubIssue);
        ```
    *   Implement the toolkit layer. This layer needs access to the `GitHubFileClient` and `GitHubIssueClient` via context. Map the structured errors from the clients to the simple `failure: Schema.String` defined in the tool schema.
        ```typescript
        // src/AiService.ts (continued)
        import { Effect, Layer, Console } from "effect";
        import { GitHubFileClient, FileNotFoundError } from "./github/FileClient.js";
        import { GitHubIssueClient, IssueNotFoundError } from "./github/IssueClient.js";
        import { GitHubApiError, RateLimitExceededError, HttpError } from "./github/Errors.js";
        import { Buffer } from "node:buffer"; // For Base64 decoding

        // Helper function to stringify errors for the AI tool failure case
        const stringifyError = (error: unknown): string => {
          if (error instanceof FileNotFoundError) return `File not found: ${error.owner}/${error.repo}/${error.path}`;
          if (error instanceof IssueNotFoundError) return `Issue not found: ${error.owner}/${error.repo}#${error.issueNumber}`;
          if (error instanceof RateLimitExceededError) return `GitHub API rate limit exceeded. Resets at ${error.resetAt.toISOString()}`;
          if (error instanceof HttpError) return `GitHub API HTTP Error: ${error.status}`;
          if (error instanceof GitHubApiError) return `GitHub API Error: ${error.message}`;
          if (error instanceof Error) return error.message;
          return String(error);
        }

        export const GitHubToolkitLive = GitHubToolkit.implement((handlers) =>
          Effect.gen(function*() {
            const fileClient = yield* GitHubFileClient;
            const issueClient = yield* GitHubIssueClient;

            return handlers
              .handle("GetGitHubFileContent", (params) =>
                Effect.gen(function*() {
                  yield* Console.log("🛠️ Tool called: GetGitHubFileContent");
                  yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2));
                  const file = yield* fileClient.fetchFile(params);
                  // Decode Base64 content
                  const content = Buffer.from(file.content, "base64").toString("utf-8");
                  yield* Console.log(`✅ Tool result: Content length ${content.length}`);
                  return content; // Return decoded string content
                }).pipe(
                  Effect.catchAll((error) => {
                     const errorString = stringifyError(error);
                     yield* Console.error(`Tool GetGitHubFileContent failed: ${errorString}`);
                     return Effect.fail(errorString); // Fail with string as per schema
                  })
                )
              )
              .handle("GetGitHubIssue", (params) =>
                Effect.gen(function*() {
                  yield* Console.log("🛠️ Tool called: GetGitHubIssue");
                  yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2));
                  const issue = yield* issueClient.fetchIssue(params);
                  // Return a summary string
                  const summary = `Issue #${issue.number}: ${issue.title} (${issue.state})\nBody: ${issue.body.substring(0, 200)}...`;
                  yield* Console.log(`✅ Tool result: ${summary.substring(0, 100)}...`);
                  return summary;
                }).pipe(
                  Effect.catchAll((error) => {
                     const errorString = stringifyError(error);
                     yield* Console.error(`Tool GetGitHubIssue failed: ${errorString}`);
                     return Effect.fail(errorString); // Fail with string as per schema
                  })
                )
              );
          })
        );

        // Layer that provides the Tool Implementation.
        // It requires the layers for the actual GitHub clients.
        export const AiServiceLive = Layer.provide(
            GitHubToolkitLive,
            Layer.merge(githubFileClientLayer, githubIssueClientLayer) // Assumes these are exported from client files
        );
        ```
        *Self-Correction:* Ensure `githubFileClientLayer` and `githubIssueClientLayer` are exported from `FileClient.ts` and `IssueClient.ts`. If not, add exports: `export const githubFileClientLayer = ...;`

5.  **Update Program Entry Point (`src/Program.ts` - temporarily):**
    *   Modify `src/Program.ts` to *temporarily* test the AI service directly (we'll move this to the server later).
    *   Import `Completions` from `@effect/ai`, `GitHubToolkit` and `AiServiceLive` from `src/AiService.ts`.
    *   Import the base GitHub layer `defaultGitHubLayer` from `src/github/Client.ts`.
    *   Remove the previous test code that directly called `GitHubFileClient` and `GitHubIssueClient`.
    *   Create a simple `main` Effect that uses `Completions.toolkitStream` with the `GitHubToolkit`.
    *   Provide all necessary layers: `AnthropicWithHttp`, `AiServiceLive`, and `defaultGitHubLayer`.

        ```typescript
        // src/Program.ts (Temporary Test Setup)
        import { AnthropicClient, Completions } from "@effect/ai-anthropic";
        import { NodeHttpClient } from "@effect/platform-node";
        import { Config, Console, Effect, Layer, Stream } from "effect";
        import { GitHubToolkit, AiServiceLive } from "./AiService.js"; // Adjust path
        import { defaultGitHubLayer } from "./github/Client.js"; // Adjust path

        // Anthropic Config (already defined earlier)
        const Anthropic = AnthropicClient.layerConfig({ apiKey: Config.redacted("ANTHROPIC_API_KEY") });
        const AnthropicWithHttp = Layer.provide(Anthropic, NodeHttpClient.layerUndici);

        // --- Main Program ---
        const main = Effect.gen(function*() {
          const completions = yield* Completions;
          const prompt = "Please fetch the content of the README.md file from the effect-ts/effect repository.";
          // const prompt = "Get details for issue number 1 in the openagentsinc/openagents repository."; // Alternate prompt

          yield* Console.log(`User: ${prompt}`);

          const resultStream = completions.toolkitStream(GitHubToolkit, {
            messages: [{ role: "user", content: prompt }],
            model: "claude-3-haiku-20240307",
            // Add system prompt if desired
             system: "You are a helpful assistant. Use tools when necessary. You can fetch GitHub files and issues."
          });

          yield* Console.log("Assistant:");

          // Process the stream
          yield* Stream.runForEach(resultStream, (event) =>
            Effect.gen(function* () {
              switch (event._tag) {
                case "ToolResult": {
                  yield* Console.log(`\n[Tool Result (${event.toolName}): ${JSON.stringify(event.result).substring(0, 200)}...]`);
                  break;
                }
                case "ToolCall": {
                  yield* Console.log(`\n[Tool Call: ${event.toolName} with params ${JSON.stringify(event.params)}]`);
                  break;
                }
                case "Text": {
                  process.stdout.write(event.text); // Write text chunk directly to stdout
                  break;
                }
                case "Error": {
                  yield* Console.error(`\n[AI Error: ${event.error}]`);
                  break;
                }
                case "Finish": {
                  yield* Console.log(`\n[Finish Reason: ${event.reason}]`);
                  break;
                }
              }
            })
          );

          yield* Console.log("\n--- End of Conversation ---");

        });

        // --- Layer Composition ---
        const runnable = Effect.provide(
          main,
          Layer.mergeAll(
            AnthropicWithHttp,
            AiServiceLive,
            defaultGitHubLayer // Provides GitHubHttpExecutor and base config
          )
        );

        // --- Run ---
        Effect.runPromise(runnable).catch(console.error);
        ```
    *   Run `pnpm dev` (you might need to adjust the `dev` script in `package.json` to run `src/Program.ts` via tsx or build first). Verify that Claude responds and attempts to call the correct GitHub tool based on the prompt. Check the console logs for `Tool called:` messages. Fix any errors in `AiService.ts` or `Tools.ts`.

**Phase 2: Backend Server & SSE Implementation**

1.  **Create Server File (`src/Server.ts`):**
    *   Create `src/Server.ts`.
    *   Add basic Node.js HTTP server setup using `node:http`.
    *   Include SSE connection management (copy relevant snippets from `oaeffect`'s `src/Server.ts` provided in the prompt):
        *   `clients` Map.
        *   `lastClientId` variable.
        *   `broadcastSSE` function (modify to send JSON).
        *   `sseHandler` function.
    *   Add a basic request handler that routes `/sse` to `sseHandler` and serves a placeholder 404 for other routes for now.
    *   Export a `startServer` function.

        ```typescript
        // src/Server.ts (Initial Setup)
        import * as Http from "node:http";
        import { Effect, Layer, Runtime, Scope, Console, Exit } from "effect";
        import { NodeHttpServer } from "@effect/platform-node";

        // --- SSE Management ---
        const clients = new Map<string, Http.ServerResponse>();
        let lastClientId = 0;

        // Modify broadcastSSE to send JSON
        const broadcastSSE = (event: string, data: unknown) => {
          const messageData = JSON.stringify(data);
          const message = `event: ${event}\ndata: ${messageData}\n\n`;
          // Using Console.log within Effect if possible, otherwise standard console
          console.log(`Broadcasting ${event} event to ${clients.size} clients`);
          for (const client of clients.values()) {
            try {
              client.write(message);
            } catch (e) {
              console.error("Failed to write to SSE client:", e);
              // Optionally remove client if write fails
            }
          }
        };

        const sseHandler = (req: Http.IncomingMessage, res: Http.ServerResponse) => {
          console.log("New SSE connection established");
          res.writeHead(200, {
            "Content-Type": "text/event-stream",
            "Cache-Control": "no-cache",
            Connection: "keep-alive",
            "Access-Control-Allow-Origin": "*", // Allow CORS for dev
          });
          res.write(`event: connected\ndata: ${JSON.stringify({ message: "SSE connection established" })}\n\n`);

          const clientId = (++lastClientId).toString();
          clients.set(clientId, res);
          console.log(`Client ${clientId} connected, total clients: ${clients.size}`);

          req.on("close", () => {
            console.log(`Client ${clientId} disconnected`);
            clients.delete(clientId);
          });

          res.on("error", (err) => {
            console.error(`SSE client ${clientId} error: ${err.message}`);
            clients.delete(clientId);
          });
        };

        // --- HTTP Request Handler ---
        const requestHandler = (req: Http.IncomingMessage, res: Http.ServerResponse) => {
          const url = req.url || "/";
          console.log(`Request received: ${req.method} ${url}`);

          // Add CORS headers for development
          res.setHeader("Access-Control-Allow-Origin", "*");
          res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
          res.setHeader("Access-Control-Allow-Headers", "Content-Type");

          if (req.method === "OPTIONS") {
            res.writeHead(204);
            res.end();
            return;
          }

          if (url === "/sse") {
            sseHandler(req, res);
            return;
          }

          // Placeholder for /chat endpoint
          if (url === "/chat" && req.method === "POST") {
            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify({ success: true, message: "Chat endpoint placeholder" }));
            // TODO: Implement chat logic here
            return;
          }

          // Placeholder 404
          res.writeHead(404, { "Content-Type": "text/plain" });
          res.end("Not Found");
        };

        // --- Effect Server ---
        // We'll use the Node native server directly for simplicity now,
        // based on the reference code provided. Effect platform integration can be complex.
        export const startServer = (): void => {
          const server = Http.createServer(requestHandler);
          const port = 3000; // Or from Config

          server.listen(port, () => {
            console.log(`Server started on http://localhost:${port}`);
          });

          server.on("error", (err) => {
            console.error(`Server error: ${err.message}`);
          });
        };

        // Start server if run directly
        if (import.meta.url === `file://${process.argv[1]}`) {
          startServer();
        }
        ```

2.  **Modify Program Entry Point (`src/Program.ts`):**
    *   Change `src/Program.ts` to import and call `startServer` instead of running the test Effect directly.
    *   Keep the `AnthropicWithHttp` layer definition, as the server will need it.

        ```typescript
        // src/Program.ts (Updated Entry Point)
        import { AnthropicClient } from "@effect/ai-anthropic";
        import { NodeHttpClient } from "@effect/platform-node";
        import { Config, Layer } from "effect";
        import { startServer } from "./Server.js"; // Import the server start function

        // Anthropic Config (remains the same)
        const Anthropic = AnthropicClient.layerConfig({ apiKey: Config.redacted("ANTHROPIC_API_KEY") });
        const AnthropicWithHttp = Layer.provide(Anthropic, NodeHttpClient.layerUndici);

        // --- Start the Server ---
        // The server will internally set up the AI interaction flow later
        // For now, we just start it.
        startServer();

        // We might pass the necessary Effect layers (like AnthropicWithHttp)
        // to the server function later if needed, or use Runtime.
        console.log("Server process started. Waiting for server to listen...");
        ```

3.  **Implement Chat Endpoint Logic (`src/Server.ts`):**
    *   Modify the `/chat` POST handler in `src/Server.ts`.
    *   Parse the incoming message (assume JSON body: `{ "message": "user query" }`).
    *   Import `Completions`, `GitHubToolkit`, `AiServiceLive`, `defaultGitHubLayer`, `AnthropicWithHttp`.
    *   Create an Effect `Runtime` to run the AI interaction Effect within the async Node.js request handler.
    *   Define the full Effect pipeline for the AI interaction (`Completions.toolkitStream(...)`) similar to the temporary test in Phase 1, Step 5.
    *   Instead of logging stream events to console, use `broadcastSSE` to send JSON-formatted events (`user-message`, `ai-thinking`, `ai-response`, `tool-call`, `tool-result`, `error`) to connected clients. Define a clear JSON structure for these events (see `oaeffect` examples).
    *   Remember to manage conversation history (add user message, add final assistant message). The reference `oaeffect` code has examples.

        ```typescript
        // src/Server.ts (Adding Chat Logic - requires significant integration)

        // ... (Imports for Effect, Layers, AI, Tools, GitHub Clients etc.)
        import { AnthropicClient, Completions } from "@effect/ai-anthropic";
        import { NodeHttpClient } from "@effect/platform-node";
        import { Config, Layer, Effect, Runtime, Scope, Console, Exit, Stream } from "effect";
        import { GitHubToolkit, AiServiceLive } from "./AiService.js";
        import { defaultGitHubLayer } from "./github/Client.js";

        // ... (SSE Management code remains the same) ...

        // --- Conversation History ---
        interface Message { role: "user" | "assistant"; content: string; }
        const conversation: Message[] = []; // Simple in-memory history

        // --- Effect Runtime Setup ---
        // Define the full layer needed for AI interaction
        const AiLayer = Layer.mergeAll(
          AnthropicClient.layerConfig({ apiKey: Config.redacted("ANTHROPIC_API_KEY") }).pipe(
             Layer.provide(NodeHttpClient.layerUndici)
          ),
          AiServiceLive, // Provides tool implementation + underlying GitHub client layers
          defaultGitHubLayer // Provides GitHubHttpExecutor + base config
        );

        // Create a Runtime with the required layer
        const AiRuntime = Layer.toRuntime(AiLayer).pipe(
          Effect.scoped // Scope the runtime
        );

        // Keep track of the scope to release it later if needed
        let runtimeScope: Scope.Scope | null = null;
        const getRuntime = Effect.runPromise(Effect.scoped(AiRuntime)).then(rt => {
            runtimeScope = rt.scope; // Store the scope
            return rt.runtime;
        }).catch(e => {
            console.error("Failed to initialize Effect Runtime:", e);
            process.exit(1);
        });

        process.on('exit', () => {
            if (runtimeScope) {
                console.log("Closing runtime scope...");
                Effect.runPromise(Scope.close(runtimeScope, Exit.unit));
            }
        });

        // --- Stream Chat Response using Effect Runtime ---
        const streamChatResponseViaEffect = async (userMessage: string) => {
          const runtime = await getRuntime; // Get the initialized runtime

          broadcastSSE("ai-thinking", { type: "thinking", content: "Thinking...", id: `thinking-${Date.now()}` });

          conversation.push({ role: "user", content: userMessage });
          const currentConversation = [...conversation]; // Copy for this request

          const aiEffect = Effect.gen(function*() {
            const completions = yield* Completions;
            const resultStream = completions.toolkitStream(GitHubToolkit, {
              messages: currentConversation,
              model: "claude-3-haiku-20240307",
              system: "You are a helpful coding assistant. Use tools to fetch GitHub files or issues when requested."
            });

            let assistantResponse = "";
            const aiMessageId = `ai-${Date.now()}`;

            // Send initial streaming message placeholder
            broadcastSSE("ai-message", { id: aiMessageId, type: "ai", status: "streaming", content: "", timestamp: Date.now() });

            yield* Stream.runForEach(resultStream, (event) => Effect.sync(() => { // Use Effect.sync for safety
              switch (event._tag) {
                case "ToolResult":
                  broadcastSSE("tool-result", { toolName: event.toolName, result: event.result });
                  break;
                case "ToolCall":
                  broadcastSSE("tool-call", { toolName: event.toolName, params: event.params });
                  break;
                case "Text":
                  assistantResponse += event.text;
                  // Send streaming update
                  broadcastSSE("ai-response", { id: aiMessageId, type: "ai", status: "streaming", content: assistantResponse, timestamp: Date.now() });
                  break;
                case "Error":
                  console.error(`AI Error: ${event.error}`);
                  broadcastSSE("error", { type: "error", error: `AI Error: ${event.error}`, id: `error-${Date.now()}` });
                  break;
                case "Finish":
                   console.log(`AI Finish Reason: ${event.reason}`);
                   // Send final complete message
                   broadcastSSE("ai-response", { id: aiMessageId, type: "ai", status: "complete", content: assistantResponse, timestamp: Date.now() });
                   // Add final response to history *only if* it wasn't an error finish
                   if (event.reason !== "error") {
                       conversation.push({ role: "assistant", content: assistantResponse });
                   }
                   break;
              }
            }));
          });

          // Run the effect using the runtime
          Runtime.runPromise(runtime)(aiEffect).then(() => {
              // Remove thinking message after stream completes or errors
              broadcastSSE("ai-thinking", { type: "thinking", content: "", id: `thinking-${Date.now()}` }); // Send empty to clear
          }).catch(error => {
              console.error("Error running AI effect:", error);
              broadcastSSE("ai-thinking", { type: "thinking", content: "", id: `thinking-${Date.now()}` }); // Send empty to clear
              broadcastSSE("error", { type: "error", error: `Core AI processing error: ${stringifyError(error)}`, id: `error-${Date.now()}` });
          });
        };


        // --- HTTP Request Handler (Updated Chat Endpoint) ---
        const requestHandler = (req: Http.IncomingMessage, res: Http.ServerResponse) => {
          // ... (CORS and /sse handling) ...

          if (url === "/chat" && req.method === "POST") {
            let body = "";
            req.on("data", chunk => body += chunk.toString());
            req.on("end", async () => {
              try {
                const jsonData = JSON.parse(body);
                const userMessage = jsonData.message;

                if (!userMessage || typeof userMessage !== 'string' || userMessage.trim() === "") {
                  res.writeHead(400, { "Content-Type": "application/json" });
                  res.end(JSON.stringify({ success: false, error: "Invalid or empty message" }));
                  return;
                }

                console.log(`Received chat message: ${userMessage}`);

                // Acknowledge request immediately
                res.writeHead(200, { "Content-Type": "application/json" });
                res.end(JSON.stringify({ success: true }));

                // Broadcast user message via SSE
                broadcastSSE("user-message", { type: "user", content: userMessage, id: `user-${Date.now()}`, timestamp: Date.now() });

                // Trigger AI response stream (don't await here)
                streamChatResponseViaEffect(userMessage);

              } catch (err) {
                console.error(`Error processing chat request: ${err}`);
                // Avoid sending 500 if headers already sent
                if (!res.headersSent) {
                    res.writeHead(500, { "Content-Type": "application/json" });
                    res.end(JSON.stringify({ success: false, error: "Error processing message" }));
                }
              }
            });
            return;
          }

          // ... (Static file serving & 404 handling) ...
        };

        // ... (startServer function remains similar, just uses the updated requestHandler) ...
        ```

4.  **Update Package Scripts:**
    *   Add/modify scripts in `package.json` for running the server:
        ```json
        "scripts": {
          // ... existing scripts ...
          "serve": "node build/esm/Server.js", // Run the built server
          "dev:watch-server": "nodemon --watch src --ext ts --exec \"pnpm build-esm && node build/esm/Server.js\"", // Watch and restart server
          "dev": "tsx watch src/Server.ts" // Use tsx for faster dev startup
        },
        ```
    *   Run `pnpm dev` and test sending a POST request to `http://localhost:3000/chat` (e.g., using `curl` or Postman) with a JSON body like `{"message": "get readme for effect-ts/effect"}`. Monitor the server console output.

**Phase 3: Frontend Implementation**

1.  **Set up React Project (Vite):**
    *   If not already done, create the frontend directory: `mkdir frontend && cd frontend`
    *   Run `pnpm create vite . --template react-ts` (use `.` to create in current dir).
    *   Run `pnpm install`.
    *   Configure `frontend/vite.config.ts` to proxy `/chat` and `/sse` requests to `http://localhost:3000`. (See `oaeffect` snippet).
    *   Add alias for shared types: `'shared-types': resolve(__dirname, '../packages/shared-types')` (requires creating the shared package next).

2.  **Create Shared Types Package:**
    *   In the *root* (`apps/overnight`), create `packages/shared-types`.
    *   Add an `index.ts` file: `packages/shared-types/index.ts`.
    *   Define shared interfaces/enums for `Message` types (`UserMessage`, `AiMessage`, `ThinkingMessage`, `ErrorMessage`) and `SseEvent` enum. Copy structure from `oaeffect` snippets.
    *   Add a basic `package.json` for this package:
        ```json
        // packages/shared-types/package.json
        {
          "name": "shared-types",
          "version": "1.0.0",
          "main": "index.ts",
          "types": "index.ts"
        }
        ```
    *   Update the root `pnpm-workspace.yaml` (create if it doesn't exist) to include the new package:
        ```yaml
        # pnpm-workspace.yaml
        packages:
          - '.' # Include the main package
          - 'frontend'
          - 'packages/*'
        ```
    *   Run `pnpm install` in the root directory (`apps/overnight`) again to link the workspace packages.

3.  **Develop React Components:**
    *   In `frontend/src/`, create `components/` directory.
    *   Implement `ChatApp.tsx` (main container), `ChatHistory.tsx`, `ChatMessage.tsx`, `ChatInput.tsx`. Use the structures from the `oaeffect` frontend snippets as a guide.
    *   Import types from `shared-types`.
    *   Style using CSS modules (e.g., `ChatMessage.module.css`) or basic CSS (`App.css`).

4.  **Implement SSE Hook (`frontend/src/hooks/useSSE.ts`):**
    *   Create `frontend/src/hooks/useSSE.ts`.
    *   Implement the custom hook to manage the `EventSource` connection, handle events, manage connection state, and include basic reconnection logic. Copy the structure from the `oaeffect` `useSSE.ts` snippet.
    *   Import `SseEvent` from `shared-types`.

5.  **Integrate SSE in `App.tsx`:**
    *   Use the `useSSE` hook in `frontend/src/App.tsx`.
    *   Implement the `onMessage` handler within `useSSE` options to process incoming JSON events (`user-message`, `ai-thinking`, `ai-response`, etc.) based on their `type` property.
    *   Update the `messages` state based on these events (add new messages, update streaming messages, remove thinking indicators).

6.  **Implement Message Sending:**
    *   In `App.tsx`, create a function `sendMessage` that takes the user input string.
    *   This function should make a `POST` request to `/chat` with a JSON body `{ "message": message }`.
    *   Pass this function down to the `ChatInput` component.

7.  **Configure Static Serving (`src/Server.ts`):**
    *   Update `src/Server.ts` to serve static files from the React build output (`frontend/dist`).
    *   Add logic to serve `frontend/dist/index.html` for the root path (`/`) and potentially for any unknown paths to support client-side routing if added later. Use `node:fs` and `node:path`. (See `oaeffect` server snippet for static file serving example).

8.  **Update Package Scripts:**
    *   Ensure root `package.json` has scripts to build the UI and run both frontend and backend concurrently for development:
        ```json
        "scripts": {
          // ... existing backend scripts ...
          "build:ui": "pnpm --filter frontend build",
          "dev:ui": "pnpm --filter frontend dev",
          "build": "pnpm build:ui && pnpm build-esm", // Build UI then backend
          "serve": "node build/esm/Server.js",
          "dev": "tsx watch src/Server.ts", // Run backend with tsx watch
          "dev:all": "concurrently \"pnpm dev\" \"pnpm dev:ui\"" // Run both concurrently
        },
        ```

9.  **Run and Test:**
    *   Run `pnpm dev:all`.
    *   Open `http://localhost:5173` (Vite dev server port).
    *   Test sending messages and verify:
        *   User messages appear immediately.
        *   Thinking indicator appears.
        *   AI responses stream in correctly.
        *   GitHub tools are invoked when appropriate (check server logs).
        *   Errors are displayed gracefully.

**Phase 4: Testing**

1.  **Backend AI Service/Tool Tests (`test/AiService.test.ts`):**
    *   Create `test/AiService.test.ts`.
    *   Test the `GitHubToolkitLive` layer implementation.
    *   Mock the `GitHubFileClient` and `GitHubIssueClient` layers.
    *   Verify that the tool handlers correctly call the underlying client methods with the right parameters.
    *   Test the error mapping from structured client errors to the tool's string failure case.

2.  **Backend Server Tests (`test/Server.test.ts`):**
    *   Create `test/Server.test.ts`.
    *   Test the `/chat` endpoint: Mock the `AiRuntime` or `streamChatResponseViaEffect` function. Verify it receives the message, sends the correct initial response, and triggers the AI stream.
    *   Test the `/sse` endpoint: Simulate client connections and disconnections, verify correct headers are set. (More complex to test full SSE broadcast without a client).

3.  **Frontend Tests (`frontend/src/`...):**
    *   Set up Vitest within the `frontend` package (`pnpm --filter frontend add -D vitest @testing-library/react @testing-library/jest-dom jsdom`). Configure `frontend/vitest.config.ts`.
    *   Write unit tests for components (`ChatMessage`, `ChatInput`).
    *   Write integration tests for `ChatHistory` and `App.tsx`.
    *   Test the `useSSE` hook, potentially mocking `EventSource`.

**Phase 5: Documentation & Cleanup**

1.  **Update `README.md`:** Add instructions on setting up environment variables (`ANTHROPIC_API_KEY`), installing dependencies (`pnpm install`), building (`pnpm build`), and running the full application (`pnpm serve` or `pnpm dev:all`).
2.  **Update `copyToClipboard.js`:** Add all newly created/modified files from `src/`, `test/`, `frontend/`, `packages/` to the script.
3.  **Review Code:** Ensure consistency, remove console logs used for debugging, add comments where necessary.
4.  **Update `CLAUDE.md`:** Add notes about the new AI service, server, and frontend structure if relevant for future interactions.

---

This is a large task. Proceed step-by-step through the phases. Verify each phase before moving to the next. Pay close attention to Effect Layer composition, Runtime usage in the server, and the SSE message formats between backend and frontend. Good luck!
