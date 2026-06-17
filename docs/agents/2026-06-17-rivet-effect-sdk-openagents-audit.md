# Rivet Effect SDK OpenAgents Audit

Date: 2026-06-17
Status: architecture audit, no implementation yet
Scope: Rivet's June 16, 2026 Effect SDK announcement and the local
`projects/repos/rivet` clone as a possible OpenAgents actor/runtime substrate.

## Executive Summary

Rivet's Effect SDK is worth a serious OpenAgents pilot as a stateful actor
substrate for long-running agent sessions, workroom event loops, realtime
collaboration, and sandbox/workflow coordination.

It should not replace the `openagents.com` Worker, product authority, payment
authority, GitHub write authority, private workspace membership, public
projection rules, or existing Effect/Foldkit product architecture.

The best near-term fit is narrower: use Rivet as a candidate runtime for one
bounded, non-authoritative actor surface where a single logical agent or
workroom owns durable state, typed actions, realtime events, background fibers,
and testable Effect services. Keep OpenAgents as the system of record and treat
Rivet actor state as derived, redacted, and revocable until production
boundaries are proven.

Relative to the Flue audit, the split is:

- Flue is a connector and agent-harness candidate, especially for third-party
  webhook/channel ingress.
- Rivet is an actor-runtime candidate, especially for durable per-agent,
  per-session, per-workroom, or per-sandbox state and realtime fanout.
- OpenAgents remains the authority layer for identity, repo/workspace binding,
  billing, accepted-work records, credentials, and public-safe projections.

## Sources Reviewed

- Rivet changelog, June 16, 2026:
  <https://rivet.dev/changelog/2026-06-16-introducing-the-effect-sdk/>
- Local reference clone: `projects/repos/rivet`
- Top-level Rivet README: `projects/repos/rivet/README.md`
- Effect counter example:
  - `projects/repos/rivet/examples/hello-world-effect/README.md`
  - `projects/repos/rivet/examples/hello-world-effect/src/actors/counter/api.ts`
  - `projects/repos/rivet/examples/hello-world-effect/src/actors/counter/live.ts`
- Effect AI agent example:
  - `projects/repos/rivet/examples/ai-agent-effect/README.md`
  - `projects/repos/rivet/examples/ai-agent-effect/src/actors/agent/api.ts`
  - `projects/repos/rivet/examples/ai-agent-effect/src/actors/agent/live.ts`
  - `projects/repos/rivet/examples/ai-agent-effect/src/model.ts`
  - `projects/repos/rivet/examples/ai-agent-effect/tests/agent.test.ts`
- Effect chat room example:
  - `projects/repos/rivet/examples/chat-room-effect/src/actors/chat-room/api.ts`
  - `projects/repos/rivet/examples/chat-room-effect/src/actors/chat-room/live.ts`
- Effect SDK package:
  - `projects/repos/rivet/rivetkit-typescript/packages/effect/src/Action.ts`
  - `projects/repos/rivet/rivetkit-typescript/packages/effect/src/Actor.ts`
  - `projects/repos/rivet/rivetkit-typescript/packages/effect/src/State.ts`
  - `projects/repos/rivet/rivetkit-typescript/packages/effect/src/Registry.ts`
- Existing OpenAgents Flue docs for comparison:
  - `openagents/docs/flue/2026-06-16-flue-framework-openagents-audit.md`
  - `openagents/docs/flue/2026-06-16-flue-github-web-ui-integration-roadmap.md`

## What Rivet Is Claiming

Rivet Actors are long-running, lightweight processes with durable state. The
announced SDK maps Effect primitives onto that model:

- `Action.make` defines value-level action contracts with Effect `Schema`
  payload, success, and error channels.
- `Actor.make` defines the public actor contract, while `Actor.toLayer` binds
  server-only wake-scope implementation.
- Actor wake maps cleanly to Effect `Scope`: finalizers run when the actor
  sleeps, and `forkScoped` background fibers cancel on sleep.
- `State` presents a `Ref` / `SubscriptionRef`-shaped API over persisted actor
  state.
- `Layer` and `Context.Service` provide normal Effect dependency injection
  inside the actor wake scope and handlers.
- Tagged schema errors cross the wire as typed failures, not opaque exceptions.
- The typed client is derived from the actor contract and can be used by
  external clients or actor-to-actor RPC.
