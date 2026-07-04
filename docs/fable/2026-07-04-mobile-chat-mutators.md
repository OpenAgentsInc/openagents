# MC-1 Owner-Private Chat Mutator Receipt

Date: 2026-07-04
Issue: #8352
Epic: #8339

## Scope

MC-1 adds server-authoritative Khala Sync chat mutators:

- `chat.createThread`
- `chat.appendMessage`
- `chat.renameThread`

The mutators write `khala_sync_chat_threads` and
`khala_sync_chat_messages` in Cloud SQL and append Khala Sync changelog
entries in the same transaction. The mutation ledger remains the replay and
idempotency boundary.

## Scope Privacy

Thread metadata is replicated into `scope.user.<owner>` for owner-private
thread-list discovery and into `scope.thread.<threadId>` for the selected
thread. Message bodies replicate only into `scope.thread.<threadId>`.

No MC-1 mutator writes `scope.public.*` or a shared chat firehose. Newly-created
thread scopes are owned by the first-writer-wins `khala_sync_scope_owners` row.
Legacy `scope.thread.*` agent-run/autopilot mappings remain supported.

## Verification

- `bun test packages/khala-sync-server/src/chat-mutators.test.ts`
- `bun test packages/khala-sync/src/index.test.ts`
- `bun run --cwd packages/khala-sync-server typecheck`
- `bun run --cwd packages/khala-sync typecheck`
- `bun run --cwd apps/openagents.com/workers/api test -- src/khala-sync-mutators.test.ts src/khala-sync-scope-auth.test.ts src/khala-sync-push-routes.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun run check:deploy`

The local-Postgres chat mutator suite verifies create/append/rename, in-band
business rejections, duplicate replay, foreign-owner refusal, no public-scope
changelog rows, no message bodies in the owner thread-list scope, and catch-up
visibility via `logPage`.
