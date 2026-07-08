/**
 * Offline sync runtime for App Store reviewer demo mode.
 *
 * In demo mode the app must never open a real Khala Sync runtime (no Expo
 * SQLite identity, no durable session, no network). This stub satisfies the
 * `KhalaMobileSyncRuntime` shape the provider/screens expect while doing
 * nothing over the wire: the scope-entities hook short-circuits to
 * `demoSyncScopeEntities` before it ever touches `overlay`/`session`/`store`,
 * so those primitives are inert placeholders here, and the chat mutations
 * resolve as harmless no-op successes so a reviewer tapping around never hits
 * an error.
 */

import type { KhalaMobileSyncRuntime } from "../sync/khala-mobile-sync-runtime"
import { DEMO_REVIEWER_OWNER_USER_ID, demoChatThreads } from "./demo-fixtures"

// The scope-entities hook never reads these in demo mode (it returns fixtures
// before the effect that would use them), so an inert placeholder is safe.
const inertPrimitive = {} as never

export const createDemoKhalaMobileSyncRuntime = (): KhalaMobileSyncRuntime => ({
  appendMessage: async ({ threadId }) => ({ messageId: "demo-msg-noop", ok: true, threadId }),
  bindThreadRepo: async ({ threadId }) => ({ ok: true, threadId }),
  chatMessages: async ({ threadId }) => ({
    authState: "connected",
    cursor: null,
    messages: [],
    ok: true,
    ownerUserId: DEMO_REVIEWER_OWNER_USER_ID,
    pendingMutations: 0,
    phase: "live",
    reason: null,
    rejections: [],
    threadId,
  }),
  chatThreads: async () => ({
    authState: "connected",
    cursor: null,
    ok: true,
    ownerUserId: DEMO_REVIEWER_OWNER_USER_ID,
    pendingMutations: 0,
    phase: "live",
    reason: null,
    rejections: [],
    threads: demoChatThreads,
  }),
  close: async () => undefined,
  createThread: async ({ threadId }) => ({ ok: true, threadId }),
  overlay: inertPrimitive,
  session: inertPrimitive,
  store: inertPrimitive,
})
