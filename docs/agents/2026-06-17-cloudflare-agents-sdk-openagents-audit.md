# Cloudflare Agents SDK OpenAgents Audit

Date: 2026-06-17
Status: architecture audit, no implementation yet
Scope: the local Cloudflare Agents SDK clone at `projects/repos/agents`, plus
current Cloudflare Agents documentation, as a possible OpenAgents runtime for
stateful agents, workrooms, connector events, realtime UI, and durable turns.

## Executive Summary

Cloudflare's Agents SDK is the most production-adjacent runtime candidate in
this audit set if OpenAgents wants to keep agent execution close to the current
Cloudflare Worker product surface.

The SDK is not just a chat helper. It layers an `Agent` class on top of
Durable Objects and PartyServer, then adds persistent state, SQLite, WebSocket
state sync, callable RPC, scheduling, queues, durable fibers, Workflows, MCP,
email, browser tooling, chat recovery, and React hooks. That makes it a strong
candidate for OpenAgents workroom agents, connector-event actors, operator
dashboards, and durable agent turn admission.

It should still not replace OpenAgents authority. Identity, workspace access,
GitHub write grants, payment/credit/payout authority, accepted-work records,
and public projections should remain in OpenAgents-owned Worker code. Agents
SDK instances can own bounded runtime state and realtime coordination only
after trusted OpenAgents code selects the user, workspace, repo, issue, agent
instance key, and grant.

Relative to the sibling audits:

- **Flue** is best framed as a connector and agent-harness sidecar candidate.
- **Rivet** is an independent Effect-native actor substrate candidate.
- **Cloudflare Agents SDK** is the Cloudflare-native Durable Object agent
  runtime candidate.

The recommended next step is a small non-authoritative OpenAgents pilot: one
private workroom/session agent behind the existing OpenAgents auth gate, no
generic `/agents/...` public routing, no provider credentials, no raw private
repo persistence, and a focused `startFiber()` or state-sync proof.

## Sources Reviewed

- Cloudflare Agents documentation:
  - <https://developers.cloudflare.com/agents/>
  - <https://developers.cloudflare.com/agents/runtime/agents-api/>
  - <https://developers.cloudflare.com/agents/runtime/lifecycle/agent-class/>
  - <https://developers.cloudflare.com/agents/runtime/lifecycle/state/>
  - <https://developers.cloudflare.com/agents/runtime/lifecycle/callable-methods/>
- Local reference clone:
  - `projects/repos/agents/README.md`
  - `projects/repos/agents/AGENTS.md`
  - `projects/repos/agents/docs/index.md`
  - `projects/repos/agents/docs/agent-class.md`
  - `projects/repos/agents/docs/state.md`
  - `projects/repos/agents/docs/routing.md`
  - `projects/repos/agents/docs/callable-methods.md`
  - `projects/repos/agents/docs/durable-execution.md`
  - `projects/repos/agents/docs/queue.md`
  - `projects/repos/agents/docs/workflows.md`
  - `projects/repos/agents/docs/chat-agents.md`
  - `projects/repos/agents/docs/server-driven-messages.md`
  - `projects/repos/agents/docs/resumable-streaming.md`
  - `projects/repos/agents/docs/mcp-client.md`
  - `projects/repos/agents/docs/mcp-servers.md`
  - `projects/repos/agents/docs/webhooks.md`
  - `projects/repos/agents/docs/email.md`
  - `projects/repos/agents/docs/observability.md`
  - `projects/repos/agents/docs/configuration.md`
  - `projects/repos/agents/docs/cross-domain-authentication.md`
  - `projects/repos/agents/docs/readonly-connections.md`
  - `projects/repos/agents/packages/agents/package.json`
  - `projects/repos/agents/packages/agents/src/index.ts`
  - `projects/repos/agents/packages/agents/src/react.tsx`
  - `projects/repos/agents/packages/agents/src/workflows.ts`
  - `projects/repos/agents/examples/github-webhook/src/server.ts`
  - `projects/repos/agents/examples/workflows/src/server.ts`
  - `projects/repos/agents/examples/assistant/src/server.ts`
  - `projects/repos/agents/examples/assistant/agents/assistant/agent.ts`
  - `projects/repos/agents/examples/resumable-stream-chat/src/server.ts`

## What Cloudflare Agents SDK Is

The local repo describes Agents as persistent, stateful execution environments
for agentic workloads, powered by Cloudflare Durable Objects. The core model is
"one agent per user, session, room, task, or conversation", with hibernation
when idle and wake-on-demand.

The core package is `agents` v0.16.1 in the local clone. Its public export
surface includes:

