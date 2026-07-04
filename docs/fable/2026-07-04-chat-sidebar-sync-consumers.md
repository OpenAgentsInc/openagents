# Chat Sidebar Sync Consumers Receipt

Date: 2026-07-04
Issue: OpenAgentsInc/openagents#8353
Epic: OpenAgentsInc/openagents#8339

## Summary

MC-3 wires the `chat_thread` Khala Sync collection into both expected consumer
surfaces:

- Khala Code Desktop exposes `khalaSyncChatThreads`,
  `khalaSyncChatCreateThread`, and `khalaSyncChatRenameThread` over the desktop
  RPC bridge. With `KHALA_SYNC_CHAT=1` and
  `KHALA_SYNC_CHAT_OWNER_USER_ID=...`, the thread sidebar uses the connected
  chat collection as its primary list source, creates new thread rows
  optimistically, and routes renames through `chat.renameThread`.
- The TanStack Start staging app adds `/khala/chat-sync`, a client-side panel
  that imports the same `chat_thread` collection adapter and projection helper.
  It renders the shared newest-first sidebar ordering and includes a deterministic
  remote-create control for route-level verification.

The legacy desktop session catalog remains the fallback only when chat sync is
disabled, missing auth/owner scope, or unavailable.

## Behavior Contract

The new enforced UX contract is
`khala_code.chat.sync_remote_thread_appears_without_restart.v1` in
`clients/khala-code-desktop/src/contracts/ux-contracts.ts`. It states that a
thread created on another device appears in the sidebar without restart, that
ordering remains newest-first, that spinner semantics remain truthful, and that
the sidebar does not poll the legacy session catalog while a connected
`chat_thread` sync source is available.

## Verification

```sh
bun test packages/khala-sync-db-collection/src/index.test.ts
bun test tests/rpc-schema.test.ts tests/ux-contracts.test.ts tests/app-shell.test.ts
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-chat-sync.test.tsx
```
