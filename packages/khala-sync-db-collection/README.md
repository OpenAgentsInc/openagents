# @openagentsinc/khala-sync-db-collection

TanStack DB collection adapter for Khala Sync.

This package turns the shipped `@openagentsinc/khala-sync-client` session and
overlay into a TanStack DB `SyncConfig`. It does not create a second local
store: confirmed rows stay in the Khala Sync local store, optimistic effects
stay in the Khala Sync overlay, and TanStack DB receives snapshots through
`begin`, `truncate`, `write`, `commit`, and `markReady`.

## API

```ts
import {
  createKhalaSyncMutationTracker,
  khalaSyncCollectionOptions,
} from "@openagentsinc/khala-sync-db-collection"
import { createCollection } from "@tanstack/db"

const mutationTracker = createKhalaSyncMutationTracker()

// Pass mutationTracker.onRejection into createKhalaSyncSession({ onRejection }).

const collection = createCollection(
  khalaSyncCollectionOptions({
    scope,
    collection: "fleet_run",
    session,
    overlay,
    mutationTracker,
    getKey: row => row.runId,
    mutators: {
      update: mutation => ({
        mutator: fleetSetDesiredSlotsClientMutator,
        args: {
          runId: mutation.key,
          desiredSlots: mutation.changes.desiredSlots,
        },
      }),
    },
  }),
)
```

`awaitMutation(session, mutationId, { tracker })` waits for the Khala Sync
pending queue to ack the mutation id returned by `session.mutate`. If the
tracker saw an in-band rejection for that same id, it rejects with
`KhalaSyncDbCollectionError` instead of silently treating the optimistic row as
confirmed.

`loadSubset` delegates to the Khala Sync session catch-up path. TanStack's
offset/cursor request is therefore answered by the session's durable scope
cursor and resumable log catch-up, not by an adapter-owned row store.

## Fleet run helper

`fleetRunKhalaSyncCollectionOptions` is the first typed consumer. It binds the
already-live `scope.fleet_run.<runId>` scope to the `fleet_run` entity type and
maps `desiredSlots` updates to the named `fleet.setDesiredSlots` mutator.

## Chat thread helper

`chatThreadKhalaSyncCollectionOptions` binds an owner personal scope to the
`chat_thread` entity type. Inserts map to `chat.createThread`; title updates
map to `chat.renameThread`. The helper keeps optimistic rows visible while the
server supplies authoritative timestamps, so it defaults `awaitServerSync` to
`false` for chat rows.

`chatThreadsForSidebar` is the shared sidebar projection for desktop and Start
web consumers. It filters by title/thread id and sorts newest-first by
`updatedAt`, with a stable `threadId` tie-breaker.

## Chat message helper

`chatMessageKhalaSyncCollectionOptions` binds a thread scope
(`scope.thread.<threadId>`) to the `chat_message` entity type. Inserts map to
`chat.appendMessage` and require a caller-supplied `messageId`, so mobile and
desktop can submit idempotent control intents without leaking message bodies
into the owner personal thread-list scope.

`chatMessagesForTranscript` is the shared transcript projection. It removes
soft-deleted messages and sorts by `createdAt`, with `messageId` as the stable
tie-breaker.

## Verification

```sh
bun run --cwd packages/khala-sync-db-collection test
bun run --cwd packages/khala-sync-db-collection typecheck
```
