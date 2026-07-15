/**
 * LiveHub window constants (KS-4): shared between the Khala Sync substrate
 * and the Google Cloud Run LiveHub service.
 *
 * This module is intentionally dependency-free and exposed as the
 * `@openagentsinc/khala-sync-server/hub` subpath so the Node services can
 * consume the window bounds without pulling the Postgres outbox writer into
 * their module graphs.
 */

export const KHALA_SYNC_HUB_BINDING = "KHALA_SYNC_HUB"

/** Bounds for LiveHub's bounded in-memory replay window. */
export const HUB_WINDOW_MAX_ENTRIES = 10_000
export const HUB_WINDOW_MAX_BYTES = 64 * 1024 * 1024
