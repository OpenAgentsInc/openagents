import { invoke } from "@tauri-apps/api/core";

/**
 * Get the Tinyvex WebSocket URL from the backend
 * On mobile, this will be overridden with a dynamically discovered server URL
 */
export async function getDefaultTinyvexWsUrl(): Promise<string> {
  // Try environment variable first
  const envUrl = (import.meta as any).env?.VITE_TINYVEX_WS_URL;
  if (envUrl) {
    return envUrl;
  }

  // Get from Tauri backend
  try {
    return await invoke<string>("get_websocket_url");
  } catch (error) {
    console.error("Failed to get WebSocket URL from backend:", error);
    // Fallback to localhost (should never happen but just in case)
    return "ws://127.0.0.1:9100/ws";
  }
}

/**
 * Default Tinyvex WebSocket URL for desktop platforms (synchronous fallback)
 * Use getDefaultTinyvexWsUrl() for the proper async version
 * @deprecated Use getDefaultTinyvexWsUrl() instead
 */
export const DEFAULT_TINYVEX_WS_URL = "ws://127.0.0.1:9100/ws";

