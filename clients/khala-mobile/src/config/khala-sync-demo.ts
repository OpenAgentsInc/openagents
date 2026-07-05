/**
 * Dev-only wiring for the mobile Khala Sync chat UI (no login flow exists
 * yet). Base URL and owner user id are non-secret routing hints; the bearer
 * token must be exported as EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN before
 * `expo start` / `expo run:ios` — never hardcode a real token here.
 */
export const KHALA_SYNC_DEMO_BASE_URL =
  process.env.EXPO_PUBLIC_OPENAGENTS_BASE_URL?.trim() || "https://openagents.com"

export const KHALA_SYNC_DEMO_OWNER_USER_ID =
  process.env.EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID?.trim() ?? ""

export const KHALA_SYNC_DEMO_TOKEN =
  process.env.EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN?.trim() ?? ""

export const KHALA_SYNC_DEMO_CLIENT_GROUP_ID = "khala-mobile-chat-ui"