- `agents`: `Agent`, `routeAgentRequest`, `getAgentByName`, connections,
  state, SQL, scheduling, queue, retries, MCP client plumbing, email, and
  routing helpers.
- `agents/react`: `useAgent`, state sync, RPC from React, sub-agent addressing,
  and client identity callbacks.
- `agents/workflows`: `AgentWorkflow`, typed access back to the originating
  Agent, progress reporting, approval flows, and workflow state callbacks.
- `agents/mcp` and `agents/mcp/client`: MCP server and client support.
- `agents/email`: routing and signed reply helpers for Cloudflare Email
  Service.
- `agents/observability`: diagnostics-channel event subscriptions.
- Experimental or adjacent surfaces for browser automation, skills, x402,
  Code Mode, and AI chat compatibility.

The package is fast-moving and the repo explicitly marks some surfaces as
experimental. Treat the raw `Agent` class and mature integrations as the
initial production candidates; treat Think, browser tools, voice, Code Mode,
and workspace execution as later or reference-only until proven in a narrow
OpenAgents pilot.

## Local Source Findings

### Agent Is Durable Object Infrastructure, Not A Separate Control Plane

The docs and source are clear that `Agent` is a Durable Object-backed class.
It extends PartyServer's `Server`, which wraps Durable Object lifecycle and
WebSocket handling. The SDK adds state, SQL, scheduling, queueing, MCP, email,
workflows, and observability on top.

This is a good fit for OpenAgents because it can live inside the same
Cloudflare deployment model as the current Worker surface. It is not a new
system of record unless we make it one.

### Routing Is Powerful And Dangerous

`routeAgentRequest()` maps URLs like:

```text
/agents/{agent-name}/{instance-name}
```

to Durable Object instances. `getAgentByName()` gets a specific instance for
trusted server-side RPC or request forwarding.

For OpenAgents, the default public router is not an acceptable first boundary.
The `examples/assistant/src/server.ts` file shows the better shape: the Worker
handles GitHub auth, maps the signed-in user to an `AssistantDirectory`, routes
only through `/chat`, and deliberately avoids a fallback to
`routeAgentRequest()` so clients cannot directly address
`/agents/assistant-directory/<login>` or child chat facets.

That is the pattern OpenAgents should copy. Trusted code should decide the
agent class and instance key. Browsers should not be able to invent workroom,
repo, issue, or user agent keys by URL.

### State Sync Needs OpenAgents-Owned Validation

Agent state is persisted to SQLite and synchronized to all connected WebSocket
clients. Both server and clients can update state, and `validateStateChange()`
can reject invalid updates before persistence and broadcast.

That is useful for operator dashboards and workroom presence/progress, but the
SDK does not make state schema-validated by default. State is JSON-serialized
and TypeScript-typed at compile time. OpenAgents should define explicit runtime
validation for any state that crosses trust boundaries.

The `readonly-connections` docs are important: readonly connections block
client-originated `setState()` and any callable method that eventually calls
`this.setState()`. They do not stop side effects that happen before a state
write. For authority-bearing methods, permission checks must run before any
email, GitHub, payment, workflow, filesystem, or provider side effect.

### Callable RPC Is Client Surface, Not Internal Authority

`@callable()` exposes methods to WebSocket clients. The docs explicitly
separate this from Worker-to-agent or agent-to-agent calls, where normal
Durable Object RPC is preferred and no decorator is needed.

That separation matters. OpenAgents can expose small readonly or bounded
callables to UI clients, but writeback and authority paths should be internal
DO RPC or Worker routes that have already resolved trusted grants.

The decorator surface also has build constraints:

- use the `agents/vite` plugin when using decorators in Vite;
- extend `agents/tsconfig` or keep an ES2021-compatible target;
- do not enable TypeScript legacy `experimentalDecorators`.

Those constraints are manageable, but they belong in any implementation issue.

### Durable Fibers Are A Strong Fit For Accepted Work

`runFiber()` and `startFiber()` are the most OpenAgents-relevant primitives in
the SDK. They persist work records in SQLite, keep the Durable Object alive
during execution, support `stash()` checkpoints, and call `onFiberRecovered()`
when a Durable Object was evicted mid-task.

`startFiber()` is especially useful for webhooks and external retries. It can
durably accept work with an idempotency key, return quickly, and let callers
inspect or recover status later. This maps well to OpenAgents connector events,
approved writebacks, and workroom jobs.

The caveat is important: interrupted closures are not automatically replayed.
Recovery code must use the fiber name, snapshot, idempotency key, and metadata
to resume, compensate, or leave a record for inspection. OpenAgents must treat
that recovery policy as product logic, not magic runtime behavior.

### Workflows Are For Longer Multi-Step Jobs

