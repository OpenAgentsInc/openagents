# P0 TASK D1-H: real streamed Desktop conversation with mobile continuation

- Issue: #8676
- Parents: #8574, #8597, #8566
- Depends on: closed #8670–#8674 and #8675 D1-G trace acceptance
- Priority: next product milestone after the trace acceptance receipt

## Outcome

Start one real Codex-backed conversation through the provider-neutral,
host-owned Desktop Runtime Gateway; bind its canonical thread, run, message,
and timeline refs; render the live stream in the existing Effect Native
workspace; then continue the same conversation on mobile with one safe
follow-up or interrupt. Restart and reconnect converge without inventing a
second thread or optimistic completion.

This is the first complete live application slice. It is not a FleetRun and
does not wait for broad files/editor/terminal/settings parity.

## Implementation state — 2026-07-11

The deterministic product path is implemented. One canonical runtime turn is
still the execution authority; the server transactionally mirrors it into the
existing `agent_run` / `agent_run_event` projections consumed by both native
clients. There is no second Desktop executor, Pylon, event store, or run
universe.

```text
Effect Native Desktop renderer
  -> Runtime Gateway v6 (tokenless exact refs)
  -> host-owned Khala Sync durable mutation queue
  -> runtime.startTurn (canonical authority + immutable WorkContext snapshot)
  -> standing Pylon runtime-intent consumer (named isolated account selection)
  -> canonical runtime.recordEvent
  -> transactional agent_run / agent_run_event mirror
  -> confirmed thread timeline
  -> Desktop renderer and Expo mobile Home
```

The implemented laws are:

- `threadRef`, owner `messageRef`, and `runRef` are client-chosen once and stay
  identical through admission, dispatch, event projection, Desktop, and
  mobile.
- The starting message body, repository binding when present, and
  `work_context.thread.<threadRef>` are snapshotted on the runtime turn.
  Mutating the chat-thread binding later cannot rewrite the run context.
- An exact semantic retry reconciles as applied without inserting or
  dispatching again. Reusing the same intent/idempotency identity with changed
  semantics rejects as `runtime_intent_conflict`.
- Pylon durably records the deterministic sequence-one `turn.started` claim
  before invoking Codex. A duplicate or indeterminate claim does not start a
  second provider execution.
- Raw provider callbacks remain in the provider host. Sync stores canonical
  `openagents.khala_runtime_event.v1`; clients decode that into bounded
  canonical timeline items and never receive provider credentials, account
  homes, loopback authority, raw payload JSON, or process handles.
- Renderer remount reads the same main-process session. Host restart rebuilds
  the current thread/run/timeline from durable Sync state. Neither path
  invents completion.
- Desktop and mobile use the same shared runtime-intent builders. Mobile sends
  a follow-up to the exact running run or starts a new exact run, can interrupt
  only the confirmed run, and streams later confirmed state while the action
  is pending.
- Proven unlink/revocation closes mutation before queue insertion, burns
  already-queued hosted commands, and retracts subscribed hosted state.
  Transient disconnect remains non-destructive and reconstructible.

## Requirement/evidence map

| Requirement | Enforced implementation and oracle | State |
| --- | --- | --- |
| Durable admission, exact retry/conflict, immutable WorkContext | `packages/khala-sync-server/src/runtime-mutators.ts`, migration `0059`, and the real-Postgres `runtime-mutators.test.ts` | Enforced |
| One provider execution generation | Pylon sequence-one durable claim in `runtime-intent-enforcement.ts`; focused 54-case runtime-intent suite including a two-consumer race | Enforced deterministically |
| Canonical run/event binding | Transactional thread/personal/run-scope mirror plus `khala-sync-client` thread-route discovery | Enforced |
| Tokenless Desktop launch and rich stream | Runtime Gateway v6, `runtime-conversation.ts`, Gateway e2e and renderer tests | Enforced |
| Same-thread mobile continuation/interrupt | Shared `runtime.ts` builders, mobile adapter/Home, mobile conversation test | Enforced |
| Restart/replay and revoke-without-replay | Shared SQLite timeline/session tests and native-host lifecycle | Enforced |
| Real named isolated Codex + built Electron + physical mobile | Public-safe live receipt procedure below | Not yet receipted; required to close |

## Live acceptance and receipt

The issue must stay open until this last environmental acceptance is run. Use
one real, ready, named isolated Codex account (never default `~/.codex`), the
built Electron app signed into the same deployed OpenAgents owner as the Expo
app, and a physical phone.

