# Khala Sync DB Collection Receipt

Date: 2026-07-04
Issue: OpenAgentsInc/openagents#8345
Epic: OpenAgentsInc/openagents#8339

## Summary

`@openagentsinc/khala-sync-db-collection` is the TS-3 TanStack DB adapter over
the shipped Khala Sync client engine. The adapter consumes a
`KhalaSyncSession` plus its `KhalaSyncOverlay`, reads the overlay as the single
source of visible confirmed-plus-optimistic state, and publishes TanStack DB
sync transactions with `begin`, `truncate`, `write`, `commit`, and `markReady`.

It intentionally does not persist a second row store. Browser clients can supply
the SQLite-WASM-backed Khala Sync store and Bun clients can supply the
`bun:sqlite` store; the adapter itself depends only on the shared session and
overlay interfaces.

## Mutation Matching

Collection `onInsert`, `onUpdate`, and `onDelete` handlers route through named
Khala Sync client mutators. The adapter captures the assigned Khala
`mutation_id` from the session pending queue immediately after enqueue and uses
`awaitMutation` to wait for that exact id to leave the queue.

`createKhalaSyncMutationTracker` is the companion hook for
`createKhalaSyncSession({ onRejection })`. Rejections are matched by
`mutation_id` and surface as `KhalaSyncDbCollectionError` instead of resolving
as a successful write.

## First Consumer

The package includes `fleetRunKhalaSyncCollectionOptions`, binding the existing
`scope.fleet_run.<runId>` projection to a TanStack DB collection over the
`fleet_run` entity type. Its first supported optimistic intent is
`fleet.setDesiredSlots`, matching the server-side fleet mutator vocabulary.

MC-3 adds the second typed consumer helper:
`chatThreadKhalaSyncCollectionOptions` binds an owner personal scope to
`chat_thread`, maps inserts to `chat.createThread`, maps title updates to
`chat.renameThread`, and shares `chatThreadsForSidebar` for desktop and Start
web thread-list projections.

The package test runs a real Khala Sync session and overlay against a
production-shaped fake fleet transport seeded from the `FleetRunEntity`
fixture and a chat fake transport seeded from `ChatThreadEntity` rows. It proves
initial catch-up, live updates, an optimistic `desiredSlots` mutation round
trip, a two-client chat create/rename propagation path, newest-first sidebar
ordering, and typed rejection handling.

## Verification

```sh
bun run --cwd packages/khala-sync-db-collection test
bun run --cwd packages/khala-sync-db-collection typecheck
```
