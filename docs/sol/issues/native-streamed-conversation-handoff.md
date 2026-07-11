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
