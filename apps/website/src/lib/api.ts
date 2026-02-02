/**
 * OpenAgents API base URL. Same-origin when deployed; configurable via PUBLIC_API_URL.
 */
export function getApiBase(): string {
  if (typeof import.meta !== "undefined" && import.meta.env?.PUBLIC_API_URL) {
    return String(import.meta.env.PUBLIC_API_URL).replace(/\/$/, "");
  }
  if (typeof window !== "undefined") {
    return `${window.location.origin}/api`;
  }
  return "https://openagents.com/api";
}
