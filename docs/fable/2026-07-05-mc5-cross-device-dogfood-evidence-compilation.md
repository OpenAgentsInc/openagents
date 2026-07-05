# MC-5 Cross-Device Chat Dogfood — Evidence Compilation

Date: 2026-07-05
Issue: OpenAgentsInc/openagents#8354
Epic: OpenAgentsInc/openagents#8339

## Status

`pending_owner_signoff`. This is a **compilation pass**, not a new owner-run
device test. Its job is to gather the public-safe counts/latencies/scope
refs/receipt refs that ALREADY exist from this session's real production
activity, so the owner's remaining device run + sign-off is fast, and to be
honest about what still genuinely has not happened. It does not flip
`#8354`'s bundle to `owner_signed` and does not close the issue — only the
owner can do that, by running the device test and recording sign-off.

Compiled bundle:
`docs/khala-sync/receipts/2026-07-05-mc5-cross-device-chat-dogfood-evidence-compilation.json`
(validated with `bun scripts/validate-khala-sync-cross-device-evidence.ts`).
It sits alongside, and does not replace, the still-open preflight bundle
`docs/khala-sync/receipts/2026-07-04-cross-device-chat-dogfood.pending.json`.

## What is genuinely proven this session (real production, not fixtures)

All of the following are real production Khala Sync activity, independently
readable from `docs/khala-code/2026-07-04-mobile-tailnet-handshake.md`:

1. **Real production chat mutators work end-to-end.** `chat.createThread` and
   `chat.appendMessage` against `openagents.com`'s production Worker + Cloud
   SQL Postgres both return `applied`, and a `POST /api/sync/bootstrap` read
   immediately after shows the real `chat_thread`/`chat_message` rows.
2. **Desktop write -> mobile live pickup, without restart.** A message
   appended through Khala Code Desktop's real `khalaSyncChatAppendMessage` RPC
   showed up in the very next production bootstrap read, and the mobile app's
   feed (bootstrap snapshot + live WebSocket tail) picked it up with no app
   changes — proving the desktop-authoring / mobile-observing direction of
   the sync loop for real.
3. **A real, unattended production dispatch loop.** The standing
   `com.openagents.runtime-supervisor` launchd process discovered a queued
   `runtime.startTurn` control intent on thread
   `019f309c-d9b1-70f2-9228-e3992ca1fa5a` (owner
   `user_ccf97bf1-ad33-4c55-b9c7-41eeeb9e0c93`) on its own 3-second poll,
   dispatched a real Codex SDK turn (`turn.rrfinal2.1783235990379882000`),
   and streamed real events (`turn.started` -> `text.delta` -> `text.completed`
   -> `usage.recorded` -> `turn.finished`) back through
   `runtime.recordEvent` — all observed via production Postgres, with zero
   manual intervention after the initial push.
4. **That same thread's transcript rendered correctly on the mobile app**
   (Expo `clients/khala-mobile`) via `com.openagents.khala.mobile://thread/...`,
   in the iOS Simulator, driven entirely by real Khala Sync data (not a
   fixture).
