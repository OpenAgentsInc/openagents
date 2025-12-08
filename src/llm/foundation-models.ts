/**
 * Apple Foundation Models client for local LLM inference.
 *
 * Uses a Swift HTTP bridge that provides an OpenAI-compatible API at /v1/chat/completions.
 * The bridge wraps Apple's Foundation Models framework (macOS 26+).
 *
 * Features:
 *   - On-demand auto-start: Server starts automatically if not running
 *   - OpenAI-compatible API: Same interface as Ollama
 *   - Graceful degradation: Falls back to other providers if unavailable
 *
 * Usage:
 *   const client = createFMClient();
 *   const response = await Effect.runPromise(client.chat({ messages: [...] }));
 */

import { Effect, Context, Layer, Schedule } from "effect";
import type { ChatRequest, ChatResponse, ChatMessage, ChatToolCall } from "./openrouter-types.js";
import { existsSync, writeFileSync, unlinkSync, readFileSync } from "fs";
import { join } from "path";
import { homedir, tmpdir } from "os";

// --- Configuration ---

export interface FMConfig {
  /** Server port (default: 11435) */
  port: number;
  /** Path to foundation-bridge binary (auto-detected if not specified) */
  bridgePath?: string;
  /** Request timeout in ms (default: 300000 = 5 minutes) */
  timeoutMs?: number;
  /** Auto-start server if not running (default: true) */
  autoStart?: boolean;
}

export const DEFAULT_FM_PORT = 11435;
export const DEFAULT_FM_TIMEOUT_MS = 300_000;

// Default paths to search for the bridge binary
const DEFAULT_BRIDGE_PATHS = [
  join(process.cwd(), "bin", "foundation-bridge"),
  join(process.cwd(), "swift", "foundation-bridge", ".build", "release", "foundation-bridge"),
  join(homedir(), ".local", "bin", "foundation-bridge"),
  "/usr/local/bin/foundation-bridge",
  "/opt/homebrew/bin/foundation-bridge",
];

// Lock file for singleton server access
const FM_LOCK_FILE = join(tmpdir(), "fm-bridge.lock");
const LOCK_TIMEOUT_MS = 30000; // 30 seconds max wait for lock
const LOCK_STALE_MS = 60000; // Consider lock stale after 60 seconds

/**
 * Acquire exclusive lock for FM bridge operations.
 * Returns true if lock acquired, false if timeout.
 */
const acquireLock = (): Effect.Effect<boolean, never> =>
  Effect.gen(function* () {
    const startTime = Date.now();
    const pid = process.pid;

    while (Date.now() - startTime < LOCK_TIMEOUT_MS) {
      // Check if lock exists
      if (existsSync(FM_LOCK_FILE)) {
        // Read lock to check if stale
        try {
          const lockData = readFileSync(FM_LOCK_FILE, "utf-8");
          const lockInfo = JSON.parse(lockData) as { pid: number; timestamp: number };

          // Check if lock is stale (older than LOCK_STALE_MS)
          if (Date.now() - lockInfo.timestamp > LOCK_STALE_MS) {
            // Stale lock, remove it
            try {
              unlinkSync(FM_LOCK_FILE);
            } catch {
              // Ignore removal errors
            }
          } else {
            // Lock is held, wait and retry
            yield* Effect.sleep("100 millis");
            continue;
          }
        } catch {
          // Invalid lock file, remove it
          try {
            unlinkSync(FM_LOCK_FILE);
          } catch {
            // Ignore removal errors
          }
        }
      }

      // Try to create lock
      try {
        writeFileSync(FM_LOCK_FILE, JSON.stringify({ pid, timestamp: Date.now() }), { flag: "wx" });
        return true;
      } catch {
        // Lock creation failed (race condition), retry
        yield* Effect.sleep("100 millis");
      }
    }

    return false; // Timeout
  });

/**
 * Release the FM bridge lock.
 */
const releaseLock = (): Effect.Effect<void, never> =>
  Effect.sync(() => {
    try {
      unlinkSync(FM_LOCK_FILE);
    } catch {
      // Ignore errors (lock may not exist or be owned by another process)
    }
  });

// --- Error Types ---

export class FMError extends Error {
  readonly _tag = "FMError";
  constructor(
    readonly reason:
      | "not_macos"
      | "bridge_not_found"
      | "server_not_running"
      | "model_unavailable"
      | "request_failed"
      | "invalid_response"
      | "timeout",
    message: string,
    readonly statusCode?: number,
  ) {
    super(message);
    this.name = "FMError";
  }
}

