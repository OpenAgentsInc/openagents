import { AnthropicClient } from "@effect/ai-anthropic"
import { NodeContext, NodeHttpClient } from "@effect/platform-node"
import { Config, Console, Effect, Layer } from "effect"
import { ContextManagerLayer } from "./github/ContextManager.js"
import { GitHubClientLayer } from "./github/GitHub.js"
import { GitHubToolsDefault } from "./github/GitHubTools.js"
import { MemoryManagerLayer } from "./github/MemoryManager.js"
import { PlanManagerLayer } from "./github/PlanManager.js"
import { TaskExecutorDefault } from "./github/TaskExecutor.js"
// Note: Removed import of startServer from Server.js to avoid circular dependency

// Define Anthropic Layer
console.log("DEBUG: CRITICAL - Creating AnthropicClient layer with config")
const AnthropicLayer = AnthropicClient.layerConfig({
  apiKey: Config.secret("ANTHROPIC_API_KEY")
})
console.log("DEBUG: CRITICAL - AnthropicClient layer created successfully")

// Combined layers for the application
console.log("DEBUG: CRITICAL - Creating application layers composition")

// Create the HTTP client layer first
console.log("DEBUG: CRITICAL - Creating NodeHttpClient.layerUndici")
const httpClientLayer = NodeHttpClient.layerUndici
console.log("DEBUG: CRITICAL - Successfully created NodeHttpClient.layerUndici")

// Combine the Anthropic layer with HTTP client
console.log("DEBUG: CRITICAL - Providing HTTP layer to Anthropic layer")
const anthropicWithHttpLayer = Layer.provide(AnthropicLayer, httpClientLayer)
console.log("DEBUG: CRITICAL - Successfully created anthropicWithHttpLayer")

// Final merged layer
export const AllLayers = Layer.mergeAll(
  // Service layers
  GitHubClientLayer,
  PlanManagerLayer,
  ContextManagerLayer,
  MemoryManagerLayer,
  // Tools layer
  GitHubToolsDefault,
  // Task execution layer
  TaskExecutorDefault,
  // AI layer
  anthropicWithHttpLayer
).pipe(
  Layer.provide(NodeContext.layer),
  Layer.tap(() => Effect.sync(() => {
    console.log("DEBUG: CRITICAL - AllLayers composition created and initialized successfully")
  }))
)

// We don't start the server directly from Program.ts anymore
// This file is now just responsible for providing AllLayers
// Let the caller know the module was processed
console.log("DEBUG: Program.js module processed - AllLayers ready for use")
Effect.runPromise(Console.log("Program.js: AllLayers initialized successfully"))
