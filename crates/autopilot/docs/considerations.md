A lot. This is basically “Codex-as-a-service over stdio,” and that maps *really* cleanly onto what Autopilot wants: a controllable agent runtime with a stable protocol, streaming events, approvals, tools, and persistence. Concretely, you can use it as:

## 1) Make Autopilot backend-agnostic via a Runtime Adapter

Treat the Codex app-server as **one runtime** behind a single Autopilot “AgentRuntime” interface.

* Autopilot side: `start_thread(repo)`, `start_turn(input)`, `stream_events()`, `respond_to_approval()`, `stop()`
* Codex side: `thread/start`, `turn/start`, `turn/* notifications`, `review/*`, approval request/response routing

Result: Autopilot can orchestrate **Codex**, Claude CLI headless, your own agent, etc., with the same HUD + task pipeline.

## 2) Turn the event stream into your Autopilot HUD + Telemetry spine

The best part here is the **structured v2 notifications**: `turn/*`, `item/*/delta`, `turn/plan/updated`, `turn/diff/updated`, token usage updates, errors, context compaction.

That’s basically a ready-made “agent trace bus.” You can:

* Drive your live HUD panes (plan pane, diff pane, logs pane, tool pane)
* Compute APM-style metrics (latency per item, tool call rates, approval wait time, tokens per phase)
* Persist “Flight Recorder” logs using the rollout JSONL as your canonical trace format (or map it into your ATIF-like format)

## 3) Approval gating = your “Autonomy Levels” control surface

Codex already has explicit approval events (`ExecApprovalRequestEvent`, `ApplyPatchApprovalRequestEvent`) and server-initiated requests with routing.

You can use this to implement Autopilot autonomy modes without reinventing:

* **Read-only:** deny `command/exec`, deny patch apply
* **Propose:** allow patch generation, require approve to apply
* **Auto:** pre-approve within policy (e.g., tests only, `git diff` only, no network)
* **Escalate:** route certain approvals to “human-in-the-loop” UI or Slack/whatever

This is exactly the control mechanism you need for “drop-in replacement” positioning: the runtime exposes the safety levers.

## 4) Use its rollout persistence for replay, resume, and evaluation

Rollouts are JSONL sessions with resume helpers and thread history reconstruction.

That enables Autopilot features that usually take forever:

* **Resume a stuck job** (recreate thread state from rollouts)
* **Deterministic-ish replay** for debugging (“why did it do that?”)
* **Offline eval harness**: run the same repo/task across runtimes and compare traces
* **Dataset collection**: good trajectories → training/eval data (your HillClimber loops, regression suites)

## 5) Make “tools” and “skills” first-class in Autopilot (without inventing a new format)

Codex has:

* tool orchestration + sandboxes (`core/src/tools`, `exec_policy`, platform sandboxes)
* skills manager + injection (`skills/*`, `skills/list`)
* MCP integration and grouping

You can piggyback on this by:

* Treating “Autopilot Skills” as either Codex skills *or* MCP tools, depending on where you want them to live
* Standardizing on “capabilities discovery” (`skills/list`, `model/list`, MCP status) at the start of every run
* Keeping your own “skillpack” format, but providing a compiler/mapper into Codex skill files if you want compatibility

## 6) A clean “client embedding” story for your own products

Because it’s JSON-RPC-ish over stdio (JSONL), you can embed the agent in:

* your desktop IDE (“Commander” / OANIX)
* your TerminalBench Command Center
* a server worker that runs jobs for marketplace contributors
* even mobile (spawn as a sidecar) if you ever go there

Autopilot can become the “mission control” while Codex app-server is the “engine,” with minimal coupling.

## 7) Multi-backend orchestration: Codex as “planner/executor” inside a swarm

If Autopilot’s job graph wants multiple workers:

* Use Codex thread as the “main conductor” for a repo
* Spawn other runtimes for specific subtasks (tests, refactors, search, docs)
* Merge results back via diffs/patches + approvals

The key is you already get a **turn lifecycle** and an **item lifecycle**; Autopilot can schedule work at “item boundaries” cleanly.

---

# The 3 highest-leverage things to do next

1. **Write an Autopilot ↔ Codex adapter** that:

   * starts server
   * does `initialize/initialized`
   * `thread/start` + `turn/start`
   * turns streamed notifications into your internal event model
   * handles approval requests (round-trip)

2. **Map Codex’s event stream into your trace format**

   * Don’t fight it—Codex already emits the right semantic events.
   * Build a lossless mapping and store both raw + normalized.

