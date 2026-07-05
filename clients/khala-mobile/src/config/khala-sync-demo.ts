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

/**
 * Fleet runs are scoped per active fleet session (`scope.fleet_run.<id>`),
 * not a stable per-owner id — there is no "list all my fleet runs" scope
 * today. This points the Settings > Fleet section at one specific run id
 * (e.g. the desktop's currently active run) until a stable discovery path
 * exists.
 */
export const KHALA_SYNC_DEMO_FLEET_RUN_ID =
  process.env.EXPO_PUBLIC_KHALA_SYNC_DEMO_FLEET_RUN_ID?.trim() ?? ""

export const KHALA_SYNC_DEMO_CLIENT_GROUP_ID = "khala-mobile-chat-ui"