// --- API Types ---

interface FMApiResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: {
      role: "assistant";
      content: string | null;
      tool_calls?: Array<{
        id: string;
        type: "function";
        function: {
          name: string;
          arguments: string;
        };
      }>;
    };
    finish_reason: string;
  }>;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

interface FMHealthResponse {
  status: string;
  model_available: boolean;
  version: string;
  platform: string;
}

interface FMErrorResponse {
  error: {
    message: string;
    type: string;
    code?: string;
  };
}

interface FMModelApiResponse {
  id: string;
  object: "model";
  created: number;
  owned_by: string;
}

interface FMModelsApiResponse {
  object: "list";
  data: FMModelApiResponse[];
}

// --- Discovery ---

/**
 * Check if running on macOS.
 */
export const isMacOS = (): boolean => process.platform === "darwin";

/**
 * Find the foundation-bridge binary in default locations.
 */
export const findBridgePath = (): string | null => {
  // Check environment variable first
  const envPath = process.env.FM_BRIDGE_PATH;
  if (envPath && existsSync(envPath)) {
    return envPath;
  }

  for (const path of DEFAULT_BRIDGE_PATHS) {
    if (existsSync(path)) {
      return path;
    }
  }
  return null;
};

/**
 * Health check result.
 */
export interface FMHealthResult {
  available: boolean;
  serverRunning: boolean;
  modelAvailable: boolean;
  version?: string;
  error?: string;
}

/**
 * Check Foundation Models availability.
 */
export const checkFMHealth = (
  port = DEFAULT_FM_PORT,
): Effect.Effect<FMHealthResult, FMError> =>
  Effect.gen(function* () {
    // Platform check
    if (!isMacOS()) {
      return {
        available: false,
        serverRunning: false,
        modelAvailable: false,
        error: "Foundation Models requires macOS",
      };
    }

    // Try health endpoint
    const response = yield* Effect.tryPromise({
      try: async () => {
        const resp = await fetch(`http://localhost:${port}/health`, {
          signal: AbortSignal.timeout(5000),
        });
        if (!resp.ok) {
          throw new Error(`HTTP ${resp.status}`);
        }
        return resp.json() as Promise<FMHealthResponse>;
      },
      catch: (e) =>
        new FMError(
          "server_not_running",
          `Foundation Models server not running on port ${port}: ${e instanceof Error ? e.message : String(e)}`,
        ),
    });

    const result: FMHealthResult = {
      available: response.model_available,
      serverRunning: true,
      modelAvailable: response.model_available,
      version: response.version,
    };
    if (!response.model_available) {
      result.error = "Model not available on this device";
    }
    return result;
  });

// --- Server Auto-Start ---

/**
 * Ensure the Foundation Models server is running.
 * If not running, attempts to start it using the bridge binary.
 * Uses file-based locking to prevent concurrent server starts.
 */
export const ensureServerRunning = (
  config: Partial<FMConfig> = {},
): Effect.Effect<void, FMError> =>
  Effect.gen(function* () {
    const port = config.port ?? DEFAULT_FM_PORT;

    // Quick check before acquiring lock (optimization)
    const quickHealth = yield* checkFMHealth(port).pipe(
      Effect.catchAll(() => Effect.succeed({ available: false, serverRunning: false, modelAvailable: false } as FMHealthResult)),
    );

    if (quickHealth.serverRunning) {
      return;
    }

    // Acquire exclusive lock to prevent concurrent server starts
    const lockAcquired = yield* acquireLock();
    if (!lockAcquired) {
      // Timeout waiting for lock - another process may be starting the server
      // Check if server is now running (started by another process)
      const retryHealth = yield* checkFMHealth(port).pipe(
        Effect.catchAll(() => Effect.succeed({ available: false, serverRunning: false, modelAvailable: false } as FMHealthResult)),
      );
      if (retryHealth.serverRunning) {
        return;
      }
      return yield* Effect.fail(
        new FMError("timeout", "Timeout waiting for FM bridge lock - another process may be stuck"),
      );
    }

    // Re-check after acquiring lock (another process may have started it)
    const health = yield* checkFMHealth(port).pipe(
      Effect.catchAll(() => Effect.succeed({ available: false, serverRunning: false, modelAvailable: false } as FMHealthResult)),
    );

    if (health.serverRunning) {
      yield* releaseLock();
      return;
    }

    // Find bridge binary
    const bridgePath = config.bridgePath ?? findBridgePath();
    if (!bridgePath) {
      yield* releaseLock();
      return yield* Effect.fail(
        new FMError(
          "bridge_not_found",
          "foundation-bridge binary not found. Build with: cd swift/foundation-bridge && ./build.sh",
        ),
      );
    }

    // Start server in background
    console.log(`Starting Foundation Models server on port ${port}...`);
    Bun.spawn([bridgePath, String(port)], {
      stdout: "inherit",
      stderr: "inherit",
    });

    // Wait for server to become ready
    const schedule = Schedule.recurs(10).pipe(
      Schedule.addDelay(() => "500 millis"),
    );

    const startResult = yield* Effect.retry(
      checkFMHealth(port).pipe(
        Effect.flatMap((h) =>
          h.serverRunning
            ? Effect.succeed(undefined)
            : Effect.fail(new FMError("server_not_running", "Server not ready yet")),
        ),
      ),
      schedule,
    ).pipe(
      Effect.map(() => true),
      Effect.catchAll(() => Effect.succeed(false)),
    );

    // Release lock after server startup (or failure)
    yield* releaseLock();

    if (!startResult) {
      return yield* Effect.fail(
        new FMError("server_not_running", `Server failed to start after 5 seconds`),
      );
    }

    console.log("Foundation Models server started successfully");
  });

