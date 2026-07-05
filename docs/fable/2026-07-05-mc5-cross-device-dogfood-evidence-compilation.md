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

## Update (issue #8413): the "web" gap in item 2 above is now closed

`/khala/chat-sync` (`apps/openagents.com/apps/start/src/routes/khala/chat-sync.tsx`)
is a real Khala Sync client now, not the fixture demo described above. It
does real `POST /api/khala-sync/bootstrap`, `GET /api/khala-sync/connect`
(WebSocket live-tail), and `POST /api/khala-sync/push` calls — proxied
same-origin through this app's own Worker
(`apps/openagents.com/apps/start/src/khala-sync-proxy.ts`) to production
`openagents.com`'s real `/api/sync/bootstrap` `/api/sync/connect`
`/api/sync/push` routes, with the bearer token held only in an httpOnly
cookie server-side (never in browser JS). Full design rationale, and real
production verification transcripts (real bootstrap read, real
`chat.createThread`/`chat.appendMessage` pushes, and a real live `DeltaFrame`
delivered over the local dev Worker's own WebSocket proxy), are recorded in
`docs/khala-code/2026-07-04-mobile-tailnet-handshake.md`.

This closes the specific gap named in item 2 and item 4 of "What the owner
still needs to do" above — the web leg is now a real sync consumer, so
"phone <-> desktop <-> web" has a real web leg to test. It does NOT by
itself satisfy #8354's remaining physical-device requirements (items 1 and 3
in "What is honestly NOT proven yet" are unrelated to the web leg and remain
open); it only removes "web has no real sync wiring" as a reason to treat
that leg as an explicit known gap.

## Second pass (2026-07-05, same session): code-level scope/mutator compatibility trace, plus one new honest gap

This pass verified — by reading actual code, not by trusting the docs above —
whether phone (mobile + the interim SwiftUI bridge), desktop, and web
genuinely read/write the **same** Khala Sync scope for a chat thread with
compatible mutator shapes, or only look similar. It also amended
`docs/khala-sync/receipts/2026-07-05-mc5-cross-device-chat-dogfood-evidence-compilation.json`
in place: the `blocker.web_leg_not_wired_to_real_sync`/matching gap entry is
now `resolvedByIssueRef: "OpenAgentsInc/openagents#8413"`, a new `flows[]`
entry records the #8413 verification transcript below, and one new gap
(`gap.khala_sync.cross_device.ios_bridge_no_message_transcript_read_path`) is
recorded honestly rather than silently absorbed into the existing TestFlight
gap.

### Verdict: genuinely wire-compatible, not just superficially similar

Ground truth is the server mutator/entity schema:
`packages/khala-sync-server/src/chat-mutators.ts` (scope layout comment,
lines 27-29: `scope.user.<owner>` for `chat_thread` metadata,
`scope.thread.<threadId>` for both `chat_thread` and `chat_message`;
`CreateThreadArgs{threadId,title}` / `AppendMessageArgs{threadId,messageId,body}`
at lines 55-66) and `packages/khala-sync/src/chat.ts` (`ChatThreadEntity` /
`ChatMessageEntity` field lists, lines 35-58).

- **Mobile** (`clients/khala-mobile`), **desktop**
  (`clients/khala-code-desktop`), and **web**
  (`apps/openagents.com/apps/start`) all import the identical
  `personalScope`/`threadScope` builders and `ChatThreadEntity`/
  `ChatMessageEntity` decoders from the shared `@openagentsinc/khala-sync`
  package, and desktop + web additionally share the exact same
  `chatCreateThreadClientMutator`/`chatAppendMessageClientMutator` builder
  functions from `@openagentsinc/khala-sync-db-collection` — this is one
  shared implementation, not three independent ports that happen to agree.
- **The interim SwiftUI `khala-ios` bridge** (`KhalaChatSync.swift`) is a
  genuinely separate Swift implementation with no shared TS package, but it
  was checked field-by-field against the same server schema: identical
  mutator names (`chat.createThread`/`chat.appendMessage`), identical arg
  field names (`threadId`, `title`, `messageId`, `body`), identical entity
  field names on read (`threadId`, `ownerUserId`, `title`, `messageCount`,
  `lastMessageAt`, `createdAt`, `updatedAt`), and its own client-generated ref
  formats (`ios.thread.<uuid>`, `ios.message.<uuid>`) satisfy the shared
  server-side ref-validation regex
  (`^[A-Za-z0-9][A-Za-z0-9._:-]*$`, present in both
  `packages/khala-sync/src/chat.ts` and
  `packages/khala-sync-server/src/chat-mutators.ts`). No field-name or
  type mismatch was found anywhere (no stray `syncThreadId`/`thread_id`
  variant on any surface).
