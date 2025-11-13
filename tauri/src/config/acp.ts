/**
 * Default Tinyvex WebSocket URL for desktop platforms
 * On mobile, this will be overridden with a dynamically discovered server URL
 */
export const DEFAULT_TINYVEX_WS_URL = (import.meta as any).env?.VITE_TINYVEX_WS_URL || "ws://127.0.0.1:9099/ws";