3. **Define your Autonomy Policy layer above approvals**

   * A policy engine that decides “auto-approve/deny/ask” based on:
     command type, path globs, network, repo status, test scope, etc.

If you do those, you basically get: “Autopilot can drive Codex like a motor,” plus a full observability + replay substrate “for free.”



Short answer: **it runs *alongside* your deployed Rust binary, not *inside* it** — but you *control it like a library*.

Long answer, with the exact mental model you want for Autopilot:

---

## The correct mental model

Think of **Codex app-server** as a **sidecar agent runtime**:

* **Separate process**
* **Same machine / same container**
* **Spoken to over stdio (JSONL)**
* **No network dependency required**
* **Full Codex brain inside**

Your **Autopilot Rust binary is the supervisor / orchestrator**.

```
+----------------------------+
|   Autopilot (Rust)         |
|                            |
|  - job graph               |
|  - autonomy policy         |
|  - HUD / telemetry         |
|  - marketplace logic      |
|  - payments / accounting  |
|                            |
|  spawns + controls         |
+-------------+--------------+
              |
        stdio | JSONL
              |
+-------------v--------------+
|   codex-app-server         |
|                            |
|  - threads / turns         |
|  - tool calls              |
|  - sandboxed exec          |
|  - approvals               |
|  - skills / MCP            |
|  - rollout persistence    |
+----------------------------+
```

This is **exactly** how:

* Claude Code CLI works
* VS Code integrations work
* TerminalBench-style agents should work

---

## Why this is *not* a library dependency

Even though there’s a `codex_app_server` Rust crate, **you do not want to embed it directly** for Autopilot v1:

### ❌ Embedding as a lib would:

* entangle your lifecycle with Codex internals
* make crashes fatal to Autopilot
* complicate upgrades
* break the “multi-backend” story
* make sandboxing harder

### ✅ Running as a subprocess gives you:

* crash isolation
* version pinning
* hot swapping (Codex vs Claude vs Custom)
* a uniform “AgentRuntime” abstraction
* clean kill / restart semantics

This is the *right* architecture for a marketplace + autopilot.

---

## How this looks in Rust (conceptually)

```rust
let mut child = Command::new("codex-app-server")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .env("CODEX_HOME", codex_home)
    .spawn()?;
```

Then:

* write JSONL to stdin
* read JSONL events from stdout
* maintain a request/response map
* forward approvals through your policy engine

From Autopilot’s POV, **Codex is just another worker**.

---

## When *would* you embed it?

Only later, *optionally*, if you want:

* a **single-binary “edge agent”**
* or an **offline appliance mode**
* or deep integration for a special product tier

Even then, you’d probably:

* keep it behind a feature flag
* preserve the JSON-RPC boundary internally
* still treat it like a subprocess conceptually

For now: **sidecar always wins**.

---

## Deployment scenarios

### Local dev / contributor mode

```
autopilot
└── spawns codex-app-server
    └── runs in repo working dir
```

### CI / cloud worker

```
job container
├── autopilot
└── codex-app-server
```

### Marketplace node

```
node supervisor
├── autopilot daemon
├── codex-app-server (N copies)
└── other runtimes (Claude, local models)
```

---

## Key takeaway (the one-liner)

> **Autopilot is the air traffic controller. Codex app-server is one of the planes.**

You don’t link planes into the tower — you talk to them over a protocol.

If you want, next we can:

* define the `AgentRuntime` trait
* sketch the exact approval policy DSL
* or show how to multiplex multiple Codex instances safely



Below is a concrete spec you can hand to an agent to implement. It’s structured so you can drop in **Codex app-server** today, and later swap/stack other runtimes (Claude CLI headless, your own Rust agent, remote workers) without changing Autopilot’s core.

---

# Autopilot ↔ Runtime Adapter Spec (Codex app-server first)

## Goals

1. Autopilot supervises one or more agent runtimes via a **single uniform interface**.
2. Runtimes are **sidecar processes** (subprocesses) by default.
3. Autopilot provides:

   * autonomy policy (approve/deny/ask)
   * observability (HUD events + metrics)
   * persistence (raw + normalized traces)
   * concurrency + cancellation
4. Codex app-server integration:

   * JSONL over stdio
   * streamed notifications
   * approval request/response routing
   * tool events and diffs/plan updates

Non-goals (v1):

* embedding Codex as a Rust library
* cross-machine RPC transport (you can add later)
* perfectly stable schema across Codex versions (we’ll version-pin + feature-detect)

