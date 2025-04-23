import { AnthropicClient } from "@effect/ai-anthropic"
import { NodeContext, NodeHttpClient } from "@effect/platform-node"
import { Config, Console, Effect, Layer } from "effect"
import { ContextManagerLayer } from "./github/ContextManager.js"
import { GitHubClientLayer } from "./github/GitHub.js"
import { GitHubToolsDefault } from "./github/GitHubTools.js"
import { MemoryManagerLayer } from "./github/MemoryManager.js"
import { PlanManagerLayer } from "./github/PlanManager.js"
import { TaskExecutorDefault } from "./github/TaskExecutor.js"
import { startServer } from "./Server.js"

// Define Anthropic Layer
const AnthropicLayer = AnthropicClient.layerConfig({
  apiKey: Config.secret("ANTHROPIC_API_KEY")
})

// Combined layers for the application
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
  Layer.provide(AnthropicLayer, NodeHttpClient.layerUndici)
).pipe(Layer.provide(NodeContext.layer))

// Start the server when running the program directly
if (import.meta.url === `file://${process.argv[1]}`) {
  startServer()
} else {
  // If imported as a module, log a message
  Effect.runPromise(Console.log("Module imported, not starting server."))
}