The Workflows integration draws a useful line:

- Agents handle realtime state and WebSocket coordination.
- Cloudflare Workflows handle long-running, retryable, multi-step work and
  human approval waits.
- `AgentWorkflow` gives workflow code typed access back to the originating
  Agent and step helpers for durable progress updates.

For OpenAgents, this is a good fit for approved work execution, operator
approval gates, background artifact processing, and long-running connector
jobs. Use `queue()` or `startFiber()` for smaller work; use Workflows for
multi-step processes where retries, pauses, and approval waits are first-class.

### AI Chat And Think Are Useful, But Retention-Heavy

`@cloudflare/ai-chat` provides `AIChatAgent` with message persistence,
resumable streaming, tool support, data parts, row-size protection, and React
hooks. `Think` layers a more opinionated agentic loop on top, with sessions,
tools, memory, compaction, recovery, and multi-channel delivery.

The recovery features are compelling:

- stream chunks persist to SQLite so reconnecting clients can replay buffered
  chunks;
- `chatRecovery` can run turns inside fibers so a Durable Object eviction can
  be detected and handled;
- `waitUntilStable()` protects server-driven turns from racing active streams
  or pending tool approvals;
- `submitMessages()` gives Think a durable, idempotent turn admission path.

The risk is the same as in the Rivet audit: message persistence is convenient
and dangerous. OpenAgents should not put raw private repo contents, raw
provider payloads, raw shell logs, raw prompts, secrets, or sensitive workspace
material into persistent chat history by default. A pilot should use redacted
envelopes, trusted IDs, bounded excerpts, hashes, and retention caps.

### MCP, Email, Webhooks, And Browser Tools Need Authority Wrappers

The SDK has broad integration surfaces:

- MCP client connections with OAuth and SSRF checks for remote URLs.
- `McpAgent` for stateful MCP servers.
- Email send/receive with signed reply routing.
- Webhook examples that route provider events to per-entity agents.
- Browser and Code Mode tools for agents that inspect pages or execute code.

These should not be exposed as generic model-callable tools in OpenAgents.
OpenAgents trusted code must choose the provider account, grant, repository,
workspace, issue, target email, browser session, or sandbox. The agent can
draft, classify, plan, or request an action; the authority layer decides if it
is allowed.

The local `github-webhook` example is useful as a teaching reference, but not
production-shaped for OpenAgents. It verifies GitHub signatures when a secret
is configured and stores derived events, but it creates random event IDs rather
than claiming GitHub delivery IDs for idempotency. OpenAgents needs delivery
dedupe before agent dispatch.

### Observability Is Better Than Ad Hoc Logging

The SDK emits diagnostics-channel events for state, RPC, message lifecycle,
chat recovery, durable fibers, schedule/queue, lifecycle, workflow, MCP, and
email. In production those can flow to Tail Workers.

OpenAgents should use this instead of inventing ad hoc logs. Required
correlation fields should include workspace id, run id, connector event id,
GitHub delivery id, actor/agent instance key, and public projection id where
applicable.

## OpenAgents Fit

Good first fits:

1. A private workroom-status agent that owns redacted realtime state, connected
   clients, progress updates, and readonly operator views.
2. A durable connector-event agent downstream of OpenAgents or Flue signature
   verification, delivery-id claim, and event redaction.
3. A `startFiber()` pilot for accepted webhook/workroom work with
   idempotency-key dedupe and explicit recovery policy.
4. A Workflows pilot for human-in-the-loop approved jobs where the Agent owns
   realtime UI state and Workflows own durable steps.
5. A non-production `AIChatAgent` or Think pilot with redacted transcripts and
   `chatRecovery` enabled, used to prove stream recovery and turn admission.

Bad first fits:

- A public catch-all `routeAgentRequest()` for OpenAgents production routes.
- Generic client-callable GitHub, Stripe, email, filesystem, browser, or MCP
  tools.
- Persisting raw private workspace content or raw model transcripts in agent
  state or chat history.
- Moving auth, membership, billing, accepted-work, payout, GitHub writeback, or
  public projection authority into agent classes.
- Treating readonly connections as a complete authorization system.

## Architecture Boundary

Cloudflare Agents should sit behind OpenAgents-owned authority:

```text
browser / provider / worker event
  -> OpenAgents verifies session, workspace, repo, provider, and grant
  -> OpenAgents claims idempotency key or delivery id
  -> OpenAgents reduces payload to a bounded envelope
  -> trusted Worker code resolves the Agent class and instance key
  -> Agent updates redacted runtime state, runs a fiber, or starts a workflow
  -> Agent emits draft/progress/status events
  -> OpenAgents records authoritative result or public projection
  -> outbound provider mutation uses OpenAgents-selected credentials
```