---

# 1) Core Abstractions

## 1.1 IDs and Types

```rust
pub type RuntimeId = String;   // "codex:local:0" etc
pub type ThreadId  = String;   // "thr_123" (runtime-specific)
pub type TurnId    = String;   // optional if runtime exposes
pub type ItemId    = String;   // streaming units / tool calls / assistant msgs
pub type RequestId = u64;      // JSON-RPC id

#[derive(Clone, Debug)]
pub struct RepoSpec {
    pub root: std::path::PathBuf,
    pub default_branch: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TurnInput {
    pub messages: Vec<InputMessage>,
}

#[derive(Clone, Debug)]
pub enum InputMessage {
    Text(String),
    // extend: Images, files, etc.
}
```

## 1.2 AgentRuntime Trait (Autopilot-facing)

This is the **only** thing Autopilot core needs.

```rust
#[async_trait::async_trait]
pub trait AgentRuntime: Send + Sync {
    fn id(&self) -> &RuntimeId;
    fn capabilities(&self) -> &RuntimeCapabilities;

    async fn start(&self) -> anyhow::Result<()>;
    async fn shutdown(&self) -> anyhow::Result<()>;

    async fn open_thread(&self, repo: &RepoSpec) -> anyhow::Result<ThreadHandle>;

    async fn start_turn(
        &self,
        thread: &ThreadHandle,
        input: TurnInput,
        opts: TurnOptions,
    ) -> anyhow::Result<TurnHandle>;

    /// Stream events for a turn (plan updates, deltas, approvals, tool events, etc.)
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<RuntimeEvent>;

    /// Respond to runtime-originated requests (approvals, auth prompts, etc).
    async fn respond(&self, req: RuntimeRequestResponse) -> anyhow::Result<()>;
}

#[derive(Clone, Debug)]
pub struct RuntimeCapabilities {
    pub supports_streaming: bool,
    pub supports_approvals: bool,
    pub supports_diff_updates: bool,
    pub supports_plan_updates: bool,
    pub supports_tool_exec: bool,
    pub supports_git: bool,
    pub supports_skills: bool,
    pub supports_mcp: bool,
}
```

Thread/Turn handles:

```rust
#[derive(Clone, Debug)]
pub struct ThreadHandle {
    pub runtime_id: RuntimeId,
    pub thread_id: ThreadId,
    pub repo: RepoSpec,
}

#[derive(Clone, Debug)]
pub struct TurnHandle {
    pub runtime_id: RuntimeId,
    pub thread_id: ThreadId,
    pub turn_id: Option<TurnId>,
}
```

Turn options:

```rust
#[derive(Clone, Debug, Default)]
pub struct TurnOptions {
    pub allow_network: Option<bool>,
    pub max_steps: Option<u32>,
    pub tags: Vec<String>, // for telemetry grouping
}
```

---

# 2) Unified Event Model (HUD + Telemetry Spine)

Everything you render or measure in Autopilot comes from `RuntimeEvent`.

```rust
#[derive(Clone, Debug)]
pub struct RuntimeEvent {
    pub ts: std::time::SystemTime,
    pub runtime_id: RuntimeId,
    pub thread_id: Option<ThreadId>,
    pub turn_id: Option<TurnId>,
    pub kind: RuntimeEventKind,
    pub raw: Option<serde_json::Value>, // optional raw payload for lossless capture
}

#[derive(Clone, Debug)]
pub enum RuntimeEventKind {
    // lifecycle
    Initialized { server_info: serde_json::Value },
    ThreadStarted { thread_id: ThreadId, cwd: std::path::PathBuf },
    TurnStarted   { turn_id: Option<TurnId> },
    TurnCompleted { status: TurnStatus, summary: Option<String> },
    Error         { message: String, recoverable: bool },

    // assistant output
    AssistantMessageStarted { item_id: ItemId },
    AssistantMessageDelta   { item_id: ItemId, delta: String },
    AssistantMessageCompleted { item_id: ItemId },

    // planning + diffs
    PlanUpdated { markdown: String },
    DiffUpdated { unified_diff: String },

    // tools
    ToolCallStarted { item_id: ItemId, tool: String, input: serde_json::Value },
    ToolCallStdout  { item_id: ItemId, chunk: String },
    ToolCallStderr  { item_id: ItemId, chunk: String },
    ToolCallCompleted { item_id: ItemId, ok: bool, output: serde_json::Value },

    // approvals (critical)
    ApprovalRequested(ApprovalRequest),
    ApprovalResolved { approval_id: String, decision: ApprovalDecision },

    // usage
    TokenUsageUpdated { input_tokens: u64, output_tokens: u64, total_tokens: u64 },

    // context
    ContextCompacted { from_tokens: u64, to_tokens: u64 },
}

#[derive(Clone, Debug)]
pub enum TurnStatus { Success, Failed, Cancelled }
```

