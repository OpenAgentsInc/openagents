import { Console, Effect, Layer } from "effect"
import { ContextManagerLayer } from "./github/ContextManager.js"
import { GitHubClientLayer } from "./github/GitHub.js"
import { GitHubToolsDefault } from "./github/GitHubTools.js"
import { MemoryManagerLayer } from "./github/MemoryManager.js"
import { PlanManagerLayer } from "./github/PlanManager.js"
import { TaskExecutorDefault } from "./github/TaskExecutor.js"
import { startServer } from "./Server.js"

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
  TaskExecutorDefault
)

// Start the server when running the program directly
if (import.meta.url === `file://${process.argv[1]}`) {
  startServer()
} else {
  // If imported as a module, log a message
  Effect.runPromise(Console.log("Module imported, not starting server."))
}
