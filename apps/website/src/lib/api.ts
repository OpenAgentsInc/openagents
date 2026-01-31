/**
 * OpenAgents API base URL for website.
 * Same-origin when deployed (openagents.com); configurable via PUBLIC_API_URL for dev or alternate deploy.
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

export const OA_API_KEY_STORAGE = "openagents_api_key";