5. **A real production cross-agent handoff mutation** (#8407): pushing a
   `chat.appendMessage` + `runtime.startTurn` pair against the same thread
   returned `{"results":[{"mutationId":1,"status":"applied"},{"mutationId":2,"status":"applied"}]}`,
   and a follow-up bootstrap read confirmed the new `runtime_turn` row exists
   with the other lane (`claude_pylon`).
6. **Two independently-confirmed TestFlight builds** for the Expo
   `clients/khala-mobile` app (`com.openagents.khala.mobile`, App Store
   Connect app id `6787620136`): build `3bb487cf-73b6-470f-a2ee-867ee924426e`
   and build `bb16234d-0cb0-4049-90c5-be9c65ac07e2`, both
   `processingState: VALID`, confirmed directly against the App Store
   Connect API today (`docs/fable/2026-07-04-ts-8-expo-mobile-scaffold.md`).

None of this is chat content — every count/ref above is a route, scope,
thread id, turn id, or build id, never a message body.

## What is honestly NOT proven yet (read this before treating #8354 as done)

1. **No physical device has run any part of this flow.** Every mobile-side
   verification cited above and in the source docs is either a direct
   HTTP/RPC call, or the app running in the **iOS Simulator**
   (`com.openagents.khala.mobile://thread/...` deep link, Simulator
   screenshots). Grepping the whole handshake doc for "physical device" or
   "real phone" turns up only the ROUTING LOGIC that distinguishes simulator
   vs. physical-device probing (`Device.isDevice`), never an actual
   physical-device test run. The issue's own acceptance criterion — "owner
   creates a chat on the phone" — has not happened yet on real hardware.
2. **The "web" leg does not exist as a real Khala Sync consumer today.**
   Read directly: `apps/openagents.com/apps/start/src/routes/khala/chat-sync.tsx`
   is a client-only React component seeded with `useState(initialThreads)`
   hardcoded fixture data and a "Simulate remote create" button that just
   appends another hardcoded object to local state. It makes **zero** calls
   to `/api/sync/push`, `/api/sync/bootstrap`, or `/api/sync/connect`. This
   matches the honest caveat already written into the preflight doc ("record
   the current web gap if the route is still in deterministic-demo mode") —
   confirmed: it is. So "phone <-> desktop <-> web" as stated in the issue
   title currently has no real web leg to test at all, independent of the
   phone leg's readiness. A sibling receipt for a different issue
   (`docs/khala-sync/receipts/2026-07-05-runtime-ai-sdk-shaped-dogfood.simulator.json`,
   #8375) flags the same web gap independently
   (`gap.khala_sync.runtime.web_projection_runtime_stream`).
3. **The interim SwiftUI bridge (`clients/khala-ios`) has not been uploaded
   to TestFlight.** The MC-5 preflight commit (`4f60ac649d`) added
   `KhalaChatSync.swift` and wired `chat.createThread`/`chat.appendMessage`
   into that app, verified only by Swift unit tests
   (`KhalaClientTests.swift`, 48 tests) and `xcodebuild ... test` — never a
   real device install. The two TestFlight builds confirmed `VALID` today
   belong to the **separate** Expo `khala-mobile` app
   (`com.openagents.khala.mobile`), not the SwiftUI `khala-ios` app
   (`com.openagents.khala`) the issue names as the "interim SwiftUI app
   path." A TestFlight refresh is still needed for whichever app the owner
   actually runs the device test from.
4. **No explicit latency measurement exists for this session.** Every
   "picked up... with no further app changes" / "within seconds" statement in
   the source docs is qualitative, not instrumented. This pass deliberately
   did **not** manufacture a new timed test by pushing a throwaway thread into
   the owner's real production chat scope — that scope is the owner's actual
   personal Khala Code chat history, and creating test data there without
   being asked crosses from "compile existing evidence" into "generate new
   production side effects the owner didn't request." The correct place to
   capture a real, meaningful latency number is the owner's own device run,
   which will naturally produce one.

## What the owner still needs to do

This section intentionally does not attempt to close #8354. Per the issue's
own "Delegability: MED" note, the remaining steps are:

1. Decide which app to run the device test from — the interim SwiftUI
   `clients/khala-ios` bridge (code-complete, unit-tested, no TestFlight
   build yet) or the Expo `clients/khala-mobile` app (two VALID TestFlight
   builds today, real Khala Sync chat UI, Simulator-verified only) — and, if
   needed, cut a fresh TestFlight build for that choice.
2. On a real phone, create a chat and confirm it appears in Khala Code
   Desktop. Note actual elapsed time if convenient (not required).
3. Reverse direction: create/rename a thread on desktop, confirm the phone
   picks it up via its sync/refresh path.
4. Decide whether "web" is in scope for this specific acceptance run — today
   it is a fixture-only demo route with no real sync wiring, so either treat
   that leg as an explicit known gap in the sign-off, or file the (separate,
   real) follow-up to wire `/khala/chat-sync` to the real
   `khala-sync-client`/bootstrap/connect routes first.
5. Record counts/latencies/route+scope+receipt refs only (never chat text)
   into an `owner_signed` bundle (schema
   `openagents.khala_sync.cross_device_chat_dogfood.v1`), validate it with
   `bun scripts/validate-khala-sync-cross-device-evidence.ts <bundle>.json`,
   and close #8354 referencing it.

## Verification run for this compilation pass

```sh
bun scripts/validate-khala-sync-cross-device-evidence.ts docs/khala-sync/receipts/2026-07-05-mc5-cross-device-chat-dogfood-evidence-compilation.json
bun test scripts/validate-khala-sync-cross-device-evidence.test.ts
```

Both pass. No app code changed in this pass — this is a docs/evidence-only
compilation.
