The Codex app-server is effectively Codex-as-a-service over stdio, and it maps
cleanly onto what Autopilot wants: a controllable agent runtime with a stable
protocol, streaming events, approvals, tools, and persistence. If you treat it
as a sidecar runtime rather than a black-box CLI, you get a structured event bus
and an approval surface that you can wire directly into the Autopilot HUD and
policy layer.

## 1) Make Autopilot backend-agnostic via a runtime adapter

Autopilot should treat Codex as one runtime behind a single AgentRuntime
interface. On the Autopilot side, that interface is the minimal lifecycle needed
for real work: start a thread for a repo, start a turn with input, stream events
as they arrive, respond to approvals, and stop cleanly. On the Codex side, those
calls map to `thread/start`, `turn/start`, the stream of `turn/*` notifications,
`review/*` flows, and approval request/response routing. This structure keeps
Autopilot capable of orchestrating Codex, Claude CLI headless, or future custom
agents without rewiring the HUD or task pipeline.

## 2) Turn the event stream into your Autopilot HUD and telemetry spine

The v2 notification stream already includes the semantics Autopilot needs: item
streams, reasoning deltas, plan updates, diffs, token usage, and compaction.
Instead of inventing a parallel trace format, Autopilot can map these events into
its internal timeline and store both the raw JSONL and a normalized event log.
That gives you live HUD panes, metrics such as tool latency and approval wait
time, and a flight recorder that can be replayed for evaluation or debugging.

## 3) Approval gating is the autonomy control surface

Codex emits explicit approval requests for command execution and patch
application. Autopilot can map those into autonomy modes such as read-only,
propose, auto, and escalate. Read-only denies execution; propose allows patch
generation but requires approval to apply; auto pre-approves within policy
constraints; escalate routes higher-risk approvals to a human-in-the-loop UI.
This is exactly the safety lever Autopilot needs without inventing a new policy
surface.

## 4) Use rollout persistence for replay, resume, and evaluation

Rollouts are JSONL sessions that can be replayed, resumed, and analyzed. If
Autopilot preserves those logs and associates them with local session metadata,
it can resume a stuck job, replay a session to understand behavior, and build an
offline evaluation harness. Those same logs are also the raw material for
training data, allowing high-quality trajectories to feed the DSPy improvement
loop.

## 5) Make tools and skills first-class without inventing a new format

Codex already exposes tool orchestration, sandboxes, skills, and MCP
integration. Autopilot can treat its own skills as Codex skills or MCP tools
based on where they live, and it can standardize on capability discovery at the
start of each run. That means the UI and policy layer always see the same tool
surface, and skillpacks can be compiled into Codex-compatible bundles when
needed.

## 6) Build a clean client-embedding story

Because the app-server is JSONL over stdio, Autopilot can embed the runtime in
any product that can spawn a subprocess. That includes a desktop IDE, a terminal
control center, or a server worker that runs paid jobs for the marketplace.
Autopilot stays the mission control layer while the app-server remains a
replaceable engine.

## 7) Multi-backend orchestration inside a swarm

Autopilot can also treat Codex as a primary planner and executor inside a
broader swarm. One Codex thread can run the main repo flow while other runtimes
handle tests, refactors, or documentation. Because the app-server defines clear
item and turn boundaries, Autopilot can schedule work at those boundaries and
merge results back into the main thread with approvals.

---

# The three highest-leverage next steps

First, write a Codex adapter that starts the server, performs the initialize
handshake, starts threads and turns, streams notifications, and routes approvals
back through a policy engine. That adapter should be the only path from Autopilot
into Codex to keep all call sites consistent.

Second, map Codex events into Autopilot's trace format without losing semantic
information. Store both the raw JSONL and the normalized events so replay and
analytics do not depend on reparsing.

Third, define the autonomy policy layer above approvals so that auto-approve,
auto-deny, and human prompts follow a single ruleset. This keeps behavior
consistent across UI and CLI while giving users clear control.

---

# The correct mental model

Codex app-server should run alongside the Rust binary, not inside it. Treat it
as a sidecar agent runtime: a separate process on the same machine, spoken to
through stdio, with no network dependency required. Autopilot is the supervisor
that owns the job graph, autonomy policy, and HUD, while Codex provides the
threads, turns, tool calls, sandboxed execution, approvals, skills, and rollout
persistence.

This separation is important. Embedding Codex as a library would entangle the
lifecycle of Autopilot with Codex internals, make crashes fatal, complicate
upgrades, and undermine the multi-backend story. Running as a subprocess gives
crash isolation, version pinning, hot-swapping between runtimes, and clean kill
or restart semantics.

Conceptually, the Rust code looks like this:

```rust
let mut child = Command::new("codex-app-server")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .env("CODEX_HOME", codex_home)
    .spawn()?;
```

From there, Autopilot writes JSONL requests to stdin, reads JSONL events from
stdout, and keeps a request/response map while routing approvals through the
policy engine. This provides the right foundation for a marketplace-grade
runtime that can evolve without rewriting the core UI or telemetry paths.
