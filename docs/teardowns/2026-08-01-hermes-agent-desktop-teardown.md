# Hermes Agent and Desktop Teardown — 2026-08-01

Status: source-complete, commit-pinned teardown<br>
Primary repository: [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent)<br>
Audited revision: [87bc710609f8b89b6e6b4aa418dde8ee30ec6873](https://github.com/NousResearch/hermes-agent/tree/87bc710609f8b89b6e6b4aa418dde8ee30ec6873)<br>
Repository version: <code>0.19.1</code><br>
Desktop package version: <code>0.17.0</code><br>
License: MIT

## Executive verdict

Hermes is one of the most complete open personal-agent systems in this study set. It is not just a terminal loop with a thin chat window. One Python agent core serves a classic CLI, an Ink terminal UI, messaging gateways, ACP clients, a browser dashboard, cron workers, delegated agents, and a purpose-built Electron/React desktop. The desktop is an IDE-like work surface: streaming chat, multiple sessions and profiles, project files, terminals, Git review, worktrees, artifacts, persistent panes, voice, memory and skill management, scheduling, and an extension system.

Its strongest architectural move is also the one most relevant to OpenAgents and Omega: the desktop does not contain the agent. Electron owns machine capabilities and supervises authenticated, profile-scoped <code>hermes serve</code> processes; the React renderer speaks a shared JSON-RPC/WebSocket protocol to the Python runtime. That separation allows the same session and agent semantics to survive across interfaces while keeping raw filesystem, process, PTY, keychain, and window authority out of the renderer.

Hermes also exposes the cost of pursuing the whole personal-agent stack at once. The system has several large coordination files, extensive surface-dependent approval behavior, mutable instructions and memories, trusted renderer plugins, a broad Electron capability bridge, and a source-oriented updater that can rebuild a moving checkout. The test investment is significant, but the product's rate of change and authority breadth make trust reasoning harder than the UI suggests.

For OpenAgents:

- **Adopt** the separation between an agent service and desktop shell, durable-versus-runtime session identity, byte-stable replay, explicit streaming event vocabulary, project/session composition, and performance-specific regression harnesses.
- **Adapt** the profile backend pool, approval policy, memory/skill review, gateway contract, and persistent terminal/Git workflows behind narrower capabilities and clearer lifecycle contracts.
- **Avoid** adopting Hermes's Electron/Python shell wholesale, treating local execution as a sandbox, loading agent-written code into a fully trusted renderer realm, or coupling application updates to a mutable source checkout.
- **Integrate first through ACP.** Hermes already presents a reduced, permission-aware ACP surface. Evaluate <code>hermes serve</code> only when OpenAgents needs Hermes-specific profile, scheduling, memory, or desktop-adjacent features that ACP does not expose.

## Central finding

Hermes's differentiator is not any single tool. It is the continuity of one stateful agent across terminal, messaging, scheduling, ACP, and a serious desktop workbench.

That continuity is earned through disciplined session persistence and a shared event protocol, but it also concentrates authority and complexity in the Python runtime and Electron main process. The transferable design is the boundary and state model—not the full implementation stack.

## Evidence model and scope

The teardown follows the evidence labels used elsewhere in this directory:

- **[source]** Directly observed in the pinned repository.
- **[history]** Derived from Git history at or before the audited revision.
- **[test]** Encoded by repository tests or CI. Tests were inspected, not executed against Hermes for this review.
- **[public]** Product behavior described by repository documentation or the public project site.
- **[inferred]** Architectural or product conclusion drawn from the evidence.
- **[limitation]** A boundary on what this review establishes.

This is a source and history audit. It does not claim that release artifacts, cloud services, provider accounts, messaging networks, signing infrastructure, or every operating-system path were exercised.

### Snapshot

| Field | Audited value |
|---|---|
| Revision | <code>87bc710609f8b89b6e6b4aa418dde8ee30ec6873</code> |
| Commit subject | <code>fix(agent): scope parallel batches from V4A patch headers</code> |
| Commit date | 2026-07-22 |
| Python package version | <code>0.19.1</code> |
| Desktop package version | <code>0.17.0</code> |
| Repository files | 8,204 |
| Approximate repository lines | 2.65 million |
| Python agent/runtime files and lines | 892 files; approximately 742,000 lines |
| Desktop TypeScript/TSX files and lines | 1,335 files; approximately 288,000 lines |
| Python test files and lines | 2,619 files; approximately 603,000 lines |
| Desktop test files | 468 |
| Desktop end-to-end specs | 18 |

These measurements use tracked source files at the pinned revision and intentionally separate tests from the runtime groups. They describe scale, not code quality.

### Prior OpenAgents coverage

Before this document, OpenAgents had **not** performed a direct source teardown of the Hermes desktop application.

- [The terminal-agent study](../research/terminal-agents/hermes-agent.md) inspected Hermes's registry, executor, approval, and tool-selection layer at revision <code>10043c6d0cd942487f7ef94231e22d91e1734a20</code>. It did not inspect the desktop architecture.
- [The Omega Zed primary-surface plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md) analyzed Hermes as an ACP integration candidate. That work answered how Omega should attach to Hermes, not how Hermes's own desktop works.
- [An Agentic Society note](../agenticsociety/2026-07-03.md) mentioned desktop profiles and souls as observed product behavior, but was not a source audit.

**[history]** The desktop was already present at the old terminal-agent study revision: that snapshot contains 711 tracked files under <code>apps/desktop</code>. The missing desktop analysis was a scope omission, not a timing issue. The desktop first landed in [Add Hermes desktop app](https://github.com/NousResearch/hermes-agent/commit/51c68d4ab1a9e3c62fb1048fccb84144c409f0e7), and more than 2,100 later commits touched its directory before this audit.

## System map

~~~mermaid
flowchart LR
    User["User"]

    subgraph Surfaces["Interaction surfaces"]
        CLI["Classic CLI"]
        TUI["Ink TUI"]
        MSG["Messaging gateways"]
        ACP["ACP adapter"]
        WEB["Browser dashboard"]
        DESK["Electron + React desktop"]
        CRON["Cron / board workers"]
    end

    subgraph DesktopBoundary["Desktop machine boundary"]
        RENDER["Sandboxed renderer"]
        MAIN["Electron main process"]
        POOL["Profile backend pool"]
        RENDER -->|"contextBridge IPC"| MAIN
        RENDER -->|"authenticated JSON-RPC + WS"| POOL
        MAIN -->|"spawn / supervise"| POOL
    end

    subgraph Runtime["Python Hermes runtime"]
        GATE["tui_gateway / hermes serve"]
        LOOP["Conversation loop"]
        PROMPT["Prompt + context engine"]
        TOOLS["Registry + executor + approvals"]
        STATE["SQLite sessions + usage + delivery"]
        LEARN["Memory + skills + review"]
        ORCH["Delegation + MoA + scheduling"]
        GATE --> LOOP
        LOOP --> PROMPT
        LOOP --> TOOLS
        LOOP <--> STATE
        LOOP <--> LEARN
        LOOP <--> ORCH
    end

    subgraph External["External authority"]
        MODELS["Model providers"]
        HOST["Host / containers / SSH"]
        NETWORKS["Messaging + MCP + web"]
    end

    User --> Surfaces
    CLI --> LOOP
    TUI --> GATE
    MSG --> LOOP
    ACP --> LOOP
    WEB --> GATE
    DESK --> RENDER
    CRON --> LOOP
    POOL --> GATE
    LOOP --> MODELS
    TOOLS --> HOST
    TOOLS --> NETWORKS
~~~

**[inferred]** Hermes has two important seams:

1. The shared agent seam around <code>AIAgent</code>, session persistence, and tool execution.
2. The interface seam around <code>tui_gateway</code>, JSON-RPC, and streamed events.

ACP attaches at the first seam through a deliberately reduced toolset. Hermes Desktop attaches at the second and supplements it with privileged Electron IPC for local workstation features.

## Product surface

### One agent, many interfaces

**[source]** The repository presents Hermes as a self-improving personal agent with:

- persistent sessions, full-text session search, branching, handoff, and compression;
- locally managed skills, memories, profiles, and project instructions;
- terminal tools with local, container, remote, and hosted execution backends;
- delegated subagents, mixture-of-agents synthesis, cron, and Kanban workers;
- provider plugins spanning several API families;
- messaging adapters and a gateway;
- classic CLI, TUI, web dashboard, ACP, and desktop interfaces.

The interfaces do not all expose identical behavior. They converge on the same runtime and persisted sessions, then add surface-specific presentation, permissions, prompts, and lifecycle.

**[inferred]** This is a more consequential product choice than “available everywhere.” A conversation begun in a terminal can be resumed by the desktop because session state, system prompt, model accounting, tool messages, and compression lineage live below either UI.

### Desktop product position

**[public]** Hermes's desktop documentation describes the application as a separate, purpose-built UI—not an Electron wrapper around the TUI or web dashboard.

The desktop combines:

- streaming chat, reasoning and tool activity;
- attachments, prompt queue editing, steering, interrupt, find, timeline, context and cost status;
- session tabs, split panes, multiple windows, and per-session unread state;
- repositories, projects, file trees, previews, CodeMirror editors, and artifacts;
- persistent terminals and agent-process terminals;
- Git status, diff/review, stage, revert, commit, push, branches, worktrees, and pull-request creation;
- profile, model, skill, plugin, agent, cron, messaging, webhook, and memory management;
- voice input, wake behavior, global quick entry, notifications, and a pet overlay;
- a Learning Journey or “Starmap” that visualizes skills and memories.

**[source]** The desktop's design principles make chat primary, pages durable, overlays short-lived, panes contextual, and projects the owner of working directory. It explicitly avoids background focus stealing and keeps expensive panes mounted when possible.

**[inferred]** Hermes Desktop competes less with a chat client than with a lightweight agent IDE. Its workbench is built around an ongoing agent relationship rather than a repository-first editor.

## Agent runtime

### Entry point and extraction pattern

**[source]** <code>run_agent.py</code> still defines the public <code>AIAgent</code> class and is approximately 7,500 lines. Construction forwards a broad configuration surface to <code>agent.agent_init.init_agent</code>; a normal conversation allows up to 90 tool iterations by default. The public module remains a compatibility façade while execution responsibilities have moved into:

- <code>agent/conversation_loop.py</code>;
- <code>agent/prompt_builder.py</code> and <code>agent/system_prompt.py</code>;
- <code>agent/tool_executor.py</code> and dispatch helpers;
- context compression, checkpoints, relay, usage, session coordination, and hooks modules.

**[source]** <code>AIAgent.run_conversation</code> establishes accounting, relay, context, and session coordination, then calls the extracted conversation loop.

**[inferred]** Hermes is mid-refactor rather than cleanly modular. The extraction is real, and tests target the new boundaries, but compatibility and feature velocity keep large façade and coordinator files alive.

### Turn lifecycle

At a high level, one turn proceeds as follows:

1. **Prepare the turn.** Guard protocol output, sanitize input, restore or construct the prompt, add task nudges, run plugin pre-model hooks, fetch external memory, and perform preflight compression.
2. **Build a provider-safe request.** Copy clean persisted messages, repair role alternation and tool calls, strip display-only fields, restore exact API sidecars, apply provider-specific reasoning and tool transformations, and calculate cache markers.
3. **Call the model.** Prefer streaming when it helps health detection and live consumers; use a non-stream path for ACP, mixture-of-agents, or compatible mocks; route through relay middleware and retry/fallback logic.
4. **Normalize the response.** Repair Unicode and tool calls, handle refusals and content filters, record usage and cost, and persist the assistant turn.
5. **Execute tools.** Validate and deduplicate calls, choose concurrent or sequential scheduling, apply approvals and guardrails, stream progress, and append results in deterministic model-call order.
6. **Continue or settle.** Check redirect, interrupt, budget, compression, checkpoint, and final-response recovery conditions.

**[source]** A shared iteration budget follows a turn through retries and nested execution, rather than treating each helper loop as an independent allowance.

**[inferred]** The loop is designed for recovery under unreliable provider, context, network, and tool conditions. Its complexity comes from preserving the illusion of one continuous conversation while transports and schemas differ underneath.

## Prompt, context, and replay

### Stable system prompt

**[source]** Hermes separates relatively static system material from volatile turn context. Prompt construction can incorporate:

- a profile's <code>SOUL.md</code>;
- repository or directory instructions such as <code>HERMES.md</code>, <code>AGENTS.md</code>, <code>CLAUDE.md</code>, and <code>.cursorrules</code>;
- selected skill summaries;
- tool and environment hints;
- runtime, platform, date, working-directory, and project information.

Instruction files are scanned, bounded, and truncated before use.

**[source]** The system prompt is built once for a session, persisted in SQLite, and restored verbatim on resume. Provider fallback can re-decorate cache-control metadata without rewriting the underlying prompt.

### Byte-stable user replay

**[source]** Messages may retain an <code>api_content</code> sidecar containing the exact content sent to the provider. This matters when memory, plugin hooks, or system notices augment what the user typed. Replayed turns use the provider-facing bytes rather than reconstructing semantically equivalent content.

**[inferred]** This is one of Hermes's best ideas. Prompt caching depends on stable prefixes, and session recovery depends on replay fidelity. Keeping display content and provider content distinct avoids making either concern corrupt the other.

### Compression and lineage

**[source]** Hermes checks context before model calls, handles provider context errors, and can compress a session into a new runtime continuation. Compression uses locks and atomic publication so two workers do not publish competing tips.

The data model distinguishes:

- a durable lineage root that users navigate and pin;
- a live runtime or compressed tip used for the next model call;
- parent-child relationships for branches, handoffs, subagents, and compression.

Root sessions and user-created branches are listable. Internal compression and subagent tips can remain hidden from normal navigation.

**[inferred]** Compression is modeled as lineage, not in-place mutation. That makes rollback, branch display, and cross-surface resume tractable.

## Provider architecture and reliability

### Plugin-driven providers

**[source]** The audited tree contains 33 model-provider plugins. Transports include:

- OpenAI-compatible Chat Completions;
- Anthropic Messages;
- AWS Bedrock Converse;
- Codex Responses and Codex app-server paths;
- native Gemini translation;
- GitHub Copilot over ACP;
- virtual mixture-of-agents aggregation.

Plugins are discovered lazily, and model/provider configuration can be profile-scoped.

### Fallback and response repair

**[source]** Provider failover is not just retrying another URL. Hermes has explicit adaptations for:

- reasoning and hidden-thought formats;
- tool schema and tool-call differences;
- role alternation requirements;
- prefill behavior;
- cache-control placement;
- streamed versus non-streamed responses;
- credential pools and exhausted credentials;
- refusals, content filters, empty responses, malformed Unicode, and context overflow.

**[inferred]** Cross-provider fallback is a state-translation problem. Hermes acknowledges this in code. OpenAgents should copy that framing if it ever offers transparent provider switching.

### Usage accounting

**[source]** Usage is normalized into input, output, cache-read, cache-write, and reasoning tokens, with estimated or actual cost. It is persisted per session and per model/task.

**[inferred]** Cost is treated as session state rather than UI telemetry. That lets terminal, desktop, background review, cron, and delegated work report through one accounting model.

## Tool system

The earlier [Hermes terminal-agent study](../research/terminal-agents/hermes-agent.md) remains the detailed historical analysis of registry discovery, schemas, and execution at its older pin. The findings below describe how that layer fits into the present full system.

### Registry and discovery

**[source]** <code>tools/registry.py</code> discovers tool registrations from source, caches discovery on disk, and records:

- name, schema, handler, and handler-defining module;
- availability checks and required environment;
- sync or async execution;
- metadata, output caps, and dynamic schema behavior;
- a generation counter for invalidation.

Availability checks use a TTL and tolerate transient failures for a grace period. Plugin overrides require authorization tied to the module that actually defines the handler.

**[source]** <code>model_tools.py</code> composes requested toolsets, excludes disabled tools, sanitizes schemas, performs progressive “Tool Search” disclosure, and dispatches calls.

### Execution scheduling

**[source]** The executor supports fully sequential, fully concurrent, and segmented batches. A batch is parallel only after an explicit concurrency authorization gate. Dispatch helpers classify read-only operations and infer path conflicts; V4A patch headers contribute write scopes.

Tool results are appended to conversation state in original model-call order even when work completes out of order.

**[inferred]** Deterministic result ordering is more important than maximum concurrency. It stabilizes replay and provider expectations while still allowing safe latency wins.

### Toolsets as policy

**[source]** <code>toolsets.py</code> defines a core Hermes set and named presets. The ACP adapter uses a reduced <code>hermes-acp</code> set rather than inheriting every host capability.

**[inferred]** Named toolsets are an architectural policy layer, not only a convenience. OpenAgents should preserve this property when exposing third-party agents in different surfaces.

## Approvals, guardrails, and execution backends

### Approval modes

**[source]** The default approval mode is <code>smart</code>, with a five-minute timeout, a denial circuit breaker, user deny patterns, and special behavior for cron and slash commands. Modes are profile-scoped:

- <code>manual</code> asks the user for dangerous actions;
- <code>smart</code> can route warnings to an auxiliary model guardian for approve, deny, or escalate;
- <code>off</code>, often presented as “yolo,” bypasses ordinary dangerous-operation prompts.

A user can override a smart denial once. The override is intentionally not converted into a permanent allow rule.

### Unconditional protections

**[source]** Some checks run before approval-mode bypass:

- destructive recursive deletion of root, home, or protected system targets;
- filesystem formatting and raw-device writes;
- fork bombs, broad process killing, and shutdown or reboot patterns;
- unsafe sudo password delivery;
- user-configured deny patterns.

Tirith content scanning can add prompt-injection or suspicious-content checks and can be configured fail-closed.

### Backend reality

**[source]** Terminal execution can target local processes, Docker, Singularity, Modal, Daytona, Vercel, and SSH. Container execution may skip host guardrails only when it has no host bind mount; host-connected containers remain guarded.

The <code>execute_code</code> path is a child-process sandbox with:

- a small RPC tool intersection;
- secret environment scrubbing;
- output redaction and limits;
- controlled communication with the parent.

**[limitation]** Local execution is a normal, supported backend and is often the default. Hermes has sandbox options; this does not mean all Hermes tools run in a sandbox.

### Surface-dependent semantics

**[source]** Approval behavior varies for interactive CLI, gateway/desktop, background work, delegated agents, and cron. A generic non-interactive call outside a known interactive surface can permit operations that a desktop session would ask about; cron separately defaults toward denial.

**[inferred]** Hermes has good individual safeguards but a difficult global question: “Would this operation ask?” depends on mode, surface, task origin, backend, tool, path, and callback availability. OpenAgents should make execution provenance and effective policy observable at the operation itself.

## Persistence and session state

### SQLite as the canonical store

**[source]** <code>state.db</code> is canonical; JSON snapshots are disabled by default. The audited schema is version 23 and includes:

- sessions;
- messages;
- per-model session usage;
- state metadata;
- gateway routing;
- compression locks;
- asynchronous delegation delivery.

Session records include source surface, platform/thread identity, profile, model configuration, persisted system prompt, lineage, message and token counts, cost, working directory, Git context, handoff state, compression health, archive, and pin state.

Message records include role, content, tool calls and results, reasoning, provider replay items, observation/activity/compaction state, exact API content, and display metadata.

### Search and durability

**[source]** Hermes maintains FTS5 indexes with normal tokenization and trigram/CJK paths. The indexes are external-content structures with repair strategies. SQLite uses WAL where safe and contains fallbacks for filesystems or builds that cannot support expected locking behavior. The code avoids silently downgrading a live database.

**[inferred]** The persistence layer is not a transcript cache. It is the coordination plane across interfaces, compression, delegated work, accounting, and recovery.

### Durable and non-durable edges

**[source]** Async delegated completions have durable delivery records. Active child processes, live sockets, and portions of the in-process task registry remain process-local.

**[limitation]** A durable record does not make every running operation crash-resumable. Hermes can recover and deliver completed work more reliably than it can resurrect arbitrary mid-execution tool state.

## Profiles

**[source]** Profile selection occurs before most Python imports. <code>_apply_profile_override</code> sets <code>HERMES_HOME</code>, and a profile receives isolated configuration, environment, skills, memories, sessions, and gateway state.

**[source]** Desktop runs separate child backends for profiles rather than switching one long-lived Python process's global home repeatedly.

**[inferred]** This is a practical containment response to process-global configuration. It improves isolation and concurrency, though it also pushes lifecycle, resource, and version management into the desktop supervisor.

## Memory, skills, and self-improvement

### Built-in memory

**[source]** Profile memory is represented by:

- <code>memories/MEMORY.md</code> for environment, projects, and durable facts;
- <code>USER.md</code> for preferences and persona.

The files use structured delimiters. Memory features are opt-in, and the normal agent can receive periodic nudges to consider remembering useful facts.

### External memory providers

**[source]** Hermes supports at most one external memory provider alongside built-in memory. The audited provider set includes ByteRover, Hindsight, Holographic, Honcho, Mem0, OpenViking, RetainDB, and Supermemory.

The manager can:

- prefetch relevant context before a turn;
- fence external context in the model request;
- synchronize the completed turn afterward;
- enforce prefetch timeouts and drain background durability work;
- scrub injected memory context from streamed UI output.

**[inferred]** The stream scrubber recognizes a subtle UX and security issue: model-visible memory should not automatically become user-visible assistant speech.

### Skill sources

**[source]** Skills can be bundled, hub-installed, optional, externally supplied, or created by the agent. Selected skill metadata appears in the prompt; tools can list, inspect, install, edit, and remove skills subject to ownership and policy.

### Background review

**[source]** After a turn, a background reviewer can fork a restricted <code>AIAgent</code> with only memory and skill tools. It disables normal persistence and compression, skips external-memory injection, and can:

- reuse a warm prompt on the same model;
- receive a compact digest when routed to an auxiliary model;
- update memory or produce a reusable skill.

The reviewer is intentionally assertive: its prompt says that most sessions should yield at least one skill update. Bundled, hub, pinned, and user-owned skills receive stronger protection, and provenance is recorded.

Skill writes do not require approval by default.

### Curator

**[source]** The curator is enabled by default and runs on an idle, roughly weekly cadence. Default policy marks skills stale after 30 days and archives after 90 days, retains five backups, can prune built-ins, and leaves LLM consolidation disabled unless configured.

Archive is recoverable. When LLM consolidation is enabled, an auxiliary agent can merge related material into umbrella skills.

### Self-improvement assessment

**[inferred]** Hermes implements “self-improving” as persistent instruction and memory mutation, not model-weight training. This is useful and inspectable, but it creates a durable feedback loop:

~~~mermaid
flowchart LR
    TURN["Conversation + tools"] --> REVIEW["Background reviewer"]
    REVIEW --> MEMORY["Profile memory"]
    REVIEW --> SKILLS["Agent-created skills"]
    MEMORY --> PROMPT["Future prompt"]
    SKILLS --> PROMPT
    PROMPT --> TURN
    CURATOR["Scheduled curator"] --> SKILLS
~~~

The loop deserves stronger user-facing governance than ordinary chat history. OpenAgents should require clear provenance, diffs, rollback, scope, and an explicit policy for automatic versus proposed writes.

## Delegation, mixture of agents, cron, and Kanban

### Delegated agents

**[source]** <code>delegate_task</code> creates isolated child <code>AIAgent</code> instances with:

- maximum concurrency and nesting depth;
- leaf or orchestrator roles;
- restricted or inherited toolsets;
- configurable MCP inheritance;
- inherited, auto-denied, or auto-approved permission behavior;
- foreground or background completion;
- parent-facing summaries.

**[inferred]** This is process-level reuse of the full agent runtime, not a special prompt macro. It brings provider, session, tool, and approval complexity into every child.

### Mixture of agents

**[source]** The MoA path fans a prompt to reference models and sends their outputs to an aggregator, retaining usage traces for the subcalls.

**[inferred]** Hermes treats multi-model synthesis as a virtual provider. That is a cleaner seam than embedding aggregation branches throughout the main loop.

### Cron

**[source]** Cron persists jobs and supports agent jobs, scripts, skill use, and delivery destinations. Prompts receive suspicious-content and invisible-Unicode defenses. Dangerous commands default to denial in unattended execution.

### Kanban workers

**[source]** The Kanban subsystem persists boards, tasks, comments, attachments, and links in SQLite. A dispatcher starts profile-scoped workers and can use worktrees. Worker ownership is enforced; delegated child agents cannot directly mutate board state.

**[inferred]** Hermes separates plan-of-record mutation from arbitrary child execution. That is a strong pattern for any durable multi-agent workflow.

## Interface and gateway architecture

### Shared gateway

**[source]** <code>tui_gateway</code> is the main interactive service contract. It exposes JSON-RPC methods and streamed events for:

- session create, list, resume, history, branch, compress, undo, save, steer, redirect, and interrupt;
- prompt submission, background prompts, and attachments;
- approvals, clarification, sudo, and secret requests;
- projects, repositories, configuration, profiles, skills, plugins, and tools;
- delegated agents, cron, memory/learning, billing, voice, wake, process control, and rollback.

The server has been split into handler modules, but <code>tui_gateway/server.py</code> is still approximately 13,700 lines.

**[source]** Host supervision categorizes methods into turn-path, idle-gated, and concurrent operations so long-running agent work does not serialize every request.

### TUI and dashboard

**[source]** The Ink TUI speaks JSON-RPC to the Python gateway over standard I/O. The dashboard can embed a TUI process in a browser-oriented shell. They share the runtime contract, but the dashboard is not the implementation of Hermes Desktop.

### Messaging

**[public]** Repository documentation lists adapters for Telegram, Discord, Slack, WhatsApp, Matrix, Mattermost, DingTalk, Feishu, LINE, Microsoft Teams, Signal, BlueBubbles/iMessage, QQ, API access, and related transports.

**[inferred]** Messaging breadth is a distribution advantage and a verification burden. Every transport adds identity, threading, formatting, attachment, approval, and delivery semantics around the same agent.

### ACP

**[source]** Hermes's ACP adapter:

- uses standard I/O JSON-RPC with protocol-only stdout;
- persists ACP sessions in the canonical session database;
- supports list, load, fork, and resume;
- translates working directories, including WSL paths;
- exposes the reduced <code>hermes-acp</code> toolset plus configured MCP servers;
- emits native ACP tool calls, todo/plan updates, and edit permission requests.

**[inferred]** ACP is the right initial boundary for Omega because it gives Omega a native agent conversation and permission surface without embedding Hermes's desktop, gateway supervisor, or provider stack.

## Desktop architecture

### Application composition

**[source]** Hermes Desktop uses Electron 40, React 19, Vite 8, TypeScript 6, Nanostores, TanStack Query, assistant-ui, CodeMirror, xterm, node-pty, Radix primitives, Motion, and Tailwind CSS.

The renderer entry point:

- mounts a React 19 root;
- uses a hash router;
- installs one TanStack Query client;
- initializes locale, theme, and haptics;
- shares one tooltip provider;
- dispatches special windows for quick entry and the pet overlay.

The main application renders a <code>ContribController</code>. Routes, sidebar items, overlays, panes, settings, and related surfaces are contribution-driven.

**[source]** Main routes include chat sessions, settings, command center, skills, messaging, webhooks, artifacts, cron, profiles, agents, and the Starmap. Contributions can add routes and navigation.

### Process boundary

~~~mermaid
flowchart TB
    subgraph Renderer["React renderer — sandboxed"]
        UI["Chat / projects / panes / settings"]
        GW["HermesGateway"]
        STORES["Nanostores + TanStack Query"]
        PLUGINS["Trusted renderer plugins"]
    end

    subgraph Electron["Electron main — machine capability owner"]
        IPC["Validated IPC handlers"]
        WIN["Windows / protocol / notifications"]
        FS["Filesystem / clipboard / external URLs"]
        PTY["PTY / Git / worktrees"]
        TOKENS["safeStorage token vault"]
        SUP["Backend supervisor"]
    end

    subgraph Python["Per-profile Python child"]
        SERVE["hermes serve"]
        RPC["JSON-RPC + streamed events"]
        AGENT["AIAgent + state.db"]
    end

    UI --> GW
    UI --> IPC
    PLUGINS --> GW
    PLUGINS --> IPC
    IPC --> FS
    IPC --> PTY
    IPC --> WIN
    IPC --> TOKENS
    SUP --> SERVE
    GW -->|"Bearer-auth REST / WS"| RPC
    RPC --> AGENT
~~~

**[source]** Renderer windows use:

- <code>contextIsolation: true</code>;
- <code>sandbox: true</code>;
- <code>nodeIntegration: false</code>.

The renderer does not receive direct Node access. <code>preload.ts</code> exposes explicit operations through <code>contextBridge</code>.

### Preload capability surface

The bridge includes:

- connection and backend lifecycle;
- profile discovery and selection;
- authenticated REST helpers;
- notifications and microphone access;
- bounded file reads and file watching;
- path and clipboard helpers;
- preview and external-URL operations;
- settings, zoom, logs, and update controls;
- file rename, writes, trash, reveal, and directory open;
- Git review, worktrees, stage/unstage, revert, commit, push, and pull-request actions;
- PTY create, input, resize, metadata, persistence, and close;
- event subscriptions for backend, terminal, window, and updater state.

**[inferred]** Context isolation is meaningful, but the bridge is broad enough that renderer compromise still has workstation consequences. The security boundary is capability validation in Electron main, not the fact that Node integration is disabled.

### Window and navigation controls

**[source]** Electron main denies arbitrary in-app window creation, sends allowed HTTP, HTTPS, mail, and file targets to operating-system handlers, and prevents navigation away from the expected development server or packaged file origin.

The app enables <code>webviewTag</code> for chat windows, keeps developer tools available, and permits autoplay without a user gesture for voice features.

**[inferred]** The baseline Electron posture is better than many desktop wrappers. Webview use and the breadth of privileged IPC remain areas that need continuing threat-model coverage.

## Desktop backend lifecycle

### Local backend

**[source]** The canonical desktop command is:

<code>hermes serve --host 127.0.0.1 --port 0</code>

with an optional profile. Older installed runtimes fall back to the dashboard command with <code>--no-open</code>.

Electron:

1. generates a random 32-byte session token;
2. selects an ephemeral loopback port;
3. starts the profile's Hermes process with desktop and authentication environment variables;
4. waits for a structured ready line;
5. probes HTTP and WebSocket authentication before treating the backend as usable.

Cold startup has a 90-second timeout. The ready parser accepts the current <code>HERMES_BACKEND_READY</code> signal and a legacy dashboard signal.

### Profile pool

**[source]** The main process keeps a primary backend and can start additional local or remote profile backends. It uses a soft least-recently-used limit and idle reaping while sparing active sockets.

**[inferred]** A backend-per-profile is a useful isolation and reliability primitive. The desktop can show several profiles concurrently without teaching the Python runtime to safely switch its process-global home.

### <code>hermes serve</code>

**[source]** The Python command starts the same gateway used by dashboard/TUI paths but marks it headless. <code>hermes_cli.web_server</code> mounts the gateway WebSocket at <code>/api/ws</code> and refuses to mount the browser SPA when <code>HERMES_SERVE_HEADLESS=1</code>.

Non-loopback serving has a fail-closed authentication gate.

**[inferred]** Hermes Desktop is a client of a real service boundary, even when it launches that service itself. This is the most reusable part of the desktop architecture.

## Remote and cloud connections

### Direct remote

**[source]** Desktop supports remote <code>hermes serve</code> endpoints with token or OAuth authentication. Native tokens are encrypted with Electron <code>safeStorage</code>, backed by the operating-system credential facility, and cached in memory. OAuth uses an HttpOnly cookie partition; WebSocket connections can use short-lived minted tickets.

### SSH-managed remote

**[source]** For SSH connections, Desktop can:

- find or launch a detached remote <code>hermes serve</code>;
- establish a local tunnel;
- validate lockfile and fingerprint information;
- adopt the server's token;
- clean up a stale process only when it can demonstrate ownership.

### Cloud profiles

**[source]** Profile descriptors can represent cloud backends, allowing local and remote profiles to coexist in the same desktop.

**[inferred]** Hermes has moved beyond “localhost WebSocket” assumptions. Token custody, OAuth cookies, WebSocket tickets, SSH process ownership, and profile descriptors make connection identity a first-class desktop concern.

## Renderer state and streaming

### Gateway client

**[source]** <code>HermesGateway</code> extends a shared JSON-RPC WebSocket client. Default request timeout is 30 seconds, startup allows 60 seconds, and prompt execution allows up to 30 minutes.

The event vocabulary includes:

- gateway ready and session information;
- message start, delta, interim, complete;
- thinking and reasoning;
- tool start, progress, completion, and generation;
- clarification, approval, sudo, and secret requests;
- background work and errors.

Plugin REST and WebSocket namespaces reject parent traversal.

### Two state systems

**[source]** The renderer uses Nanostores for hot shell and live-session state, and TanStack Query for request-oriented data.

**[inferred]** The split matches update shape: token and tool streams need direct low-latency mutation; settings, lists, and metadata benefit from query caching and invalidation.

### Stable and runtime session identities

**[source]** Desktop distinguishes the stable stored session/lineage identity from the current runtime identity. The route, composer scope, and session tile remain attached to the durable identity while compressed or resumed work can move to a new live tip.

Repository tests cover session-context drift and compression cases.

**[inferred]** This is a crucial correctness property. A UI that keys everything by the latest runtime ID will lose draft, route, unread, and pane continuity after compression.

### Multi-session behavior

**[source]** Desktop maintains runtime state for multiple session tiles and secondary windows. It supports queued prompts, steering, interruption, reconnect, resume, attachments, reasoning/tool streams, stall watchdogs, settle grace periods, and unread completion state.

Secondary profile gateways reconnect and are pruned when no longer needed.

**[inferred]** Hermes treats background sessions as live work, not hidden API requests. The desktop has explicit states for “still running elsewhere,” “finished unread,” “stalled,” and “reconnected.”

## Projects, files, terminals, and Git

### Projects

**[source]** Projects are stored through backend project RPC and own the working directory. Desktop can discover repositories and construct project-scoped session trees.

### Files and artifacts

**[source]** The workbench includes file trees, previews, CodeMirror editors, watched artifacts, and contributed pane types. Pane layout, tabs, split trees, sizes, and drag/drop zones are persisted.

### Terminal

**[source]** Electron main owns node-pty. Renderer terminals use xterm and support:

- persistent terminal metadata;
- revived scrollback buffers;
- resize and lifecycle;
- OSC working-directory tracking;
- agent-process terminals.

**[inferred]** Terminal persistence makes the desktop feel like a workspace, but it also means process ownership and cwd must remain coherent across window, project, profile, and app restarts.

### Git

**[source]** The Electron bridge supports:

- repository status and branch operations;
- diffs and review;
- stage, unstage, and revert;
- commit and push;
- worktree creation and removal;
- pull-request creation through the GitHub CLI.

**[inferred]** Git is not implemented as an agent tool rendered in chat. It is a native desktop capability with direct manipulation and optimistic UI. That is a good distinction for high-frequency, inspectable actions.

## Desktop plugins

### Loading model

**[source]** Desktop can load ESM plugins from:

<code>$HERMES_HOME/desktop-plugins/&lt;id&gt;/plugin.js</code>

The loader:

- rewrites only approved bare imports, including the SDK and React;
- creates a blob module;
- checks the exported plugin interface;
- registers contributions;
- watches plugin files and directories for hot reload;
- isolates individual plugin crashes from shell registration.

Integrity metadata can verify that bytes match an expected digest.

### Trust model

**[source]** A code comment explicitly states that the loader is **not a capability boundary**. Plugins execute in the renderer realm with application authority, including host RPC, authenticated REST, storage, and navigation. Agent-written local plugins can enter through the same disk door.

The source warns that remote plugins must not reuse this pipeline without a worker or iframe, CSP, and capability gating.

**[inferred]** Integrity answers “are these the expected bytes?” It does not answer “may this code use the filesystem bridge, credentials, Git, or terminals?” Hermes's local plugin model is trusted-code extensibility.

**Recommendation:** OpenAgents should separate declarative UI contributions from privileged extension code, give every extension an authority manifest, and make agent-created extensions proposals until a user approves both code and capabilities.

## Desktop updater and packaging

### Packaging

**[source]** Electron Builder produces:

- macOS DMG and ZIP;
- Windows NSIS and MSI;
- Linux AppImage, DEB, and RPM.

The app uses ASAR with native modules unpacked. macOS packaging enables hardened runtime, entitlements, and a notarization hook when credentials are available.

At the audited revision, Electron Builder has <code>signAndEditExecutable: false</code> for Windows.

**[limitation]** The source configuration does not establish whether every distributed Windows artifact is unsigned; release infrastructure may add signing elsewhere. It does mean code signing is not evident in the audited package configuration.

### Source-oriented update path

**[source]** Desktop update flows can:

1. run <code>hermes update</code>, which updates a Git checkout and dependencies;
2. run <code>hermes desktop --build-only</code>;
3. stage and replace or relaunch the desktop application.

Windows uses a staged updater binary and virtual-environment lock handoff. macOS and unpacked Linux have replacement paths. A packaged Linux build may update the backend but require a separate GUI reinstall/restart.

The update code includes:

- update gates and markers;
- virtual-environment blocker scans;
- emergency SQLite backup;
- stale backend cleanup;
- relaunch and staged handoff logic.

### Update assessment

**[inferred]** The implementation is cautious about interruption, but the update unit is not always an immutable, signed application artifact. Pulling and rebuilding a moving source checkout expands the supply-chain and reproducibility boundary and can create backend/GUI version skew.

OpenAgents should prefer signed, immutable artifacts with an explicit protocol compatibility range and transactional rollback.

## Security and trust boundaries

### What Hermes gets right

- **[source]** Renderer isolation is enabled and Node integration is disabled.
- **[source]** A preload bridge enumerates machine capabilities instead of exporting a raw Node object.
- **[source]** Local backends bind loopback, use random bearer credentials, and require HTTP and WebSocket authentication probes.
- **[source]** Remote credentials use OS-backed encryption; OAuth and WebSocket authentication are separated.
- **[source]** Navigation and new-window behavior are constrained.
- **[source]** File reads and writes include size and path checks in several sensitive handlers.
- **[source]** Destructive terminal patterns and deny rules can remain active even when normal approvals are off.
- **[source]** Cron and delegated work have more conservative unattended-operation policies.
- **[source]** The ACP surface is reduced and emits native permission requests.
- **[test]** Supply-chain, lockfile, package-content, native-module, security, and end-to-end checks exist in CI.

### Main risks

#### 1. Broad renderer-to-host authority

**[source]** The preload surface includes filesystem, terminal, Git, external-open, update, profile, and backend operations. Some handlers validate narrow roots and sizes; higher-level reveal, open-directory, and trash operations accept broader renderer-provided paths.

**[inferred]** A compromised trusted renderer is not confined to chat data. It can reach meaningful workstation capabilities through intended IPC.

#### 2. Trusted renderer plugins

**[source]** Desktop plugins share the renderer realm and its bridge. Agent-created plugin files can hot reload.

**[inferred]** This is the sharpest desktop-specific trust edge because self-improvement can become code execution in a privileged UI origin.

#### 3. Ambient local tool authority

**[source]** Local execution is a normal backend, and yolo mode bypasses ordinary prompts. Hardline blocks reduce catastrophic commands but do not constitute least privilege.

**[inferred]** “The agent may run local commands unless stopped” is a materially different trust model from scoped capabilities granted per project or task.

#### 4. Persistent prompt mutation

**[source]** Memories, skills, souls, project instructions, plugin hooks, and external memory all affect future model context. Some writes can happen through background review without approval.

**[inferred]** A bad turn can have durable behavioral impact without modifying source code. Review and rollback need to be as visible as file diffs.

#### 5. Update and compatibility complexity

**[source]** Desktop may update source, dependencies, Python runtime, and GUI through different OS-specific paths, with legacy command fallback.

**[inferred]** Compatibility and supply-chain reasoning are harder when the backend and desktop are independently mutable.

#### 6. Surface-specific approval behavior

**[source]** Permission callbacks and defaults differ among CLI, desktop/gateway, ACP, cron, and delegated tasks.

**[inferred]** A user can understand one surface's prompts and still mispredict another's unattended behavior.

## Performance and quality signals

### Performance work

**[source]** Several comments and tests encode measured regressions:

- permanent disabling of Chromium background throttling added roughly 20% idle CPU, so it is scoped to active streams;
- React Router transitions were disabled because continuous stream/store updates could starve route commits;
- one shared tooltip provider replaced 107 providers and approximately 52,000 renders during sash dragging;
- dedicated harnesses simulate streaming and store churn.

**[inferred]** Hermes has crossed the threshold where chat streaming, multi-pane React state, and Electron lifecycle require product-specific performance engineering.

### Test and CI investment

**[test]** The repository includes:

- more than 2,600 Python test files;
- 468 desktop test files and 18 desktop end-to-end specs;
- isolated Python test execution;
- lint and JavaScript test workflows;
- Playwright desktop runs with real API keys stripped;
- screenshot, trace, and visual-diff artifacts;
- package-content and node-pty checks;
- clean/fresh install, DMG, and NSIS coverage;
- supply-chain, OSV, and lockfile checks.

**[limitation]** Test count is not coverage, and this audit did not execute Hermes's suite.

### Velocity and debt

**[history]** Hermes has more than 24,000 commits in approximately one year of repository history. Desktop accumulated more than 2,100 touching commits after its initial landing.

Large coordination files remain:

| File | Approximate lines |
|---|---:|
| <code>hermes_cli/cli.py</code> | 18,363 |
| <code>hermes_cli/web_server.py</code> | 17,368 |
| <code>tui_gateway/server.py</code> | 13,685 |
| <code>apps/desktop/electron/main.ts</code> | 11,969 |
| <code>hermes_state.py</code> | 8,767 |
| <code>run_agent.py</code> | 7,551 |

**[inferred]** Extraction and testing are active, but feature breadth is outpacing simplification. The largest risk is not a single god file; it is cross-cutting behavior duplicated or conditioned across providers, surfaces, profiles, and operating systems.

## OpenAgents and Omega comparison

| Question | Hermes answer | OpenAgents/Omega implication |
|---|---|---|
| Where does agent authority live? | Python runtime and its tool backends | Keep agent execution behind an explicit service or protocol boundary |
| Where does desktop machine authority live? | Electron main via preload IPC | Do not let chat/render code directly own host authority |
| How is continuity preserved? | Canonical SQLite sessions, exact API replay, lineage, shared gateway | Adopt durable-versus-runtime identity and provider-facing replay |
| How are multiple profiles isolated? | Separate <code>HERMES_HOME</code> and backend child processes | Process isolation is safer than switching global configuration |
| How does Omega attach today? | ACP with a reduced toolset and permission events | Attach through ACP before considering Hermes-specific APIs |
| What requires <code>hermes serve</code>? | Full gateway RPC, profiles, desktop-style session/event features | Add only for concrete functionality absent from ACP |
| How are extensions trusted? | Local renderer plugins are trusted code | Require capability-scoped, reviewable extension isolation |
| How does self-improvement work? | Memory and skill files mutated by agent/background review | Treat instruction mutation as governed durable state |
| How are updates delivered? | Packaged artifacts plus source-oriented update/rebuild paths | Prefer immutable signed artifacts and protocol versioning |

## Recommended OpenAgents posture

### Adopt

1. **Stable session scope plus runtime tip**
   - Route, drafts, panes, and user identity stay on a stable lineage.
   - Model execution can advance to compressed or resumed tips.

2. **Exact provider-facing replay**
   - Persist display content separately from the exact API payload.
   - Preserve prompt-cache prefixes and resume fidelity.

3. **Separated agent and desktop authorities**
   - A local service owns agent state and execution.
   - A desktop main process owns explicit machine capabilities.
   - The renderer remains an untrusted presentation layer.

4. **A typed streaming vocabulary**
   - Message, reasoning, tool, approval, clarification, secret, background, and error events should be distinct.

5. **Deterministic parallel tool results**
   - Parallelize only when authorized and conflict-free.
   - Reassemble results in model-call order.

6. **Performance regression scenarios**
   - Test token streams, tool progress, panes, window changes, and route commits together.

7. **Board ownership boundaries**
   - Durable workflow state should be mutated by an owning coordinator, not arbitrary child agents.

### Adapt

1. **Profile backend pool**
   - Keep isolation and concurrency.
   - Add resource budgets, protocol negotiation, health state, and explicit eviction semantics.

2. **Gateway contract**
   - Keep the event and session concepts.
   - Split the contract by capability and publish versioned schemas rather than one expanding server façade.

3. **Memory and skill review**
   - Keep background extraction as an option.
   - Default durable mutations to proposals with diff, provenance, scope, expiry, and rollback.

4. **Approval policy**
   - Keep hard unconditional blocks and user deny rules.
   - Make effective policy visible per operation and consistent across surfaces.
   - Avoid using an auxiliary model as the sole authority for irreversible work.

5. **Native Git and terminal UX**
   - Keep direct manipulation for inspectable operations.
   - Bind authority to a project root and make process/worktree ownership explicit.

6. **Remote connection model**
   - Keep loopback tokens, OAuth cookies, WebSocket tickets, and SSH ownership checks.
   - Centralize credential custody and protocol compatibility.

### Avoid

1. **Embedding the complete Hermes shell as Omega architecture.**
2. **Assuming Electron sandbox flags make a broad preload bridge low-risk.**
3. **Loading agent-written JavaScript into the trusted renderer realm.**
4. **Equating optional container backends with universal sandboxing.**
5. **Making normal application updates depend on a mutable Git checkout and local rebuild.**
6. **Allowing permission defaults to drift by surface without an inspectable effective policy.**
7. **Putting more responsibility into already-large gateway, Electron-main, CLI, and state coordinators.**

## Integration sequence

### Phase 1: ACP attach

- Launch or discover Hermes through its ACP adapter.
- Map Hermes sessions and native tool/permission events into Omega.
- Verify cwd, WSL, resume/fork, cancellation, and reduced-toolset behavior.
- Keep Omega as the primary desktop/workbench surface.

### Phase 2: Session fidelity

- Decide whether ACP exposes enough lineage, cost, compression, and background completion state.
- If not, propose narrow additions to the adapter before importing a second protocol.

### Phase 3: Selective gateway evaluation

Evaluate <code>hermes serve</code> only for concrete needs such as:

- concurrent profile backends;
- Hermes-specific skills, memory, cron, agents, or learning views;
- full streamed session events not represented by ACP;
- remote/cloud Hermes profile connections.

Require protocol versioning, capability discovery, and authentication tests before treating it as a stable external API.

### Phase 4: Harvest patterns, not UI

Reimplement the useful desktop concepts in Omega's own architecture:

- stable session tiles;
- project-scoped working context;
- persistent terminal ownership;
- Git review and worktree state;
- background completion/unread semantics;
- inspectable memory and skill provenance.

## Questions for follow-up validation

The source audit leaves several runtime questions worth testing:

1. Does ACP preserve session lineage and recovery semantics through compression exactly as Desktop does?
2. Which gateway methods are intended as stable external API versus internal desktop implementation detail?
3. How does a packaged release negotiate an older or newer <code>hermes serve</code> protocol?
4. What user-visible review occurs before an agent-created desktop plugin becomes active?
5. Are production Windows artifacts signed outside the audited Electron Builder configuration?
6. How do approvals behave for the same dangerous action across CLI, Desktop, ACP, cron, and a delegated child?
7. What happens to an active delegated task or cron job across process crash and profile-backend eviction?
8. Can remote/cloud profiles use every local desktop feature safely, or do file, terminal, and Git capabilities silently fall back to the local machine?

## Primary source map

All source links below are pinned to the audited revision.

### Project and agent

- [README](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/README.md)
- [Python project metadata](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/pyproject.toml)
- [AIAgent façade](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/run_agent.py)
- [Conversation loop](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/conversation_loop.py)
- [Prompt builder](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/prompt_builder.py)
- [System prompt](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/system_prompt.py)
- [Tool executor](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/tool_executor.py)
- [Tool dispatch helpers](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/tool_dispatch_helpers.py)
- [Tool registry](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/tools/registry.py)
- [Approval policy](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/tools/approval.py)
- [Toolsets](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/toolsets.py)
- [State implementation](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/hermes_state.py)
- [State schema](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/hermes_state_schema.py)
- [Memory manager](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/memory_manager.py)
- [Memory provider boundary](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/memory_provider.py)
- [Skill curator](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/agent/curator.py)
- [Delegation tool](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/tools/delegate_tool.py)
- [Cron tools](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/tools/cronjob_tools.py)
- [Kanban worker orchestration](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/hermes_cli/kanban_swarm.py)
- [Headless/dashboard subcommand](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/hermes_cli/subcommands/dashboard.py)
- [Web server and gateway mount](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/hermes_cli/web_server.py)
- [Gateway server](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/tui_gateway/server.py)
- [ACP adapter](https://github.com/NousResearch/hermes-agent/tree/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/acp_adapter)
- [Model-provider plugins](https://github.com/NousResearch/hermes-agent/tree/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/plugins/model-providers)

### Desktop

- [Desktop README](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/README.md)
- [Desktop design principles](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/DESIGN.md)
- [Desktop package and build configuration](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/package.json)
- [Electron main process](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/electron/main.ts)
- [Electron preload bridge](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/electron/preload.ts)
- [Backend command construction](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/electron/backend-command.ts)
- [Backend readiness parsing](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/electron/backend-ready.ts)
- [Renderer entry point](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/src/main.tsx)
- [Desktop gateway client](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/src/hermes.ts)
- [Contribution controller](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/src/app/contrib/controller.tsx)
- [Runtime plugin loader](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/src/contrib/runtime-loader.ts)
- [Desktop plugin SDK documentation](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/website/docs/developer-guide/desktop-plugin-sdk.md)
- [Desktop update rebuild](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/electron/update-rebuild.ts)
- [Stable/runtime session drift logic](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/src/app/session/hooks/session-context-drift.ts)
- [Shared JSON-RPC gateway client](https://github.com/NousResearch/hermes-agent/blob/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/shared/src/json-rpc-gateway.ts)
- [Desktop end-to-end tests](https://github.com/NousResearch/hermes-agent/tree/87bc710609f8b89b6e6b4aa418dde8ee30ec6873/apps/desktop/e2e)

### Representative source digests

These SHA-256 digests make the most important inspected files independently identifiable even if web links later render differently.

| File | SHA-256 |
|---|---|
| <code>README.md</code> | <code>1d733df90c2836c1b9cd8d7307d2d222023456b7b55b5036c2fbb54fd8bcfdf4</code> |
| <code>pyproject.toml</code> | <code>2d8c6e3391e9d7d32e7988887a357ffc9ea53d2535be7c5657e46a2df7885378</code> |
| <code>run_agent.py</code> | <code>0695f84260cdc1c36f1fa461e1ee74fc903f4b240fc1f8b1c9496458a0e43217</code> |
| <code>agent/conversation_loop.py</code> | <code>2820909411499fb3e255f61e419871c91ce66bd398bfb2c5c726dd5e3f78d49e</code> |
| <code>agent/prompt_builder.py</code> | <code>2a8a8b2b6c0ff830349f68b5c2f26bf2c9871ae03caf516cff3628c00b5769c7</code> |
| <code>agent/tool_executor.py</code> | <code>3a133d293316210e48c2d435fc710c93e2d8555e01c0c6a68fe879eea57d6621</code> |
| <code>tools/registry.py</code> | <code>7ff70e4a00e9c499bf8548abb8dafd36dba0f7b8a1dab328391bbbc0428f1658</code> |
| <code>tools/approval.py</code> | <code>a401011b0a040ea705ab866618ec14a8beb77851984cbb404fc7a3cfba85b480</code> |
| <code>hermes_state.py</code> | <code>badd6f498e0ac1e393c9466126f9a432aada3c26e5d31bb0c3317fbd27367ed6</code> |
| <code>hermes_state_schema.py</code> | <code>e0e217b0599afa78a87ee48730cd5075460b44221b4f5f4c342d348ccbf7d5fc</code> |
| <code>apps/desktop/README.md</code> | <code>fab9d287054fd4ba9570ef35db3f29fdd24c5106e958baa43a6bd72bc79b13ae</code> |
| <code>apps/desktop/package.json</code> | <code>4f7120a138b651d9be4ea4550f13b636fedc74b03dce9c4a046193d31b11b59d</code> |
| <code>apps/desktop/electron/main.ts</code> | <code>8d48df8c6dba4aef3905bc17de2c227c8d4e57c363ee21ef73f2e78e82ae57e6</code> |
| <code>apps/desktop/electron/preload.ts</code> | <code>2278bf0e699be6f0efda2df2796c4f4d653d74dd378a724353117405c85b9a79</code> |
| <code>apps/desktop/src/main.tsx</code> | <code>808b036b7b8b0ac05224ef59d99434dcba7ca2500e3cf02776daa17665e3690e</code> |
| <code>apps/desktop/src/hermes.ts</code> | <code>8256c262537aabd92d1c9fe4ab5a1a483a23cc88d35d461fe6f52e4f8bf7e59b</code> |
| <code>apps/desktop/src/contrib/runtime-loader.ts</code> | <code>308bd5950d16c20b05501707a3724c01725d25cf6a0e90fb32183bf98a611f1c</code> |
| <code>apps/shared/src/json-rpc-gateway.ts</code> | <code>764bdb84e34d9ed141fa08b9d19ca2ff0370c853ab6164a466974dbdfa9b1a61</code> |

## Final assessment

Hermes demonstrates that an open personal agent can sustain a real desktop workbench without making the desktop the agent runtime. Its session model, gateway events, provider-facing replay, profile processes, and direct-manipulation project tools are the highest-value patterns to harvest.

It also demonstrates why Omega should stay an orchestrating surface. Replacing Omega's architecture with Hermes's whole Electron/Python stack would import a rapidly expanding authority and compatibility envelope. ACP offers the safer first integration. A narrowly versioned <code>hermes serve</code> relationship can follow if a specific Hermes-native capability justifies it.
