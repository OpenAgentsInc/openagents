/** Dev-only non-secret routing hints for the mobile Khala Sync chat feed. */
export const KHALA_SYNC_DEMO_BASE_URL =
  process.env.EXPO_PUBLIC_OPENAGENTS_BASE_URL?.trim() || "https://openagents.com"

export const KHALA_SYNC_DEMO_OWNER_USER_ID =
  process.env.EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID?.trim() ?? ""

export const KHALA_SYNC_DEMO_THREAD_ID =
  process.env.EXPO_PUBLIC_KHALA_SYNC_DEMO_THREAD_ID?.trim() ||
  "thread_khala_mobile_demo"

export const KHALA_SYNC_DEMO_CLIENT_GROUP_ID = "khala-mobile-chat-feed-demo"
