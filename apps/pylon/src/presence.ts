/**
 * Re-export shim — moved to `@openagentsinc/pylon-core/presence` (issue #8578, PY-1).
 *
 * `sendHeartbeat` is wrapped rather than a pure re-export: pylon-core's
 * presence module cannot depend on `@openagentsinc/pylon-runtime` (a nested
 * workspace package never resolvable by name from a sibling package — see
 * `packages/pylon-core/src/presence/apple-fm-status.ts`'s header comment for
 * the full diagnosis), so it cannot run the live Apple FM bridge readiness
 * probe itself. When a caller doesn't supply `appleFmStatusProbe` explicitly
 * (every current production call site), this wrapper injects the real one
 * (`collectPylonAppleFmStatus`), so production behavior is unchanged from
 * before the move. Callers that DO supply their own probe (tests) pass
 * straight through untouched.
 *
 * Every other export (registerPylon, completePylonLink, refreshPylonLink,
 * recordAccountLinkInPresence, degradeStalePresence, withPresenceRetry, the
 * capacity-ref helpers, ...) is a faithful pass-through.
 */
import {
  sendHeartbeat as sendHeartbeatCore,
  type PresenceClientOptions,
} from "@openagentsinc/pylon-core/presence/presence"
import type { BootstrapSummary } from "./bootstrap.js"
import { collectPylonAppleFmStatus } from "./node/apple-fm-status.js"

export * from "@openagentsinc/pylon-core/presence/presence"

export async function sendHeartbeat(summary: BootstrapSummary, options: PresenceClientOptions) {
  return sendHeartbeatCore(summary, {
    ...options,
    appleFmStatusProbe:
      options.appleFmStatusProbe ??
      (() =>
        collectPylonAppleFmStatus({
          env: options.env ?? process.env,
          now: options.now?.(),
          summary,
        })),
  })
}
