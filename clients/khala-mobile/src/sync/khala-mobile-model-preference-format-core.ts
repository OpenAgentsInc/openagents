/**
 * MM-F1 mobile wiring: pure display-label helpers for model ids and the
 * typed fallback reasons `resolveModelPreference` reports server-side.
 */
import type { KhalaModelPreferenceFallback } from "./khala-mobile-model-preference-api"

export const executionTargetDisplayLabel = (targetId: string): string => {
  if (targetId === "auto") return "Auto"
  if (targetId === "gemini") return "Gemini"
  if (targetId === "khala" || targetId === "openagents/khala") return "Khala"
  if (targetId.startsWith("codex:")) return "Your Codex"
  if (targetId.startsWith("claude:")) return "Claude"
  return targetId
    .split(/[-_]/)
    .filter(part => part.length > 0)
    .map(part => (part.length <= 3 ? part.toUpperCase() : part.charAt(0).toUpperCase() + part.slice(1)))
    .join(" ")
}

export const modelDisplayLabel = (modelId: string): string => {
  return executionTargetDisplayLabel(modelId)
}

export const modelPreferenceFallbackMessage = (fallback: KhalaModelPreferenceFallback): string | null => {
  switch (fallback) {
    case "none":
      return null
    case "no_preference_set":
      return null // expected, quiet default — not an error state
    case "preference_unavailable":
      return "Your chosen target isn't available right now, so Khala Code is using the default instead."
    case "default_unavailable":
      return "No execution target is currently available. Try again shortly."
  }
}
