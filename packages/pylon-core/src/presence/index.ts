/**
 * Presence (P3) — go-online / heartbeat / counted capacity refs.
 *
 * Extracted from `apps/pylon/src` (issue #8578, PY-1). Presence sits near the
 * top of the dependency graph: it needs custody, executor, and shared to be
 * in-package first (all landed), plus three small out-of-package couplings
 * resolved via dependency injection rather than a direct import, since a
 * workspace package cannot import back from the app:
 *   - `nip90-lane-refs.ts` — the 4 lane-ref/relay/capability-ref helpers
 *     presence needs from `provider-nip90.ts`, split out because the rest of
 *     that file depends on the wallet and labor-market rails.
 *   - `apple-fm-status.ts` — a structural mirror of
 *     `PylonAppleFmStatusProjection` plus the two pure capacity-ref helpers,
 *     because the real type/prober live in `apps/pylon/src/node/apple-fm-
 *     status.ts`, which is coupled to `@openagentsinc/pylon-runtime` (a
 *     nested workspace package never resolvable by name from a sibling
 *     package). `presence.ts`'s `appleFmStatusProbe` injection seam (which
 *     already existed as a test seam) is now load-bearing: the app-level
 *     shim (`apps/pylon/src/presence.ts`) wires the real probe by default so
 *     production behavior is unchanged.
 *   - the wallet-probe shape — presence only reads 4 boolean fields off
 *     `WalletStatusProjection`, so `HeartbeatWalletProbe` is defined here as
 *     a standalone structural type rather than importing the Spark wallet
 *     module (still deferred, per the issue's Spark-preservation mandate).
 */

export * from "./nip90-lane-refs.js"
export * from "./apple-fm-status.js"
export * from "./presence-error.js"
