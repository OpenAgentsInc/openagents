import type { ConnectionPhase } from "./types.js"

export const PROMPT_MESSAGE =
  "Use 3 read-only tools to explore the root folder, then summarize in 2 sentences what you see."

export const phaseLabels: Record<ConnectionPhase, string> = {
  connecting: "Connecting",
  connected: "Waiting for session",
  ready: "Session ready",
  error: "Connection error",
}