Approval request shape (runtime-agnostic):

```rust
#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    pub approval_id: String, // runtime request id or generated id
    pub kind: ApprovalKind,
    pub title: String,
    pub risk: ApprovalRisk,
    pub details: serde_json::Value, // runtime-specific + normalized fields
    pub suggested_action: Option<String>, // e.g., "Run `cargo test`"
}

#[derive(Clone, Debug)]
pub enum ApprovalKind {
    CommandExec,
    ApplyPatch,
    NetworkAccess,
    FileWrite,
    AuthLogin,
    Custom(String),
}

#[derive(Clone, Debug)]
pub enum ApprovalRisk { Low, Medium, High }

#[derive(Clone, Debug)]
pub enum ApprovalDecision { Approve, Deny, AskHuman }
```

---

# 3) Request/Response Model (Approvals + Server-Initiated Requests)

Runtimes may initiate requests (Codex does). We normalize.

```rust
#[derive(Clone, Debug)]
pub enum RuntimeRequestResponse {
    Approval { approval_id: String, decision: ApprovalDecision, note: Option<String> },
    // extend: Auth flows, MCP OAuth, etc.
}
```

---

# 4) Autonomy Policy Layer (the heart)

## 4.1 Policy Engine Contract

Autopilot decides for each approval request.

```rust
pub trait AutonomyPolicy: Send + Sync {
    fn decide(&self, ctx: &PolicyContext, req: &ApprovalRequest) -> PolicyOutcome;
}

#[derive(Clone, Debug)]
pub struct PolicyContext {
    pub repo_root: std::path::PathBuf,
    pub branch: Option<String>,
    pub actor: String,            // "autopilot", "contributor:xyz"
    pub mode: AutonomyMode,       // ReadOnly / Propose / Auto
    pub allow_network: bool,
    pub env: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub enum AutonomyMode { ReadOnly, Propose, Auto }

#[derive(Clone, Debug)]
pub struct PolicyOutcome {
    pub decision: ApprovalDecision,
    pub reason: String,
    pub constraints: Vec<PolicyConstraint>, // optional refinements
}

#[derive(Clone, Debug)]
pub enum PolicyConstraint {
    RedactSecrets,
    LimitCommandTime { seconds: u64 },
    LimitPaths { allow: Vec<String>, deny: Vec<String> },
}
```

## 4.2 A Small Policy DSL (declarative, JSON/YAML)

File: `autopilot.policy.yaml`

```yaml
version: 1
defaults:
  mode: propose
  allow_network: false

modes:
  read_only:
    rules:
      - when: { kind: CommandExec }
        then: { decision: deny, reason: "Read-only mode" }
      - when: { kind: ApplyPatch }
        then: { decision: deny, reason: "Read-only mode" }

  propose:
    rules:
      - when: { kind: ApplyPatch }
        then: { decision: ask_human, reason: "Patch requires review" }
      - when:
          kind: CommandExec
          command:
            allow:
              - "^git (status|diff|show|log)"
              - "^rg "
              - "^fd "
              - "^cargo (check|test)( .*)?$"
            deny:
              - "rm -rf"
              - "curl "
              - "wget "
        then: { decision: approve, reason: "Safe dev commands" }
      - when: { kind: NetworkAccess }
        then: { decision: deny, reason: "Network disabled by default" }

  auto:
    rules:
      - when: { kind: ApplyPatch, files_changed: { max: 50 } }
        then: { decision: approve, reason: "Auto mode patch within limits" }
      - when:
          kind: CommandExec
          command:
            allow:
              - "^cargo test( .*)?$"
              - "^pnpm test( .*)?$"
              - "^npm test( .*)?$"
              - "^pytest( .*)?$"
        then: { decision: approve, reason: "Allow tests" }
      - when: { kind: NetworkAccess }
        then: { decision: ask_human, reason: "Network is sensitive" }
```

**Normalization requirement:** for `CommandExec` approvals, the adapter must populate `details.command` as a string (when available).

## 4.3 Policy Evaluation Order