For the Flue GitHub connector roadmap, Cloudflare Agents can be a downstream
runtime:

```text
GitHub webhook
  -> Flue or OpenAgents verifies signature
  -> delivery id is claimed
  -> OpenAgents maps repo/issue/PR to workspace/work order
  -> OpenAgents records sanitized connector event
  -> Cloudflare Agent receives a typed internal call keyed by trusted refs
  -> Agent drafts, coordinates, or accepts durable work
  -> approved writeback goes through OpenAgents GitHub authority
```

The route and key selection rule is non-negotiable: no model, browser, webhook,
or prompt should be able to choose arbitrary durable object instance names for
authority-bearing data.

## Comparison With Flue And Rivet

| Runtime | Best Role | Strength | Main Risk |
| --- | --- | --- | --- |
| Flue | Connector/agent sidecar | Provider channels, agent harness, deploy targets | Do not move OpenAgents authority or credentials into connector sessions |
| Rivet | Effect-native actor substrate | Typed actions/state, actor lifetime, testable Layers | Beta SDK, raw context escape hatch, separate runtime/cloud choice |
| Cloudflare Agents SDK | Cloudflare-native DO agent runtime | Native Workers/Durable Objects, state sync, Workflows, MCP, fibers, React hooks | Direct routing, persistent raw data, fast-moving experimental surfaces |

If OpenAgents stays Cloudflare-native, Agents SDK has the shortest production
path. Rivet remains valuable as a typed actor reference and possible
alternative substrate. Flue remains valuable for connector ingress and agent
sidecar experiments.

## Risks And Required Guardrails

- **Direct routing risk:** do not expose generic `/agents/...` production
  routing. Route through OpenAgents auth and trusted lookup first.
- **State validation risk:** TypeScript generics are not runtime validation.
  Add OpenAgents-owned schemas for client-writable state and callable payloads.
- **Readonly caveat:** readonly blocks `setState`, not arbitrary side effects.
  Check authority before any external mutation.
- **Persistence risk:** state, chat messages, stream chunks, MCP OAuth tokens,
  and workflow metadata can persist in Durable Object SQLite. Define retention,
  redaction, and secret-storage rules before production.
- **Recovery policy risk:** fibers make interruptions visible and recoverable,
  but application code still owns resume/compensate/inspect decisions.
- **Migration risk:** every Agent class needs Durable Object bindings and
  SQLite migrations. Old migrations must not be edited.
- **Build risk:** callable decorators require TC39 decorator handling and the
  `agents/vite` transform in Vite projects.
- **Experimental surface risk:** Think, browser tools, voice, Code Mode, and
  workspace execution are moving quickly. Use them as references before
  authority-bearing adoption.
- **Cloudflare lock-in risk:** this is the most native option, but also the
  most Cloudflare-specific option.
- **Routing invariant risk:** do not add ad hoc keyword routing to choose
  users, repos, workrooms, or agent instances. Use trusted IDs or an approved
  typed semantic selector.

## Recommended Pilot Order

1. **Docs-only acceptance:** use this audit as the baseline. Do not start with
   a broad migration.
2. **Minimal internal Agent:** add one non-public workroom/session Agent behind
   existing OpenAgents auth. No `AIChatAgent`, no provider tools, no generic
   route fallback.
3. **Readonly operator view:** connect the web UI through `useAgent` with
   readonly connections for observer roles and explicit auth checks for any
   mutating callable.
4. **Durable accepted work:** wrap one connector/workroom event in
   `startFiber()` with an idempotency key and explicit `onFiberRecovered`
   behavior.
5. **Workflow approval spike:** prove one Agent-started Workflow with progress,
   pause/approval, completion, and UI updates.
6. **GitHub connector bridge:** after Flue/OpenAgents signed ingress and event
   recording are stable, feed sanitized repo/issue events into a trusted-keyed
   Agent instance.
7. **Chat recovery experiment:** only after redaction rules exist, test
   `AIChatAgent` or Think with `chatRecovery`, `waitUntilStable`, and
   retention caps.
8. **Production gate:** add invariants, migrations, Tail Worker observability,
   Workers-runtime tests, and rollback/cleanup plans before shipping a
   user-visible runtime dependency.

## Decision

Use Cloudflare Agents SDK as the leading Cloudflare-native runtime candidate
for OpenAgents agent/workroom pilots. Keep it behind OpenAgents authority.

The strongest immediate value is a bounded, authenticated, non-authoritative
Durable Object agent that proves realtime state sync and durable accepted work
without moving identity, GitHub, billing, or public projection authority out of
the current OpenAgents system of record.