Current prerequisite check (2026-07-11): the named isolated `codex` account is
ready, but the paired physical iPhone is offline in Xcode device discovery.
That blocks only the live receipt; it is not replaced by a simulator claim.

1. Record the ready account's public-safe name and readiness only. Do not
   record its home, credential, token, or provider session id.
2. In built Electron, create a thread and submit a prompt that necessarily
   yields text and at least one reasoning/tool/plan lifecycle item. Record only
   opaque thread/message/run refs or their hashes, ordered event-kind counts,
   terminal status, and timestamps.
3. Reload the renderer during the turn and verify the same refs continue with
   no duplicate sequence. Restart the host after settlement and verify the
   same confirmed terminal projection reconstructs.
4. On the physical phone, open the same thread and submit one follow-up while
   the run is active, or interrupt the exact confirmed run. Verify both clients
   converge on the same terminal status or explicit
   `unknown_pending_reconcile`.
5. Queue one harmless command offline, then sign out/revoke. Verify the queue
   and hosted projection are gone and reconnect does not replay the command.

The committed public-safe receipt may contain only: date/build/commit,
platform versions, the named account label and `isolated: true`, hashed opaque
refs, event-kind counts, renderer/host restart verdicts, mobile action kind,
terminal/reconciliation verdict, revocation verdict, and overall pass/fail.
It must not contain prompt/response text, repository/path names, owner ids,
raw Sync rows/events, provider metadata, device identifiers, or credentials.

## Required flow

1. Desktop resolves a selected named isolated Codex account and WorkContext in
   the host. No provider credential, account home, loopback token, process
   handle, or raw runtime stream enters the renderer.
2. A registered typed command durably admits a user message with client-chosen
   idempotency, owner scope, exact thread/message refs, delivery mode, and
   authority context before provider dispatch.
3. The Runtime Gateway invokes the canonical request processor and attaches
   provider events to the confirmed `chat_thread` plus `agent_run`/
   `agent_run_event` route binding already consumed by v4. Embedded/local/test
   transports may differ only at the transport and credential edge.
4. Desktop renders connected, heartbeat, text, reasoning summary, plan/tool/
   question/approval/error/usage, stale, reconnect, interrupted, and terminal
   semantics from bounded typed projections.
5. Mobile opens the same confirmed thread/run, submits one safe follow-up or
   interrupt through the shared command contract, and observes the same
   durable outcome.

## Effect and authority laws

- Process, WorkContext, conversation/run, request/command, and renderer/view
  lifetimes remain distinct.
- Provider execution and stream fibers are owned by a Scope that survives a
  renderer remount but closes on the owning run/runtime shutdown.
- Cancellation maps to Effect interruption and executes finalizers; it is not
  converted into an ordinary tool error or success.
- Public command/event/projection values reuse canonical Effect Schema
  identities. No app-local duplicate Thread/Turn/Item schema is introduced.
- `ManagedRuntime` or callback bridges remain at host/provider boundaries, not
  between ordinary application services.
- Model prose, a live socket, and renderer pixels are not command or completion
  authority.

## Acceptance

1. One real Codex conversation starts from Desktop through a named isolated
   account and the provider-neutral Runtime Gateway contract.
2. Durable admission is visible before execution; exact retry reconciles and a
   conflicting reuse of the same message/idempotency identity fails closed.
3. The visible Desktop transcript receives at least text plus one non-text
   typed lifecycle/tool/reasoning item and one terminal outcome with matching
   thread/run/message refs.
4. Killing/restarting the renderer does not stop or duplicate host-owned work;
   restarting the host repairs from confirmed projection/log and resumes live
   delivery honestly.
5. A physical mobile client opens the same refs and performs one follow-up or
   interrupt. Both clients converge on one accepted/rejected/failed outcome or
   explicit `unknown_pending_reconcile`.
6. Sign-out/revocation closes the authenticated session, denies new mutation,
   and leaves no credential or raw provider event in client state/logs.
7. Desktop/mobile behavior contracts, Runtime Gateway contract/e2e tests,
   app typechecks/builds, and public-safe live receipt pass.

## Non-goals

- broad OpenCode workbench parity;
- Fleet control or mixed-provider execution;
- mobile remote workroom/files/terminal breadth;
- a second local server, Pylon, Sync engine, or event database;
- raw provider history replication to mobile.

## Close

Close only after the live provider stream and physical mobile continuation are
accepted. Fixture-only cross-app continuation remains useful evidence but does
not satisfy this issue.