1. Find mode (`read_only`, `propose`, `auto`)
2. Evaluate rules top-to-bottom
3. First match wins
4. If no match:

   * `ReadOnly`: deny
   * `Propose`: ask_human
   * `Auto`: ask_human (safe fallback)

---

# 5) Codex Runtime Adapter (Concrete)

## 5.1 Process Model

Codex is spawned as a subprocess:

* executable: `codex-app-server` or `codex app-server`
* stdin: write JSONL
* stdout: read JSONL (notifications + responses interleaved)
* stderr: capture for debug logs (don’t treat as protocol)

Env:

* `CODEX_HOME` per runtime instance (important for session/rollout separation)
* possibly `RUST_LOG` / telemetry vars if you want

Working directory:

* thread/start uses `cwd` pointing to repo root

## 5.2 Handshake

Autopilot must perform:

1. `initialize` request with `clientInfo`
2. wait for response
3. `initialized` notification

If initialize fails, mark runtime unhealthy.

## 5.3 Request Routing

Because JSON-RPC-lite uses `id` for requests, the adapter maintains:

* `next_request_id: AtomicU64`
* `pending: HashMap<RequestId, oneshot::Sender<Response>>`

Outgoing:

* serialize JSON object
* write line + flush

Incoming:

* parse each line as JSON
* if contains `"id"` and `"result"`/`"error"`: resolve pending
* else: treat as notification and emit `RuntimeEvent`

## 5.4 Codex→Autopilot Event Mapping

Mapping rules (examples):

* `thread/started` → `RuntimeEventKind::ThreadStarted`
* `turn/started` → `TurnStarted`
* `item/started` where item is assistant message → `AssistantMessageStarted`
* `item/*/delta` → `AssistantMessageDelta` (or tool stdout/stderr depending on item type)
* `turn/plan/updated` → `PlanUpdated`
* `turn/diff/updated` → `DiffUpdated`
* `thread/tokenUsage/updated` → `TokenUsageUpdated`
* `contextCompacted` → `ContextCompacted`
* approval requests:

  * `ExecApprovalRequestEvent` → `ApprovalRequested(kind=CommandExec, details.command=...)`
  * `ApplyPatchApprovalRequestEvent` → `ApprovalRequested(kind=ApplyPatch, details.diff=...)`

**Lossless capture:** store the original notification payload in `RuntimeEvent.raw`.

## 5.5 Responding to Approvals

Codex uses server-initiated requests + response routing. The adapter must:

* assign an `approval_id`

  * ideally use Codex’s request identifier
  * else derive a stable ID from request payload hash + turn context
* when Autopilot sends `RuntimeRequestResponse::Approval`, map to the Codex response method for that approval type.

---

# 6) Multiplexing Multiple Codex Instances (Safe + Fast)

## 6.1 Use Case

* One Autopilot job may run multiple concurrent turns (e.g., “planner” + “executor” + “tester”)
* Or you may run multiple jobs concurrently on the same machine

## 6.2 RuntimePool

Autopilot manages a pool of runtimes:

```rust
pub struct RuntimePool {
    // keyed by runtime id
    runtimes: dashmap::DashMap<RuntimeId, std::sync::Arc<dyn AgentRuntime>>,
}

impl RuntimePool {
    pub fn register(&self, rt: std::sync::Arc<dyn AgentRuntime>) { /* ... */ }
    pub fn get(&self, id: &str) -> Option<std::sync::Arc<dyn AgentRuntime>> { /* ... */ }

    pub fn select(&self, selector: RuntimeSelector) -> anyhow::Result<std::sync::Arc<dyn AgentRuntime>> { /* ... */ }
}

pub enum RuntimeSelector {
    Preferred(Vec<String>), // ids or “types”
    CapabilityRequired(RuntimeCapabilitiesMask),
    Any,
}
```

## 6.3 Per-Job Isolation

**Strongly recommended:** each runtime instance gets:

* its own `CODEX_HOME` under `~/.openagents/runtimes/<job>/<rt>/`
* its own process
* its own session directory

This prevents cross-job rollout collisions and simplifies cleanup.

## 6.4 Concurrency Limits

Implement:

* max runtimes per host
* max turns per runtime
* queue + backpressure

Example config:

```yaml
runtimes:
  codex:
    max_instances: 4
    max_turns_per_instance: 1
```

Codex is easiest if you treat it as **one active turn at a time** per process until proven otherwise.

---

# 7) Persistence: Raw + Normalized Traces