// --- Conversion Functions ---

const convertMessage = (msg: ChatMessage): { role: string; content: string } => {
  const content =
    typeof msg.content === "string"
      ? msg.content
      : msg.content.map((block) => (block.type === "text" ? block.text : "[image]")).join("\n");

  return { role: msg.role, content };
};

const convertResponse = (resp: FMApiResponse): ChatResponse => {
  const choice = resp.choices[0];
  const toolCalls: ChatToolCall[] | undefined = choice?.message.tool_calls?.map((tc) => ({
    id: tc.id,
    name: tc.function.name,
    arguments: tc.function.arguments,
  }));

  const result: ChatResponse = {
    id: resp.id,
    choices: [
      {
        message: {
          role: "assistant",
          content: choice?.message.content ?? null,
          ...(toolCalls?.length ? { tool_calls: toolCalls } : {}),
        },
      },
    ],
  };
  if (resp.usage) {
    result.usage = {
      prompt_tokens: resp.usage.prompt_tokens,
      completion_tokens: resp.usage.completion_tokens,
      total_tokens: resp.usage.total_tokens,
    };
  }
  return result;
};

// --- Client Implementation ---

/**
 * Models list result type.
 */
export interface FMModelsResult {
  object: "list";
  data: Array<{
    id: string;
    object: "model";
    created: number;
    owned_by: string;
  }>;
}

export interface FMClient {
  readonly config: FMConfig;
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, FMError>;
  listModels: () => Effect.Effect<FMModelsResult, FMError>;
}

export class FMClientTag extends Context.Tag("FMClient")<FMClientTag, FMClient>() {}

/**
 * Create a Foundation Models client for chat completions.
 */
