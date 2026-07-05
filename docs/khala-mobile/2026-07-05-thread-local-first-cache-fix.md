# Khala Mobile Thread Cache Fix - 2026-07-05

Status: shipped. Fixes the owner report: "every time i open a new thread
session in the app it loads the messages from scratch. it needs to cache
them locally so it doesnt pull all historical messages every time, needs to
have a bunch ready to load."

## Investigation summary

Three candidate surfaces were checked for this behavior before touching any
code:

- **Khala Code desktop** (`clients/khala-code-desktop`) already reads local
  confirmed state first (in-memory `threadMessageCache` plus the durable
  `khala-sync-client` overlay/session cursor) before any network round trip.
  Not affected.
- **Web `/khala/chat-sync`** (`apps/openagents.com/apps/start`) has the same
  bootstrap-every-time anti-pattern this doc fixes, but that route is an
  explicitly-labeled Start staging/demo harness, not the production chat
  page. Left as a known follow-up, not fixed here.
- **Khala Mobile** (`clients/khala-mobile`) was the real, in-production bug:
  a full local-first sync runtime (`src/sync/khala-mobile-sync-runtime.ts` +
  `src/sync/expo-db-sqlite-persistence.ts`, real Expo SQLite persistence,
  built on `@openagentsinc/khala-sync-db-collection` /
  `@openagentsinc/khala-sync-client`) had already been built and unit-tested
  (`tests/khala-mobile-sync-runtime.test.ts`), but was never wired into the
  app — the thread message screen and thread list screen still used
  `src/sync/use-khala-sync-collection.ts`, a hook that unconditionally
  `fetch()`es `POST /api/sync/bootstrap` (the full entity history for that
  scope) every time the `scope` prop changed, i.e. every thread switch, with
  no local-cache check first. (Note: this fix landed just after a concurrent
  Expo Router → React Navigation migration in this package — see
  `docs/khala-mobile/2026-07-05-ignite-structure-audit.md` — so the screen
  paths below are the post-migration `src/screens/*` locations, not the
  earlier `app/` route files.)

## Root cause

`use-khala-sync-collection.ts`'s effect resets `items: []` and re-runs the
full bootstrap fetch on every `[scope, entityType, baseUrl, token]` change —
category (a) from the audit: a local-first cache/runtime existed, it was
simply never consulted before the network fetch on the actual thread
screens.

## Fix

1. **`src/sync/khala-mobile-sync-runtime-context.tsx`** (new): opens
   `openKhalaMobileSyncRuntime` ONCE per signed-in app session (mounted in
   `src/app.tsx`'s `AuthGate`, alongside `KhalaAuthProvider`), holding the
   durable Expo SQLite store, optimistic overlay, and Khala Sync session for
   the whole navigator tree instead of per-screen. Exposes
   `useKhalaMobileSyncPrimitives()` for screens that just need
   `{ session, overlay, store, status, error }`.
2. **`src/sync/use-khala-sync-scope-entities.ts`** (new): a generic
   local-first, delta-synced read hook for one `(scope, entityType)` pair.
   On mount it reads confirmed rows straight from the durable SQLite store
   (`store.readEntities`) — near-instant, no network wait — and renders them
   immediately. It then calls `session.subscribe(scope)`, which resumes
   `packages/khala-sync-client/src/session.ts`'s `driveScope` state machine
   from the scope's DURABLE CURSOR: a scope with no prior cursor bootstraps
   once (unavoidable — nothing cached yet), but a scope that was EVER synced
   before skips bootstrap entirely and only catches up on log entries
   committed since that cursor, then live-tails. `overlay.subscribe` +
   `session.subscribeState` notifications re-read the store on every
   confirmed change, so the view stays live.
3. **`src/screens/thread-messages-screen.tsx`**: replaced all three
   `useKhalaSyncCollection` calls (`chat_message`, `runtime_event`,
   `runtime_turn`) with `useKhalaSyncScopeEntities` backed by the shared
   runtime. Revisiting a thread now shows its cached transcript/messages
   immediately; only the entries new since the last visit come over the
   wire.
4. **`src/screens/thread-list-screen.tsx`**: same fix for the thread list
   (`chat_thread` in the owner's personal scope) — it no longer
   re-bootstraps the whole thread list on every cold app launch either.
5. **`src/screens/settings-screen.tsx`** (fleet demo panel, fixed demo scope)
   was left on the old `use-khala-sync-collection.ts` hook — out of scope
   (not a "thread session"), and that hook is kept for it.

## Correctness

Delta sync reuses the exact durable-cursor/at-least-once-apply contract
already covered by `packages/khala-sync-client`'s own test suite (idempotent
apply by `(scope, version, entityType, entityId)`; `must_refetch`/`denied`
surfaced as `status: "error"`, never silently stale). Message composition
still goes through the existing `useKhalaSyncPush` raw-push path unchanged
(pre-existing, not part of this fix); its server-confirmed result reaches
the screen via the SAME live-tail delta socket the new hook already
maintains for that scope.

## Test evidence

`clients/khala-mobile/tests/use-khala-sync-scope-entities.test.ts` (new,
`bun test`, real Expo SQLite via `bun:sqlite` + a fake Khala Sync
transport/server, same fixture shape as
`tests/khala-mobile-sync-runtime.test.ts`):

- opens a runtime, creates a thread, appends one message (one real
  bootstrap for that thread scope);
- closes it and reopens a SECOND runtime instance against the SAME
  on-device SQLite database (simulating "closed and reopened the app");
- mounts `useKhalaSyncScopeEntities` for that thread scope and asserts the
  cached message renders after a single microtask flush, with
  `bootstrapCallsByScope` for that scope still at exactly 1 (no
  re-bootstrap on reopen) while `logPageCallsByScope` shows the catch-up
  call;
- commits a new message on the fake server after the reopen and asserts it
  arrives via the live delta path, with both the old cached message and the
  new one present (no lost/clobbered data).

`bun run --cwd clients/khala-mobile test` (170 pass, 0 fail) and
`bun run --cwd clients/khala-mobile typecheck` are green.

## Follow-up not done here

- The web `/khala/chat-sync` Start staging route has the identical
  bootstrap-every-time pattern (`-use-khala-sync-collection.ts`,
  `apps/openagents.com/apps/start`). It is a demo/staging harness, not the
  production chat surface, so it was left alone; a future pass should either
  delete it or wire it to the same local-first pattern if it becomes a real
  surface.
