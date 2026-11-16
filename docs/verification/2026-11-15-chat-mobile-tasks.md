# Mobile native task activity and retained permission controls

Source checkpoint for [#13620](https://github.com/OpenAgentsInc/openagents/issues/13620),
2026-11-15. The account/Project chat now reads its saved native task association
and presents the actual computer, engine, model, effort and native lifecycle.
**Run details** opens tools and permission notices for a saved turn. Reconnect
reads the original task; it never sends another Start.

The client reuses the primary Binding, TaskStatus/TaskRequest and transcript
codecs, shared `XhClient`, complete capture adapter and inert Session reader.
The API's existing capture adapter moved into `coax/lib/agent/environment/`
without changing its procedure bodies. API and mobile now share it. The worker's
status normalizer moved beside the existing CLI task policy. The CLI's HTTP
transcript route builder and Microcoder's UTF-8 cut helper likewise moved to
their shared protocol/text modules.

Only complete, digest-verified and decoded captures publish. Two caller-owned
Session pools keep the previous valid view during a failed refresh. One reader
identity remains bound to the selected task; each capture has a fresh read ID
and retains its exact next byte cursor. A bad or expired capture restarts the
read. The existing whole-capture cap is 4 MiB; oversized content refuses rather
than publishing a prefix. Tool input/output uses explicit, contiguous UTF-8
pages of at most 49,152 bytes, with native call IDs and page copy controls.

Allow requires the whole current permission capture and its exact canonical
TaskStatus. Deny does not depend on a successful transcript decode. Permission
intent pins the original revision and interaction before requesting entropy;
a lost HTTP reply retains the exact command bytes. Explicit retry uses those
bytes. Reconciliation accepts only an authoritative outcome or advancement of
the original held interaction.

Chat Stop retains its independent saved intent. A prepared permission that was
never offered to HTTP is retired before Stop. A delivered permission command
keeps its bytes and becomes uncertain, preventing an automatic permission post
after Stop. Neither a button press nor a connection failure marks native work
as confirmed canceled.

Reads pause under navigation/settings and use the existing installed wait
service for zero-delta frames. UIKit's regular frame clock does not count the
same wait interval twice. Account reset zeros the capture, decode and both
publication planes. The fixed JavaScript memory allowance is 64 MiB; the
generated account program is admitted under a fixed 16 MiB script cap. These
bounds do not include OS, SDK or renderer allocations.

## Evidence scope

The pure reducer is mutation-qualified through the existing suite inventory:
`coax/remote/check.sh only coax/native/chat-task-checks.coax BASE`. Its current
gate passes 283 checks and all 550 changed-procedure mutants: 525 killed,
25 reviewed equivalents and zero unresolved survivors. Equivalent reasons are
at their source sites; they concern complete primary-codec length boundaries
and already-enforced monotone status constraints.

The actual generated protobuf mapper is a runtime boundary. Compile-time
evaluation refuses its storage-overlap operation (E0404); that is not treated
as a valid empty test result. The native qualifier instead compiles the actual
mobile app and shared codecs in debug/release, with and without ASan/UBSan,
using the existing fork/thread native suite driver. Its cases cover owned and
foreign associations, captured permission review, lost-control retry, large
UTF-8 output, copy, private buffer clearing, background/timer behavior, denial
and independent saved Stop.

All six native app cases pass in all four configurations, including zero-delta
wait completion, regular half-second frames during that same wait and Stop
before a prepared permission reaches HTTP. The final native gate plus Wasm
build completes in 218 seconds. The linked CLI builds in 72 seconds. The actual
generated interpreter preflight admits a 33,921,680-byte state at heap offset
8,519,680 in the fixed 67,108,864-byte memory and emits its first component frame.

The complete canonical iOS generator refreshes the shared and world interfaces
before the SDK build. On a separate clean iPhone 17 Pro/iOS 26.5 simulator,
10 unchanged NativeProgram/NativeTranscript cases pass in
`/tmp/oa-chat-task-mobile-r4.xcresult`. The new task case passes in
`/tmp/oa-chat-task-mobile-r7.xcresult`: full multi-page capture, native tool
projection, retained view identity, exact Unicode page boundaries, a real UIKit
Allow button, lost-response retry and authoritative completion. No product
procedure changed between those SDK runs; only the new fixture/driver changed.
The retained screenshot is an offscreen UIKit component projection, not a
foreground app/native GPU rendering receipt.

The SDK fixture comes from the existing 24-check primary codec oracle:
`coax/web/tool.sh coax/web/api/account-chat-task-oracle.coax --export-task coax/native/fixtures/chat-task-v1alpha1.payload.json`.
The export retains the independently hashed report bytes. The test sends that
whole body directly, just as the installed HTTP adapter does; Foundation
dictionary reserialization would change the signed object's byte identity.

The freshly linked PostgreSQL/ASan/UBSan API smoke completes in 512 seconds,
including schema/migration agreement, health, 72 signed CLI authentication
checks and the ordinary animation/realm checks. The native HTTP/CLI behavior's
118-case receipt remains the unchanged implementation recorded at
`bcb17d3cb41c48bcf639aaea2358da9bd0378b03` in
[the dispatch checkpoint](2026-11-15-chat-native-dispatch.md); it is not claimed
as a new run here. The helper `native_chat_checks.coax` is loaded by
`durable_chat_checks.coax`, not a separately runnable API suite selection.

#13620 stays open. Real staging provider execution with an expected patch/test
result, the shared gateway tool loop/strict outputs, SDK catalog pagination
and multiple accounts, durable server capture assembly and expired Stop
renewal remain. This checkpoint does not establish production rollout.
TestFlight remains deferred until the full Claude-port and Arcade goal is done.
