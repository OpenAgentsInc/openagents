# Khala Sync Cross-Device Dogfood Preflight

Date: 2026-07-04
Issue: OpenAgentsInc/openagents#8354
Epic: OpenAgentsInc/openagents#8339

## Status

Owner-run pending. This receipt preflights the code and evidence format needed
for MC-5, but it does not claim the physical phone -> desktop -> web dogfood
has happened yet.

## Implementation

- The interim SwiftUI app now writes owner-private chat turns through the
  existing `POST /api/sync/push` route with the server-registered
  `chat.createThread` and `chat.appendMessage` mutators.
- Local conversations persist their server `syncThreadId`, so a thread imported
  from desktop/web can be continued from the phone without creating a second
  server thread.
- The phone shell has a manual `Refresh Khala Sync` command. It resolves the
  owner user ref from the existing fleet-status payload, bootstraps
  `scope.user.<owner>`, and merges `chat_thread` metadata into the local
  drawer. Message bodies remain in `scope.thread.<threadId>` and are not copied
  into public evidence.
- `scripts/validate-khala-sync-cross-device-evidence.ts` validates the public
  dogfood bundle shape. It accepts counts, latency values, public route/scope
  refs, receipt refs, and owner sign-off metadata; it rejects raw body/content,
  prompts, transcripts, private paths, and secret-shaped strings.

## Owner Run

1. Install/refresh the TestFlight build that contains this change.
2. On phone, send one Khala chat turn. Confirm the phone shows Khala Sync
   updated.
3. On Khala Code Desktop, run with `KHALA_SYNC_CHAT=1` and the owner
   `KHALA_SYNC_CHAT_OWNER_USER_ID`; confirm the new thread appears in the
   sidebar without restart.
4. On the Start staging web chat-sync panel, confirm the same synced thread
   metadata is visible or record the current web gap if the route is still in
   deterministic-demo mode.
5. Create or rename a thread from desktop/web, then use the phone
   `Refresh Khala Sync` command and confirm the phone drawer imports the server
   thread.
6. Fill an owner-signed bundle using only counts, latencies, route refs, scope
   refs, receipt refs, build refs, and the sign-off comment ref. Never include
   chat text.
7. Validate it:

```sh
bun scripts/validate-khala-sync-cross-device-evidence.ts docs/khala-sync/receipts/<owner-signed-bundle>.json
```

## Pending Bundle

The committed preflight bundle is:

```text
docs/khala-sync/receipts/2026-07-04-cross-device-chat-dogfood.pending.json
```

It is deliberately `pending_owner_signoff`; #8354 should close only after an
owner-signed replacement or follow-up bundle records the real device run.

## Verification

```sh
bun test scripts/validate-khala-sync-cross-device-evidence.test.ts
xcodebuild -project clients/khala-ios/Khala/Khala.xcodeproj -scheme Khala -destination 'platform=iOS Simulator,name=iPhone 16' test
```