- The same typed client works in tests through `Registry.test`.

The announcement also says the SDK is beta. Typed support exists today for
actions, state, clients, typed errors, logging, sleep, actor address, and the
registry. Events, broadcast, schedule, embedded SQLite, queues, connections,
and some lifecycle hooks are available through `rawRivetkitContext` until typed
wrappers land.

## Local Source Findings

### Contract and Implementation Split

The `hello-world-effect` counter example is the cleanest minimum shape:

- `api.ts` exports `Action.make` values and the `Counter` actor contract.
- `live.ts` implements `Counter.toLayer`, declares the state schema and
  initial state, and returns action handlers from the wake scope.
- Negative increments fail through `NegativeAmountError` before state mutation.
- Successful increments use `State.updateAndGet` and broadcast the new count.

This split is directly useful for OpenAgents. The contract file can be shared
with clients, internal callers, and tests without importing provider wiring,
secrets, database setup, or server-only implementation details.

### State Semantics

The SDK's `State.ts` is not just a convenience wrapper. It serializes writes
through a semaphore so `set`, `update`, `updateAndGet`, and `modify` linearize
the read/apply/write sequence. It also exposes `changes` as a stream backed by
a replaying pubsub, so wake-scoped fibers can react to committed state changes.

That is attractive for workroom UI and operator surfaces because an actor can
own a small piece of durable state and publish changes without a separate
pub/sub service. It is still not a substitute for OpenAgents' authoritative
database records. Actor state should be treated as local actor memory unless we
explicitly promote it into an OpenAgents-owned ledger or projection.

### AI Agent Pattern

The `ai-agent-effect` example persists conversation history as actor state and
requires Effect AI's `LanguageModel` service from context. The concrete model
is provided as a `Layer`; production wiring uses `OpenAiLanguageModel`,
`OpenAiClient`, and `FetchHttpClient`, while tests provide a mock
OpenAI-compatible server without changing actor code.

That pattern fits OpenAgents agents well:

- one actor per conversation, issue, workroom, or sandbox session;
- model provider selected outside the actor implementation;
- typed action boundary for user turns and history reads;
- local, in-process tests with the same typed client;
- persistent memory across restarts.

The same pattern is also a data-risk trap. Persisting the full prompt/history
inside actor state is convenient, but OpenAgents must not persist raw private
repo content, raw prompts, provider payloads, broad credentials, or raw runner
logs in a third-party or experimental actor store by default. Any Rivet pilot
needs a redacted state schema and retention policy from day one.

### Raw Context Boundary

The `chat-room-effect` example shows the current boundary of the beta SDK:

- typed actions and typed errors are Effect-native;
- actor address, services, state changes, and finalizers are Effect-native;
- broadcast, scheduling, SQLite, destroy, and parts of the larger actor toolbox
  still go through `rawRivetkitContext`.

That does not block a pilot, but it means authority-bearing paths should not
lean on raw context directly. If we adopt Rivet beyond a spike, we should wrap
the raw context operations we use behind OpenAgents-owned typed modules before
they can write external state, schedule user-visible work, or publish public
events.

### Testing Posture

`Registry.test` is good enough for real actor tests because it starts the
RivetKit registry and provides a typed client. The source notes one caveat:
the public RivetKit registry does not expose a clean shutdown API today, so the
registry is effectively leaked until process exit while the client is disposed.

That is acceptable for focused test suites, but it matters for long-running
workspace tests or test matrices. A pilot should keep Rivet tests isolated and
avoid assuming perfect per-test runtime teardown until the upstream lifecycle
is cleaner.

## OpenAgents Fit

Good first fits:

1. A non-authoritative workroom actor that owns redacted session state,
   realtime event fanout, and action handlers for one workroom key.
2. A GitHub issue or PR discussion actor that receives already-sanitized
   connector events from the OpenAgents or Flue sidecar path and drafts
   responses without owning GitHub credentials.
3. A sandbox orchestration actor that tracks a workspace's ephemeral run state,
   queue, cleanup timers, and operator-visible progress.
4. A local operator demo actor for typed actions, state changes, and
   actor-to-actor calls, with no production secrets or private content.

Bad first fits:

- Replacing `apps/openagents.com`.
- Owning GitHub OAuth/App installation secrets.
- Owning payment, credit, payout, or accepted-work authority.
- Persisting raw private workspace data or raw model transcripts.
- Using raw context as an unreviewed route to schedule, broadcast, destroy, or
  mutate public-facing state.

## Architecture Boundary

Rivet should sit behind OpenAgents-owned authority:

```text
provider event / user action
  -> OpenAgents verifies identity, workspace, repository, and authority
  -> OpenAgents reduces input to a public-safe or private-bounded envelope
  -> Rivet actor receives typed action with only the bounded envelope
  -> actor updates redacted local state and emits draft/progress events
  -> OpenAgents records any authoritative result or public projection
  -> outbound provider writeback uses OpenAgents-selected credentials
```

For the Flue GitHub connector path, Rivet could be either downstream of Flue or
an alternative actor runtime for connector sessions:

```text
GitHub webhook
  -> Flue or OpenAgents verifies signature and claims delivery id
  -> trusted code maps repo/issue/PR to OpenAgents workspace/work order
  -> sanitized connector event is recorded by OpenAgents
  -> Rivet actor receives typed event action keyed by repo/issue/PR
  -> actor drafts or coordinates work
  -> approved writeback goes through OpenAgents GitHub authority
```

The important rule is that model-selected actor keys, tool parameters, or
prompt content are never authorization. Trusted OpenAgents code must select the
workspace, repository, issue, actor key, provider account, and writeback grant.

## Risks And Required Guardrails

- **Beta API risk:** `@rivetkit/effect` may change. Keep the first pilot small
  and reference-oriented.
- **Raw context risk:** broadcast, schedule, SQLite, queues, connections, and
  lifecycle hooks are not all wrapped in Effect-native typed APIs yet. Wrap
  used raw operations before authority-bearing use.
- **Data retention risk:** AI-agent memory wants to persist complete histories.
  Use redacted state schemas, bounded excerpts, content hashes, or OpenAgents
  references instead of raw private data.
- **Authority drift risk:** Actors make it easy to own state. OpenAgents must
  keep identity, membership, repo mapping, billing, accepted work, and public
  projection authority outside Rivet.
- **Test lifecycle risk:** `Registry.test` has a runtime cleanup caveat. Keep
  early tests isolated and avoid broad process-sharing assumptions.
- **Operational dependency risk:** Production use needs an explicit choice
  between local/library mode, self-hosted Rivet engine, and Rivet Cloud, plus
  deployment, logging, tracing, data residency, and failure-mode decisions.
- **Routing invariant risk:** Do not add ad hoc keyword routing to select
  actors, repos, workrooms, or users. Use trusted IDs or a typed semantic
  selector already approved by OpenAgents.

## Recommended Pilot Order

1. **Docs-only acceptance:** keep this audit as the decision baseline and do
   not start a production migration from the announcement alone.
2. **Local spike:** create a tiny owned OpenAgents lab package or example that
   defines one actor with `Action.make`, `Actor.toLayer`, typed errors,
   schema state, and `Registry.test`.
3. **Redacted state contract:** define an OpenAgents actor-state invariant
   before any AI memory pilot. State may contain trusted IDs, public-safe
   excerpts, and hashes; raw secrets, raw private repo bodies, raw prompts, and
   raw provider payloads are out of scope.
4. **Non-authoritative workroom actor:** pilot one workroom/session actor that
   receives sanitized input and emits draft/progress events only.
5. **GitHub connector bridge:** after the Flue GitHub connector path proves
   signed ingress and OpenAgents event recording, test a Rivet actor keyed by
   OpenAgents-selected repo/issue refs.
6. **Typed wrappers for raw operations:** wrap any needed broadcast, schedule,
   SQLite, or destroy operations behind OpenAgents modules before production.
7. **Production decision gate:** decide between self-hosting and Rivet Cloud,
   define observability, failure modes, backup/retention, and data residency,
   then promote only if the pilot proves value over Cloudflare Durable Objects
   or the existing OpenAgents Worker architecture.

## Decision

Use Rivet as a serious reference and pilot candidate for Effect-native actors.
Do not route immediate OpenAgents product authority through it.

The strongest immediate value is not "replace our backend." It is proving
whether a typed, Effect-native, durable actor can simplify one long-running
agent or workroom loop while preserving the OpenAgents authority boundary.