- **ID generation intentionally differs per surface** (mobile/web mint
  `` `thread.<ts><hex>` `` via a shared `makeSafeRef`/push-core helper; iOS
  mints `` `ios.thread.<uuid>` ``; desktop reuses the underlying session's own
  thread id) but this does not break cross-surface visibility: every surface
  discovers a thread via `scope.user.<owner>` bootstrap (reading back
  whichever id the creating surface picked), not by independently deriving a
  matching id. Confirmed identically implemented in the mobile drawer, the
  web thread list panel, and the desktop sidebar.

**No compatibility bug was found and nothing needed fixing.** Wire
compatibility is real, not superficial, primarily because three of the four
surfaces literally share the same TypeScript packages, and the fourth (Swift)
matches the same JSON schema exactly where it implements the protocol at all.

### One bounded, honest asymmetry found (not a wire bug): the SwiftUI bridge cannot render the message transcript

`KhalaChatSync.swift` bootstraps `scope.user.<owner>` for `chat_thread` rows
(thread list, title, message count) and pushes `chat.createThread`/
`chat.appendMessage`, so **write visibility and thread-list discovery work
correctly cross-device**. But it never bootstraps `scope.thread.<threadId>`
and never opens `/api/sync/connect` — there is no WebSocket client code
anywhere under `clients/khala-ios/Khala/Khala`. So if the owner's device test
uses the SwiftUI app specifically, they will be able to create/continue a
thread from the phone and see it (and its message count) update on
desktop/web, but the phone app itself cannot display the actual message
transcript authored elsewhere. This is consistent with that app's documented
status as an "interim companion" rather than full-parity client (see this
repo's root `CLAUDE.md`), so it is recorded as a new gap rather than treated
as a defect requiring an immediate fix. A fix, if wanted, is small and
additive: mirror the existing `fetchChatSyncThreads` bootstrap pattern in the
same file for `scope.thread.<threadId>` plus a connect/live-tail path for
`chat_message`. The Expo `clients/khala-mobile` app has no such gap — it
already bootstraps and live-tails both scopes (see item 4 in "What is
genuinely proven" above).

### New real flow: the #8413 web-leg verification transcript

The #8413 landing commit's own verification (recorded in
`docs/khala-code/2026-07-04-mobile-tailnet-handshake.md`, "Verification (real
production Khala Sync, this session)") is added to the JSON bundle as
`flow.khala_sync.chat.web_real_sync_client_signin_bootstrap_push_live_tail.v1`.
Summary: signed in through the local dev Worker (real workerd/Miniflare via
`@cloudflare/vite-plugin`, not a Node mock) using this session's own
already-registered agent credentials — not the owner's personal chat scope —
then real `POST /api/khala-sync/session` -> `bootstrap` -> `push`
(`chat.createThread` + `chat.appendMessage`, both `status: applied`) ->
`GET /api/khala-sync/connect` (real `101` upgrade), observing a live
`DeltaFrame` for the new thread within an approximate ~500ms of the push, then
a second `chat.appendMessage` over the already-open socket producing another
real-time `DeltaFrame`. This is the one qualitative-but-measured latency data
point that exists this session; it is a different account/scope than the
owner's real device run and does not substitute for one (see the refined
latency gap below).

### Updated "What is honestly NOT proven yet" (supersedes items 2 and 4 above)

1. No physical device has run any part of this flow — **unchanged, still
   open**.
2. ~~Web leg does not exist as a real sync consumer~~ — **closed by #8413**,
   see the update section above and the new flow in this section.
3. The interim SwiftUI bridge has no TestFlight build — **unchanged, still
   open** — plus the new, separately-tracked read-path gap: even once built,
   it cannot render the message transcript on-device (see above).
4. No latency measurement existed for this session — **partially updated**:
   one real qualitative measurement now exists for the web leg (~500ms,
   different account/scope, not the owner's device), but **no physical-device
   latency measurement exists**, which is the number that actually matters for
   sign-off.

### Updated "What the owner still needs to do"

Item 4 in the original checklist above ("decide whether 'web' is in scope...
today it is a fixture-only demo route") is now moot — the web leg is real, so
the owner can include it in the device test. If the owner picks the SwiftUI
app specifically, they should know before starting that it will not show the
transcript in-app for messages authored on desktop/web (thread list and
message counts will still update). The rest of the original checklist
(decide which mobile app, run the real phone <-> desktop <-> web round trip,
record counts/latencies/refs into an `owner_signed` bundle, validate, and
close #8354) is unchanged and still the pending owner action.

### Verification run for this second pass

```sh
bun scripts/validate-khala-sync-cross-device-evidence.ts docs/khala-sync/receipts/2026-07-05-mc5-cross-device-chat-dogfood-evidence-compilation.json
bun test scripts/validate-khala-sync-cross-device-evidence.test.ts
```

Both pass. This pass changed only the JSON evidence bundle and this doc; no
application code was touched (the code-level scope/mutator trace above found
no bug needing a fix).
