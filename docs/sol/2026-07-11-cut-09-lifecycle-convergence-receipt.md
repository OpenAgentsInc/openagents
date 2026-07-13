# CUT-09 lifecycle convergence receipt

- Date: 2026-07-11
- Issue: [#8689](https://github.com/OpenAgentsInc/openagents/issues/8689)
- Parent: [#8677](https://github.com/OpenAgentsInc/openagents/issues/8677)
- State: deterministic matrix landed; physical-device live rung pending
- Scope: restart, stale generation, revocation, interrupted finalization

## Result

The deterministic lifecycle matrix now converges through the canonical Khala
Sync client/server path and both native SQLite adapters:

1. A delayed bootstrap or log response is bound to the scope generation that
   requested it. Unsubscribe, close, or revocation invalidates the response
   before it can replace/advance durable state.
2. A push response arriving after revocation cannot publish a rejection or
   acknowledgement into the closed authority generation.
3. `runtime.recordEvent` accepts only the exact durable next `event_count` and
   a lifecycle-valid transition. Only `turn.started` leaves `queued`; a second
   start cannot mutate `running`; no event can mutate a completed, failed,
   interrupted, or closed turn.
4. Each hosted-runtime tick first finds stale `running` hosted turns and writes
   one `turn.interrupted` at the exact next sequence. It never requeues or
   re-invokes the provider. The unique sequence plus turn lock serializes that
   terminal against a late original worker.
5. If the recovery terminal wins, the original worker stops after its first
   rejected write. It cannot append later text, usage, or a false success.
6. Desktop SQLite and Expo SQLite close/reopen the same partially streamed
   timeline, accept one exact interrupted terminal idempotently, and expose
   matching refs, sequence, cursor, and canceled state. Proven unlink clears
   the hosted personal/thread projections and pending queue in both adapters.

This preserves the Fable streamlining boundary: no file under
`apps/pylon/src/orchestration` or `apps/pylon/src/node` changed. The fixes live
at the shared Sync authority, hosted server consumer, and native adapters.

## Counterexamples converted to regressions

- Generation 1 captures a v1 snapshot, is unsubscribed, and resolves only
  after generation 2 is live at v2. The durable cursor stays v2 and its v2 row
  remains visible.
- A running turn is interrupted, then receives matching-next-sequence stale
  text and a later sequence gap. Both reject before projection; the turn stays
  interrupted with one recorded event.
- A hosted worker's provider completion loses the next-sequence race to
  recovery. The first rejected `text.delta` stops all subsequent writes.
- Desktop/mobile restart with partial output reconstructs the same partial
  history, then the same single interrupted terminal; exact replay adds no
  duplicate output.
- Desktop/mobile unlink after cross-device restart leaves both hosted scopes
  and durable mutation queues empty while the immutable device-local identity
  remains outside the revoked hosted authority.

## Deterministic verification

Focused suites pass with injected scheduling/clock seams and real local
Postgres/SQLite where the authority boundary requires them:

- shared Sync session generation/revocation suite;
- canonical runtime mutator suite against local Postgres;
- hosted-runtime dispatch unit and reply-guard suites;
- real Desktop SQLite + Expo SQLite continuation/restart/revocation suites;
- Desktop/mobile conversation adapter suites;
- package typechecks for Sync client/server, Worker, Desktop, and mobile.

The combined focused run reported 55 Bun cases / 298 expectations plus 26
Worker Vitest cases: 81 focused cases, zero failures.

The built Electron smoke passes Runtime Gateway protocol v7 bootstrap,
operation correlation (`ipc.received` → `gateway.received` → `sync.intent` →
`ipc.returned`), trace navigation/reload restoration, and zero-active-slot
teardown.

The full `bun run check:deploy` gate also passes. Its shared Khala Sync client
corpus reported 163 pass / 3 explicitly gated live-smoke skips / 12,691
expectations with import coverage green. Expected `FAILED` strings emitted by
negative drift-guard fixtures are self-test diagnostics; the enclosing gate
exited successfully.

## Live receipt status

The required built-Desktop plus physical-mobile network-gap/restart receipt is
not yet claimed. On 2026-07-11 the paired physical iPhone was offline in both
Tailnet discovery and Xcode device discovery. The built Desktop rung is green,
but simulator/fixture evidence is intentionally not substituted for the phone.

### Native mobile preparation rung

The hardware-independent part of the exact current mobile build is complete:

- `expo prebuild --platform ios` generated the local owned Xcode project and
  installed CocoaPods without changing tracked app configuration;
- Xcode 26.6 built `com.openagents.app` for an iPhone 17 Pro Max simulator on
  iOS 26.5;
- that built development app installed, launched to the real Effect Native
  Khala composer, terminated, and relaunched to the same surface after a fresh
  Metro bundle;
- the relaunch emitted no fatal or exception entries in the app process log;
- the same project built successfully for the generic arm64 `iphoneos` target
  with signing disabled, proving that device compilation itself is not the
  blocker.

Generated `ios/`, DerivedData, Pods, and screenshots remained ignored/local
artifacts. Upstream Expo/React Native deprecation/nullability warnings did not
fail either build. What remains is specifically a signed install on the paired
phone plus the authenticated cross-device fault journey—not native project
generation, simulator boot, or device-architecture compilation.

When the phone is available, run the public-safe procedure in
[`native-streamed-conversation-handoff.md`](./issues/native-streamed-conversation-handoff.md):
reload the renderer during one active run, restart the host after settlement,
continue or interrupt the exact confirmed run on physical mobile, then queue a
harmless offline command and prove unlink/revocation removes it without replay.
Record only build/commit, platform versions, isolated account label, hashed
refs, event-kind counts, terminal/reconciliation verdicts, and restart/revoke
verdicts—never prompt/output text, owner/device identifiers, paths, provider
metadata, raw rows/events, or credentials.

### Physical-device signed-install rung — 2026-07-12

The paired physical iPhone returned to both Tailnet and Xcode/CoreDevice
discovery on 2026-07-12 (paired, developer mode enabled, device tunnel
connected), and the owner released it for this work. From a clean detached
worktree at `origin/main` commit `83efc87477`:

- `expo prebuild --platform ios` and CocoaPods regenerated the owned native
  project cleanly with no tracked-configuration changes;
- Xcode built the Debug `com.openagents.app` for the physical arm64 device
  class with the owned Apple team's automatic development signing; the managed
  provisioning profile covers the paired device;
- the signed app **installed successfully onto the paired physical iPhone**
  via CoreDevice — the first physical-hardware install of the greenfield
  mobile app (the first attempt raced a transient device reconnect; the
  immediate retry succeeded);
- the development JS bundle compiled and is served on the local network, so
  first launch does not wait on a cold bundler;
- launch was then refused by the OS with the lock-screen error only ("device
  was not, or could not be, unlocked"). No signing, pairing, developer-mode,
  provisioning, or tunnel error remains.

The Desktop counterpart is staged in the same clean worktree: the Desktop
bundle builds green, an encrypted native session vault is already present in
the dev profile, the standing runtime-intent supervisor process is alive, and
multiple named isolated Codex accounts report ready credentials. The hosted
sync/API surface answered healthy at preparation time.

The remaining gate is therefore exactly the physical-touch set: unlock the
phone, launch and sign in on-device, and run the authenticated 4-step
cross-device fault journey above. Nothing hardware- or build-shaped remains.

### Live Desktop rungs and a real counterexample — 2026-07-12 (owner unlocked the phone)

After the owner unlocked the phone, the installed signed build launched on the
physical iPhone via CoreDevice, fetched its development bundle over the local
network, and reached the running app process. On-device sign-in remains the
owner's step.

The Desktop half of the journey then ran LIVE against the deployed hosted
sync/API surface from the built Electron app (driven through the same
registered Runtime Gateway v7 command/query contract the renderer UI issues,
via devtools automation in the built renderer):

- One real hosted conversation thread (`thread.desktop.ump71pzbbd`) was
  created, a message appended, and a run started on the `codex_app_server`
  lane. The standing runtime-intent consumer dispatched it against a named
  isolated Codex account and the canonical stream settled `completed` with
  exactly nine `openagents.khala_runtime_event.v1` events in dense sequence
  1–9: `turn.started`, `text.delta`, `text.completed`, `tool.call`,
  `tool.result`, `text.delta`, `text.completed`, `usage.recorded`,
  `turn.finished(stop)` — text plus tool lifecycle plus usage plus one
  terminal, exactly the required shape.
- Host restart: after settlement the Electron host was fully restarted; the
  same thread/run refs reconstructed the identical completed terminal
  projection (nine events, same typed item ladder) from durable Sync state.
- Renderer reload mid-stream: a later run (`turn.desktop.80ltpz1ez6`) was
  reloaded while `running`; after reload the same runRef continued and settled
  `completed` with the same dense unique 1–9 ladder — no duplicate sequence,
  no second thread, no invented completion.

**Live counterexample found and fixed.** The second and every subsequent
Desktop-origin run was never dispatched: the consumer's durable sequence-one
claim id (`stableId`) truncated its seed to the first 12 characters, so every
`turn.desktop.<random>` turn produced the SAME `event.runtime_claim.*` id and
the server correctly rejected each later claim as already recorded — misfiled
as `skipped_stale` with the underlying error swallowed. Fixed in the same
change: `stableId` now derives a truncated SHA-256 (deterministic for exact
retry, distinct across turns), the swallowed claim-push error is surfaced in
the bounded outcome detail, and a regression test drives two turns sharing a
long ref prefix through dispatch and asserts distinct claim ids and both
`applied`. The full orchestration suite (180 tests) passes.

Two additional live observations recorded for follow-up, not fixed here:
the standing launchd runtime supervisor was polling with a stale owner-user
id from the pre-identity-migration universe (its poll feed no longer matched
the identity current sessions resolve to, so hosted Desktop/mobile turns were
invisible to it until a correctly-scoped consumer was run), and the Desktop
renderer's boot-time chat-host selection can race the main-process Sync
bootstrap and silently fall back to the local harness for the whole renderer
session (relevant to #8690's synchronized live-event rework).

Still pending (unchanged): on-device sign-in and the physical mobile
continuation/interrupt, the real network-gap offline queue, and
unlink/revocation without replay — the owner-touch half of the journey.

### Physical authenticated pre-event interrupt counterexample — 2026-07-12

The owner released the phone after recording. Build 117 restored its verified
native session and the exact Desktop-created thread on first launch. Mobile
submitted one continuation and exposed Cancel while its new Codex turn was
still queued. Cancel was accepted. Production truth then converged correctly:
the exact runtime turn became `interrupted` with a durable settlement time,
the agent-run projection became `canceled`, the canonical live graph ended,
and thread-scope changelog version 7 carried all of those post-images. Because
the runtime had not produced provider event one, the turn's event count
correctly remained zero.

The client nevertheless displayed `Runtime outcome is still pending
reconciliation.` Its new-turn send waiter required a runtime-event sequence
advance even when the exact new run had already reached the authoritative
canceled post-image. That requirement is valid for appending to a pre-existing
run, but impossible for a turn canceled before its first provider event.

The repair accepts only this exact bounded case: the newly-created run ref,
terminal `canceled` status, initial sequence zero, and zero current events.
Completed/failed runs still require a real event, and append-to-existing-run
waiters still require sequence advancement. The focused oracle covers all four
branches. Physical rerun remains required after the repaired bundle is served.

Physical rerun against the repaired `main` bundle passed. The same signed build
was terminated and relaunched through CoreDevice, restored the exact thread and
terminal controls, then created a second fresh mobile turn. Cancel again won
before provider event one. Production stored the exact turn as `interrupted`,
event count zero, with a durable settlement time; mobile returned directly to
Resume/Retry/Close with no reconciliation error. This proves process
replacement plus the pre-event-cancel correction on physical iOS. The network-
gap/offline queue and unlink/revocation row remain outstanding.

The repaired bundle is now served. Commit `ea2cc667af` passes the focused
mobile conversation corpus (13 tests / 44 assertions) and mobile typecheck.
The exact build-117 runtime fingerprint
`44f4fbd0b8ab6bdd1aa410467e6df96f572762b2` was exported to the owned
`openagents-production` channel and deployed as Cloud Run revision
`oa-updates-00095-lnr`, serving 100 percent of `oa-updates` traffic. A live
manifest request to `updates.openagents.com/openagents-mobile/manifest` with
the exact iOS/runtime/channel headers returned HTTP 200 and Expo protocol v1.
Subsequent physical foregrounds therefore retain the proven correction without
requiring Metro, a rebuild, or reinstall.

### Android offline-admission counterexample and repair — 2026-07-12

The owner-authenticated API 35 Android emulator supplied an autonomous dry run
of the remaining network-gap row before repeating it on physical iOS. With
Wi-Fi and cellular disabled, the shared Sync engine durably admitted user
messages. After connectivity returned, those messages confirmed, but no
corresponding runtime intent existed and no provider turn could start.

The defect was in the mobile adapter rather than Sync's FIFO. `sendMessage`
waited for the optimistic message to become confirmed before constructing the
paired runtime intent. Its bounded waiter necessarily elapsed while offline,
so it returned `Message is still pending reconciliation` and permanently
abandoned runtime dispatch even though the queued message later drained.

Mobile now captures the last confirmed run before admission, enqueues the
message and its exact runtime intent consecutively, and relies on Sync's
durable FIFO to preserve append-before-start. If message confirmation is not
yet available, the UI receives the honest bounded result `Message and runtime
command are queued pending reconciliation.` Active-run continuation remains
bound to the last confirmed run/lane, while a new turn retains the selected
exact execution target. A regression oracle withholds message confirmation,
then proves the paired `turn.start` intent still carries the exact message,
thread, turn, and target refs.

Focused mobile conversation verification is 14 pass / 47 assertions; the full
mobile suite is 124 pass / 646 assertions and mobile typecheck passes. Emulator
networking was restored after the fault pass. The physical-iOS network-gap and
unlink/revocation actions remain the literal close gate.

### Physical iPhone network-gap and revocation acceptance — 2026-07-13

The authenticated physical iPhone completed the literal remaining row against
the production Sync/runtime surface. With Airplane Mode enabled and Wi-Fi
disabled, mobile admitted marker A and visibly reported that the message and
runtime command were queued pending reconciliation. Production contained no
copy during the gap. After reconnect, the FIFO drained to exactly one durable
message and exactly one runtime turn on the existing Desktop-created thread.
The turn reached `completed` once with five events; no duplicate message, turn,
or terminal output was present.

For the revocation row, the phone returned offline, admitted marker B, and
again displayed the paired queued state. Offline sign-out attempted revocation
without pretending server success. After reconnect, online sign-out completed,
the app returned to its local-device surface, and Settings displayed **Link
OpenAgents account**. Production was queried before and after reconnect and
contained zero messages and zero runtime turns for marker B. The burned queue
therefore did not replay after authority was removed.

Public-safe evidence retains only the queue transition, counts, terminal
classification, and timestamp. The literal marker bodies, credential material,
raw provider events, and private database rows are not part of the public
receipt. The installed bundle also exposed a separate presentation regression:
typing each character dismissed the native keyboard. That renderer defect does
not alter the accepted durable queue/revocation result and is being repaired in
a separately claimed source lane.

## Close decision

#8689 and parent #8677 are accepted. The deterministic matrix, built Desktop
restart/reload, physical named-Codex continuation, pre-event cancellation,
physical network gap, exactly-once FIFO drain, and revocation/no-replay rows all
pass. #8676 shares the same physical continuation and revocation receipt and is
accepted on that evidence.