export const createFMClient = (config: Partial<FMConfig> = {}): FMClient => {
  const port = config.port ?? DEFAULT_FM_PORT;
  const timeoutMs = config.timeoutMs ?? DEFAULT_FM_TIMEOUT_MS;
  const autoStart = config.autoStart ?? true;
  const foundBridgePath = config.bridgePath ?? findBridgePath();

  const fullConfig: FMConfig = {
    port,
    timeoutMs,
    autoStart,
  };
  if (foundBridgePath) {
    fullConfig.bridgePath = foundBridgePath;
  }

  const chat = (request: ChatRequest): Effect.Effect<ChatResponse, FMError> =>
    Effect.gen(function* () {
      // Platform check
      if (!isMacOS()) {
        return yield* Effect.fail(
          new FMError("not_macos", "Foundation Models requires macOS"),
        );
      }

      // Auto-start server if enabled
      if (autoStart) {
        yield* ensureServerRunning(fullConfig).pipe(
          Effect.catchAll((e) => {
            // If auto-start fails, try anyway in case server started externally
            console.warn(`Auto-start warning: ${e.message}`);
            return Effect.succeed(undefined);
          }),
        );
      }

      const url = `http://localhost:${port}/v1/chat/completions`;

      const apiRequest = {
        model: request.model ?? "apple-foundation-model",
        messages: request.messages.map(convertMessage),
        stream: false,
        ...(request.temperature !== undefined ? { temperature: request.temperature } : {}),
        ...(request.maxTokens !== undefined ? { max_tokens: request.maxTokens } : {}),
        ...(request.responseFormat !== undefined ? { response_format: request.responseFormat } : {}),
      };

      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

      try {
        const response = yield* Effect.tryPromise({
          try: async () => {
            const resp = await fetch(url, {
              method: "POST",
              headers: {
                "Content-Type": "application/json",
              },
              body: JSON.stringify(apiRequest),
              signal: controller.signal,
            });

            const data = await resp.json();

            if (!resp.ok) {
              const errorData = data as FMErrorResponse;
              throw new FMError(
                errorData.error?.code === "model_unavailable" ? "model_unavailable" : "request_failed",
                errorData.error?.message ?? `HTTP ${resp.status}`,
                resp.status,
              );
            }

            return data as FMApiResponse;
          },
          catch: (e) => {
            if (e instanceof FMError) return e;
            if (e instanceof Error && e.name === "AbortError") {
              return new FMError("timeout", `Request timed out after ${timeoutMs}ms`);
            }
            if (e instanceof TypeError && e.message.includes("fetch")) {
              return new FMError(
                "server_not_running",
                `Failed to connect to Foundation Models server at port ${port}`,
              );
            }
            return new FMError(
              "request_failed",
              `Foundation Models request failed: ${e instanceof Error ? e.message : String(e)}`,
            );
          },
        });

        clearTimeout(timeoutId);

        // Validate response structure
        if (!response.choices?.length) {
          return yield* Effect.fail(
            new FMError("invalid_response", "Foundation Models response has no choices"),
          );
        }

        return convertResponse(response);
      } finally {
        clearTimeout(timeoutId);
      }
    });

  const listModels = (): Effect.Effect<FMModelsResult, FMError> =>
    Effect.gen(function* () {
      // Platform check
      if (!isMacOS()) {
        return yield* Effect.fail(
          new FMError("not_macos", "Foundation Models requires macOS"),
        );
      }

      // Auto-start server if enabled
      if (autoStart) {
        yield* ensureServerRunning(fullConfig).pipe(
          Effect.catchAll((e) => {
            console.warn(`Auto-start warning: ${e.message}`);
            return Effect.succeed(undefined);
          }),
        );
      }

      const url = `http://localhost:${port}/v1/models`;

      const response = yield* Effect.tryPromise({
        try: async () => {
          const resp = await fetch(url, {
            method: "GET",
            headers: {
              "Content-Type": "application/json",
            },
            signal: AbortSignal.timeout(5000),
          });

          const data = await resp.json();

          if (!resp.ok) {
            const errorData = data as FMErrorResponse;
            throw new FMError(
              "request_failed",
              errorData.error?.message ?? `HTTP ${resp.status}`,
              resp.status,
            );
          }

          return data as FMModelsApiResponse;
        },
        catch: (e) => {
          if (e instanceof FMError) return e;
          if (e instanceof TypeError && e.message.includes("fetch")) {
            return new FMError(
              "server_not_running",
              `Failed to connect to Foundation Models server at port ${port}`,
            );
          }
          return new FMError(
            "request_failed",
            `Foundation Models request failed: ${e instanceof Error ? e.message : String(e)}`,
          );
        },
      });

      return response;
    });

  return {
    config: fullConfig,
    chat,
    listModels,
  };
};

/**
 * Create a Layer that provides FMClient from config.
 */
export const fmClientLayer = (config: Partial<FMConfig> = {}): Layer.Layer<FMClientTag> =>
  Layer.succeed(FMClientTag, createFMClient(config));

// --- CLI Demo ---

if (import.meta.main) {
  const args = process.argv.slice(2);

  if (args.includes("--health") || args.includes("-h")) {
    // Health check mode
    const result = await Effect.runPromise(
      checkFMHealth().pipe(
        Effect.catchAll((e) => Effect.succeed({ available: false, serverRunning: false, modelAvailable: false, error: e.message } as FMHealthResult)),
      ),
    );
    console.log(JSON.stringify(result, null, 2));
    process.exit(result.available ? 0 : 1);
  }

  // Demo chat
  console.log("Foundation Models Client Demo");
  console.log("=============================");

  const client = createFMClient();
  const prompt = args.join(" ") || "Hello! Can you introduce yourself briefly?";

  console.log(`Prompt: ${prompt}\n`);

  try {
    const response = await Effect.runPromise(
      client.chat({
        messages: [{ role: "user", content: prompt }],
      }),
    );
    console.log("Response:", response.choices[0]?.message.content);
    console.log("\nUsage:", response.usage);
  } catch (e) {
    console.error("Error:", e);
    process.exit(1);
  }
}
