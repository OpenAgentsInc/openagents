# Pro #40 mobile controller work packet

- Status: implemented; production-broker verification pending
- Source: current owner direction to complete every open Pro issue; Pro issue #40
- Outcome: replace the one-screen desktop mirror with an authenticated, adaptive Convex controller frame
- Repository: `OpenAgentsInc/openagents`
- Base: `f6f05020ba`
- Verification: focused mobile tests, mobile typecheck, generated repository inventories, then repository `pnpm run check`

## Repository work claim

```text
CLAIM
actor/session: codex-pro-40-mobile-controller
base: f6f05020ba
worktree/branch: worktrees/openagents/pro-40-mobile-controller (detached)
scope: native OpenAuth entry, Pro Convex read client, durable command transport, typed routes/deep links, adaptive workspace frame, attention inbox, thread feed, guarded composer, approval/input response, and live interrupt
paths: apps/openagents-mobile/src/app.tsx; apps/openagents-mobile/app.json; apps/openagents-mobile/src/auth/**; apps/openagents-mobile/src/controller/**; apps/openagents-mobile/src/outbox/client-outbox-provider.tsx; apps/openagents-mobile/tests/controller-*.test.ts; apps/openagents-mobile/package.json; pnpm-lock.yaml; docs/mobile/2026-08-06-pro-40-mobile-controller-work-packet.md; generated assure-repo inventories when required
hot files: apps/openagents-mobile/src/app.tsx; apps/openagents-mobile/package.json; pnpm-lock.yaml
hot contracts: openagents.mobile_controller.v1; openagents.client_command_outbox.v1 (consumed without version change)
verification: focused mobile tests + mobile typecheck + pnpm run check
claimed_at: 2026-08-06T18:00:00Z
```

## Authority and constraints

The phone is a controller, not a second work authority. It subscribes directly
to Pro's bounded Convex projections with a short-lived read token. Command
intent enters the already-mounted durable outbox; Pro verifies the native
OpenAuth session, applies capability policy on the server, and returns the
authoritative receipt. Credentials remain in SecureStore and never enter
Convex documents, outbox payloads, logs, or projection caches.

The implementation does not expand Khala Sync, restore OTA, or add a hidden
command lane. Native terminal and diff islands remain deferred until profiling
evidence warrants them.

## Implemented frame

- OpenAuth Authorization Code + PKCE entry stores native credentials only in
  SecureStore and follows server-provided token rotation.
- The official Convex React client subscribes to the personal workspace,
  attention inbox, work shells, and bounded thread details using five-minute
  browser-surface tokens issued by Pro.
- Home and thread routes adapt from a native stack on phones to a clamped
  sidebar/thread split at `720 × 600`; all controller routes have typed deep
  links.
- Messages and approval/input decisions use the durable SQLite outbox and
  receive Pro authority receipts. Interrupt remains an explicit live-only
  command and refuses offline execution.
- Feed anchoring, retention bounds, disclosure freeze, double-submit guarding,
  route topology, command serialization, credential rotation, and responsive
  layout laws have deterministic tests.

Shipping this native UI means producing the next App Store build. OTA remains
retired by design; the production Pro broker and Convex projection deploy
independently.