## 7.1 Raw Capture

Write every inbound/outbound JSON line to:

* `runs/<run_id>/runtimes/<runtime_id>/wire.jsonl`

This is your ground truth.

## 7.2 Normalized Event Log

Write every `RuntimeEvent` to:

* `runs/<run_id>/events.jsonl`

This is what powers:

* HUD replay
* analytics
* regression tests

## 7.3 Linking to Codex Rollouts

Codex already writes rollouts under `CODEX_HOME/sessions`.

Autopilot should:

* record the `CODEX_HOME` path used
* copy or archive rollouts into your run folder on completion (optional but nice)
* store pointers in a `run_manifest.json`

---

# 8) Observability: Metrics You Should Emit (minimal set)

Emit counters/timers per turn:

* `turn.duration_ms`
* `approval.count`, `approval.wait_ms`
* `tool.calls`, `tool.duration_ms`
* `tokens.in`, `tokens.out`, `tokens.total`
* `diff.bytes`, `diff.files_changed` (if available)
* `errors.count`

These come directly from `RuntimeEventKind`.

---

# 9) Security & Sandbox Policy

Autopilot must never “trust” the runtime:

* Approvals are a hard boundary.
* Policy engine is authoritative.
* If `mode != Auto`, default to AskHuman.

Additionally:

* enforce path allow/deny at Autopilot layer when possible
* enforce command allow/deny patterns
* redact secrets from logs by default (`PolicyConstraint::RedactSecrets`)

---

# 10) Implementation Plan (what to build first)

## Milestone A: CodexAdapter MVP (1 runtime, 1 turn)

* spawn process
* initialize handshake
* thread/start
* turn/start
* stream events to broadcast channel
* raw wire capture
* normalized events capture

## Milestone B: Approval Loop + Policy

* map approval events → `ApprovalRequested`
* implement YAML policy parser + matcher
* auto-respond approve/deny/ask_human
* if ask_human: emit event and block turn until user responds (or timeout)

## Milestone C: Pool + Multiplex

* runtime pool
* job-runner selects runtime by capability
* per-job CODEX_HOME isolation
* concurrency limits

## Milestone D: HUD Integration

* consume `events.jsonl` live
* panes:

  * plan (PlanUpdated)
  * diff (DiffUpdated)
  * stream (AssistantMessageDelta)
  * tools (ToolCall*)
  * approvals (ApprovalRequested)

---

# 11) Files & Modules (suggested Rust layout)

```
autopilot/
  src/
    runtime/
      mod.rs                // AgentRuntime trait + types
      events.rs             // RuntimeEvent model
      pool.rs               // RuntimePool
      policy/
        mod.rs              // AutonomyPolicy trait
        dsl.rs              // YAML structs
        matcher.rs          // rule matching
      adapters/
        mod.rs
        codex/
          mod.rs            // CodexRuntime struct
          process.rs        // spawn/stdio
          protocol.rs       // request/response structs (serde)
          router.rs         // pending map, id gen
          mapper.rs         // codex notifications -> RuntimeEventKind
          approvals.rs      // approval mapping + response mapping
          wirelog.rs        // raw jsonl capture
    runner/
      job.rs                // Autopilot job orchestration
      turn_loop.rs          // manage approval waits, cancellation
    storage/
      run_manifest.rs
      event_log.rs
```

---

# 12) “Ask human” UX Contract (so engineering doesn’t stall)

If policy returns `AskHuman`:

* emit `ApprovalRequested`
* put runtime in “waiting” state
* Autopilot must expose:

  * CLI prompt mode: `autopilot approvals list` + `autopilot approvals decide <id> approve|deny`
  * or HTTP callback for UI

Once decision arrives:

* call `runtime.respond(Approval { ... })`
* emit `ApprovalResolved`

Also support timeout:

* if no answer within N minutes: deny by default (configurable)

---

# 13) Edge Cases You Must Handle

* Process dies mid-turn → emit `Error(recoverable=true)` and mark turn failed; pool may restart runtime
* Malformed JSON line → treat as stderr-like error; continue reading
* Out-of-order events → rely on item_id + turn context; don’t assume strict ordering
* Missing capabilities (e.g., no diff updates) → HUD pane stays blank; don’t break

---

If you want, I can also provide:

* the exact YAML matcher semantics (regex engine, AND/OR, globbing)
* a concrete Codex notification shape table (method name → normalized event)
* a minimal “approval CLI” spec (commands + JSON storage format)
