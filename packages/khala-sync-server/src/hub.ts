/**
 * Hub DO constants (KS-4): shared between the khala-sync-server substrate
 * and the `openagents.com` Worker that hosts `KhalaSyncHubDO`.
 *
 * This module is intentionally dependency-free (no Bun/Postgres imports) and
 * exposed as the `@openagentsinc/khala-sync-server/hub` subpath so the
 * Cloudflare Worker — typechecked against workers-types, without Bun
 * ambients — can consume the window bounds without pulling the Bun-typed
 * outbox writer into its module graph.
 */

export const KHALA_SYNC_HUB_BINDING = "KHALA_SYNC_HUB"

/** Bounds for the hub's DO SQLite log window. */
export const HUB_WINDOW_MAX_ENTRIES = 10_000
export const HUB_WINDOW_MAX_BYTES = 64 * 1024 * 1024
