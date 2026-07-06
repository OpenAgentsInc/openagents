/**
 * MM-F1 mobile wiring: pure display-label helpers for model ids and the
 * typed fallback reasons `resolveModelPreference` reports server-side.
 */
import type { KhalaModelPreferenceFallback } from "./khala-mobile-model-preference-api"

export const modelDisplayLabel = (modelId: string): string => {
  if (modelId === "gemini") return "Gemini"
  return modelId
    .split(/[-_]/)
    .filter(part => part.length > 0)
    .map(part => (part.length <= 3 ? part.toUpperCase() : part.charAt(0).toUpperCase() + part.slice(1)))
    .join(" ")
}

export const modelPreferenceFallbackMessage = (fallback: KhalaModelPreferenceFallback): string | null => {
  switch (fallback) {
    case "none":
      return null
    case "no_preference_set":
      return null // expected, quiet default — not an error state
    case "preference_unavailable":
      return "Your chosen model isn't available right now, so Khala Code is using the default instead."
    case "default_unavailable":
      return "No model is currently available. Try again shortly."
  }
}
