export { MyRuntimeProvider } from "./MyRuntimeProvider";
export { createAcpAdapter } from "./adapters/acp-adapter";
export { createOllamaAdapter } from "./adapters/ollama-adapter";

// Placeholder for future factory; not used yet but reserved for consumers
export function createOpenAgentsRuntime() {
  // Intentionally empty for now. Left as an extension point to
  // create a runtime instance outside of React if needed.
  return null as any;
}

