# OpenAgents Vibe

**Agentic development environment for building products**

---

## Overview

**OpenAgents Vibe** is an agentic IDE for "vibe coding" - building websites, apps, and products with AI assistance. Scaffold entire apps via prompts. Refactor code with AI. See what agents are doing via ATIF trajectories. Ship products.

**Important: Vibe starts as part of the OpenAgents desktop app**, built with GPUI like everything else. Browser extraction comes later. We build and prove the model natively first, then extract to browser when the architecture is validated.

Inspired by WANIX and Plan 9 from Bell Labs, Vibe uses OANIX namespaces to provide clean abstractions that work identically in native and browser contexts.

**Stack:**
- **Frontend**: React/JS/TS with HMR
- **Backend**: Rust compiled to WASM (no Node.js)
- **IDE**: GPUI (native first, browser later)
- **Runtime**: OANIX kernel for namespaces, jobs, and capabilities

---

## Product Position

Part of the OpenAgents family:

| Product | Purpose |
|---------|---------|
| **Commander** | Command multiple agents, StarCraft-style control |
| **Gym** | Training & benchmarking (TerminalBench, hillclimbers) |
| **Vibe** | Build products - websites, apps, full-stack projects |
| **Marketplace** | Tasks, tools, and compute trading |
| **Wallet** | Bitcoin/Lightning payments |

**Commander vs Vibe:**
- **Commander** = Commanding agents. Multi-agent orchestration, monitoring, APM-style control.
- **Vibe** = Creating products. Single-focus IDE for building a website or app with agent assistance.

Commander may eventually use Vibe components (editor, preview). They're siblings in the same desktop app, not separate products.

---

## Goals

1. **Native-first development**
   - Built into OpenAgents desktop app with GPUI
   - Same codebase extracts to browser later via WASM
   - Prove the model locally before adding browser complexity

2. **Product-focused IDE**
   - For building websites, apps, landing pages, full-stack products
   - Not for benchmarking or agent training (that's Gym)
   - Simple: open project, vibe code, ship

3. **Agent-native workflows**
   - First-class UI for running coding agents
   - Built-in ATIF trajectory browsing
   - Scaffold, refactor, generate - all agent-powered

4. **Rust backend, React frontend**
   - No Node.js - backend handlers are Rust compiled to WASM
   - Frontend is familiar React/JS/TS with modern DX

5. **Local-first state**
   - All workspace state persists on user's computer
   - No cloud sync required for basic usage
   - Optional sync/sharing comes later

---

## Architecture

### Phase 1: Native (Current Focus)

```
+-------------------------------------------------------------+
| OpenAgents Desktop App                                      |
|-------------------------------------------------------------|
|  Vibe Screen (GPUI)                                         |
|    - Editor, tabs, tree, palette                            |
|    - Terminal panel                                         |
|    - Preview panel (WebView or native)                      |
|    - Agent panel + ATIF viewer                              |
|                                                             |
|  Dev Runtime Layer                                          |
|    - File watcher on /workspace                             |
|    - Bundler (native or spawned process)                    |
|    - Preview server (localhost)                             |
|                                                             |
|  OANIX Kernel (Rust library)                                |
|    - Namespace (/workspace, /logs, /cap/*)                  |
|    - Real filesystem for WorkspaceFs                        |
|    - Wasmtime for WASI jobs                                 |
+-------------------------------------------------------------+
| macOS / Native OS                                           |
+-------------------------------------------------------------+
```

### Phase 2: Browser (Later)

```
+-------------------------------------------------------------+
| Browser Tab                                                 |
|-------------------------------------------------------------|
|  Vibe IDE (GPUI compiled to WASM)                           |
|    - Same components as native                              |
|    - DOM rendering instead of Metal                         |
|                                                             |
|  Dev Runtime Layer (WASM)                                   |
|    - SWC/esbuild-style bundler in WASM                      |
|    - Service Worker for preview                             |
|                                                             |
|  OANIX Kernel (Rust->WASM)                                  |
|    - Same namespace abstraction                             |
|    - IndexedDB for WorkspaceFs                              |
|    - Browser WASM for jobs                                  |
+-------------------------------------------------------------+
| Browser: DOM, WebAssembly, Service Worker                   |
+-------------------------------------------------------------+
```

The key insight: **same OANIX abstractions, different implementations**.

### Key Components

1. **OANIX Kernel** - Virtual FS, WASI runner, PTY, capabilities
2. **Vibe IDE** - Zed-inspired editor with tree, tabs, terminal, agent panel
3. **Dev Runtime** - Browser-based bundler + module graph + HMR
4. **Preview Frame** - Live app preview via Service Worker or in-frame loader

---

## Project Layout

Default full-stack template in `/workspace`:

```
/workspace
  /frontend
    index.html
    src/
      main.tsx
      App.tsx
      apiClient.ts
  /backend
    Cargo.toml
    src/
      lib.rs
      routes.rs
      models.rs
  vibe.toml
```

**vibe.toml** configuration:

```toml
[frontend]
entry = "frontend/index.html"

[backend]
crate_path = "backend"
router = "backend::routes::router"

[routes]
"/api/hello" = { method = "GET", handler = "backend::routes::hello" }
"/api/todos" = { method = "POST", handler = "backend::routes::create_todo" }
```

---

## Rust Backend Runtime

### Authoring Model

Simple HTTP-ish framework for WASM:

```rust
// backend/src/routes.rs
use vibe_backend::{Request, Response, Router};

pub fn router() -> Router {
    Router::new()
        .get("/api/hello", hello)
        .post("/api/todos", create_todo)
}

pub async fn hello(_req: Request) -> Response {
    Response::json(serde_json::json!({ "message": "Hello from Rust" }))
}

pub async fn create_todo(req: Request) -> Response {
    let todo: NewTodo = req.json().await?;
    Response::json(todo)
}
```

### Build Pipeline

1. Backend sources in `/workspace/backend`
2. Compilation service produces `backend.wasm`
3. OANIX runtime loads module and routes `/api/*` requests

### Request Routing

Frontend `fetch('/api/hello')` is intercepted:
- Service Worker catches `/api/*` requests
- Routes to OANIX Rust backend runtime
- Returns Response to preview frame

---

## Frontend Dev Runtime

### Capabilities

- **Module graph** - Resolve imports starting from entry HTML/TSX
- **Transpilation** - TS/JS/JSX/TSX via WASM bundler (SWC-style)
- **HMR** - Push module updates to preview frame
- **Framework presets** - React, Vue, Svelte, Solid templates

### Pipeline

1. IDE writes to `/workspace/frontend/src/...`
2. Dev runtime sees OANIX FS change event
3. Transforms run in WASM (TS->JS, JSX->JS)
4. Module graph updates
5. Preview frame reloads or HMR patches

---

## IDE Features

### Layout

```
+-------------------------------------------------------------+
|  Sidebar              |   Main Editor / IDE                 |
|-----------------------+-------------------------------------|
|  Workspaces           |  [ Tabs: main.rs | config.toml ]    |
|  OANIX Sessions       |                                     |
|  Agents               |  Code editor (Monaco/GPUI)          |
|  Jobs / Runs          |                                     |
+-----------------------+-------------------------------------+
|  Terminal (PTY->OANIX) | Agent Log / ATIF Viewer (split)    |
+-------------------------------------------------------------+
```

### Core Features

- **Tree view** over `/workspace`
- **Multi-file editor** with multi-cursor, language features, theming
- **Terminal panel** connected to OANIX PTYs
- **Preview panel** showing live app
- **Jobs & Agents panel** for OANIX jobs, logs, ATIF

### Backend Traits

```rust
pub trait IdeFs {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;
    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>>;
    async fn remove_file(&self, path: &str) -> Result<()>;
    async fn create_dir_all(&self, path: &str) -> Result<()>;
}

pub trait TerminalBackend {
    async fn open(&self, name: &str) -> Result<TerminalHandle>;
    async fn write(&self, handle: TerminalHandle, data: &[u8]) -> Result<()>;
    fn subscribe(&self, handle: TerminalHandle, on_data: Box<dyn Fn(Vec<u8>)>);
}

pub trait JobBackend {
    async fn submit(&self, job: JobSpec) -> Result<JobId>;
    async fn status(&self, id: JobId) -> Result<JobStatus>;
    async fn logs(&self, id: JobId) -> Result<Vec<LogEntry>>;
}
```

---

## Agent Integration

### Job Types

```rust
pub enum AgentJob {
    ScaffoldFullStack { prompt: String },
    RefactorFile { path: String, range: Range, instructions: String },
    AddEndpoint { path: String, method: String, schema: Value },
    GenerateTests { scope: TestScope },
    TerminalBench { task_id: String },
}
```

### Agent Workflow

1. User clicks "Add an API endpoint" or types a prompt
2. Agent job submitted to OANIX
3. Agent reads `/workspace`, generates patches
4. Changes written back to `/workspace`
5. ATIF trajectory logged to `/logs/atif/`
6. Dev runtime rebuilds, preview updates

### Example Flows

**"Create full-stack app":**
- Agent scaffolds `frontend/` React app
- Agent scaffolds `backend/` Rust routes
- Updates `vibe.toml`
- Dev runtime and backend builder kick in
- Preview comes to life

**"Add an API endpoint":**
- Agent edits `backend/src/routes.rs`
- Adds models if needed
- Backend WASM rebuilds
- TypeScript client stub generated in `frontend/src/apiClient.ts`

---

## OANIX Namespace for Vibe

### Standard Layout

```
/workspace       - WorkspaceFs (project files, read/write)
/logs            - LogsFs (stdout/stderr, build logs, ATIF)
/cap/agents      - Agent capability for refactor/testgen
/cap/net         - (optional) HTTP/WS for preview server
```

### Profiles

- **frontend-only** - React SPA, no backend
- **fullstack** - React + Rust API
- **terminalbench** - Add `/task` with TB spec
- **agent-lab** - Add `/cap/nostr`, `/cap/payments`

---

## Implementation: `crates/vibe/`

Proposed crate structure:

```
crates/vibe/
  Cargo.toml
  src/
    lib.rs
    ide/
      mod.rs
      editor.rs        # Editor component
      tree.rs          # File tree
      tabs.rs          # Tab management
      terminal.rs      # Terminal panel
      preview.rs       # Preview frame
      agents.rs        # Agents panel
      atif_viewer.rs   # ATIF trajectory viewer
    backend/
      mod.rs
      framework.rs     # vibe_backend crate (Request, Response, Router)
      runtime.rs       # Load and call Rust WASM backends
      builder.rs       # Compilation service client
    devrt/
      mod.rs
      bundler.rs       # Module graph and bundling
      transform.rs     # TS/JSX transforms via SWC-wasm
      hmr.rs           # Hot module replacement
      preview.rs       # Service Worker / iframe management
    traits.rs          # IdeFs, TerminalBackend, JobBackend
    config.rs          # vibe.toml parsing
    templates/
      mod.rs
      react.rs         # React SPA template
      fullstack.rs     # React + Rust template
```

### Additional Crate: `crates/vibe-backend/`

Minimal framework for Rust backend handlers:

```
crates/vibe-backend/
  Cargo.toml
  src/
    lib.rs
    request.rs         # Request type
    response.rs        # Response type
    router.rs          # Router builder
    json.rs            # JSON helpers
```

---

## User Stories

### "Vibe-code a landing page"
- Open Vibe, pick "Landing Page" template
- Tell agent "make it a SaaS pricing page with dark mode"
- Agent scaffolds React components, Tailwind styling
- Live preview updates as files change
- Ship it

### "Build a full-stack app"
- Pick "Full Stack" template (React + Rust API)
- Tell agent "add user authentication"
- Agent creates `/api/auth` routes in Rust, login form in React
- Preview shows working login flow
- Deploy when ready

### "Iterate on design"
- Open existing project in Vibe
- Tell agent "make the hero section more minimal"
- Agent refactors components, updates styles
- See ATIF trajectory showing what changed
- Accept or rollback via git

### "Quick prototype"
- Have an idea for a product
- Open Vibe, describe what you want
- Agent scaffolds entire project structure
- Iterate with agent assistance
- Go from idea to working prototype in hours, not days

---

## What Runs Where

### Phase 1: Native Desktop (Current Focus)

Everything runs locally on the user's machine:

| Component | Implementation |
|-----------|---------------|
| Vibe IDE | GPUI (native macOS) |
| OANIX kernel | Rust library |
| FileService | Real filesystem (`std::fs`) |
| Bundler | Bun (spawned process) |
| Preview | WebView or localhost |
| WASI runtime | Wasmtime |
| State | Local filesystem |

### Phase 2: Browser (Later)

Same abstractions, browser implementations:

| Component | Implementation |
|-----------|---------------|
| Vibe IDE | GPUI compiled to WASM |
| OANIX kernel | Rust compiled to WASM |
| FileService | IndexedDB or in-memory |
| Bundler | SWC-wasm or similar |
| Preview | Service Worker + iframe |
| WASI runtime | Browser WASM |
| State | IndexedDB + optional server sync |

### Why This Works

The same `Namespace`, `Mount`, and `FileService` trait code works in both contexts. Only the implementations change. OANIX is the abstraction boundary - Vibe never touches platform APIs directly.

See the [OANIX README](../oanix/README.md#native-first-browser-later) for more details.

---

## Implementation Phases

### Phase 1: Basic Native IDE
- Vibe screen in OpenAgents desktop app (GPUI)
- File tree, editor, terminal panel
- Open existing project from filesystem
- Basic preview (external browser to localhost)
- **Deliverable:** Can open and edit a React project

### Phase 2: Dev Runtime Integration
- Bun integration for bundling/dev server
- Live preview in WebView panel
- File watcher triggers rebuild
- **Deliverable:** Edit code, see live preview

### Phase 3: Agent Integration
- Agent panel in IDE
- Basic scaffolding agent ("create landing page")
- ATIF trajectory viewer
- **Deliverable:** Prompt-driven scaffolding

### Phase 4: Full-Stack Support
- Rust backend templates
- Local cargo builds to WASM
- API route handling
- **Deliverable:** Full-stack React + Rust projects

### Phase 5: Browser Extraction (Later)
- Compile GPUI components to WASM
- Replace filesystem with IndexedDB
- WASM-based bundler
- **Deliverable:** Vibe runs in browser

---

## Why This Approach?

### Zero-Install Entry
- User taps a link -> instant agentic IDE
- No installer, no permissions, no trust decisions
- Works on locked-down machines

### Compute on User's CPU
- Agent work and test runs on user's machine
- Server only syncs results/ATIF
- "Try before you buy" agents without burning infra

### URL-Addressable Environments
- `/env/fullstack?template=react-rust`
- Shareable repros, teaching labs, marketing demos
- Plan 9 meets "send me a link"

### Double Sandboxing
- Browser sandbox isolates from host OS
- WASM sandbox isolates each workload
- OANIX namespace isolates capabilities

### Same Experience Everywhere
- Desktop, laptop, tablet -> same UX
- No separate distributions for macOS/Windows/Linux
- Easy PWA wrapping

---

## Integration with OpenAgents Ecosystem

Vibe is not a standalone product. It's deeply integrated with the existing OpenAgents infrastructure.

### System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Commander (Desktop App)                  │
│         GPUI-based desktop UI with 6 main screens           │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────┬───────┬───┴────┬─────────┬────────┐
        ▼         ▼       ▼        ▼         ▼        ▼
    ┌────────┐ ┌──────┐ ┌────────┐ ┌──────┐ ┌─────┐ ┌────────┐
    │  Gym   │ │Wallet│ │Compute │ │Market│ │Chat │ │ Vibe   │
    │(TB CC) │ │      │ │        │ │place │ │     │ │(browser│
    └────────┘ └──────┘ └────────┘ └──────┘ └─────┘ └────────┘
        │                                            │
        └──────────────────┬─────────────────────────┘
                           │
             ┌─────────────▼──────────────┐
             │      OANIX Kernel          │
             │   (Plan 9-style agent OS)  │
             └─────────────┬──────────────┘
                           │
         ┌─────────┬───────┼───────┬─────────┐
         ▼         ▼       ▼       ▼         ▼
    ┌─────────┐ ┌──────┐ ┌─────┐ ┌─────┐ ┌──────┐
    │  Agent  │ │ LLM  │ │ATIF │ │Nostr│ │Tools │
    │  Crate  │ │Layer │ │Store│ │     │ │      │
    └─────────┘ └──────┘ └─────┘ └─────┘ └──────┘
```

### Relationship to Commander

| Aspect | Commander | Vibe |
|--------|-----------|------|
| **Entry** | Native app (macOS) | Browser URL |
| **Rendering** | GPUI (Metal GPU) | DOM + React |
| **Filesystem** | Real filesystem | OANIX virtual FS |
| **Agent execution** | Wasmtime + orchestrator | WASM module in OANIX |
| **LLM** | FM-Bridge (Apple FM) | FM-Bridge + multi-provider |
| **Build target** | macOS binary | WASM (browser + server) |
| **Use case** | Command center | Development IDE |

**Shared code:** OANIX kernel, ATIF types, LLM layer, UI components (extracted to WASM)

### Crate Dependencies

```
vibe
  └─ oanix (Plan 9-style namespaces)
      └─ (standalone, no deps)

# Vibe will eventually integrate:
vibe
  ├─ oanix
  ├─ atif (trajectory format)
  ├─ llm (multi-provider LLM)
  ├─ agent (orchestrator/subagent)
  └─ nostr (publishing)
```

### Integration with Gym

Gym and Vibe serve different purposes:

- **Gym** = Training and benchmarking agents (Terminal-Bench, hillclimbers, APM)
- **Vibe** = Building products (websites, apps, landing pages)

They don't directly integrate. Vibe is not for running Terminal-Bench tasks - that's Gym's job. However, agents trained in Gym could be used in Vibe for scaffolding and refactoring.

### Integration with Marketplace

Marketplace trades agents, compute, and services via Nostr (NIP-90).

From Vibe:
- Browse published agents to add to workspace
- Publish completed projects to Nostr
- Discover agents that match workspace language/framework
- After running Vibe job, publish scores back to marketplace

### Integration with Agent Crate

The `agent` crate implements orchestrator/subagent architecture:

```rust
// Agent crate types that Vibe will use
pub struct Subtask {
    pub id: String,
    pub description: String,
    pub status: SubtaskStatus,
}

pub enum SubtaskStatus {
    Pending, InProgress, Done, Verified, Failed,
}
```

In Vibe:
- "Add API endpoint" → submits `AgentJob::AddEndpoint` to OANIX
- OANIX spawns agent as WASI module
- Agent edits `/workspace`, writes ATIF to `/logs/atif/`
- Dev runtime sees file changes, rebuilds, updates preview

### Integration with LLM Layer

The `llm` crate provides multi-provider abstraction:

- Anthropic (Claude)
- OpenAI (GPT-4)
- Gemini
- Ollama (local)
- OpenRouter (aggregator)
- FM-Bridge (Apple Foundation Model)

Vibe agents can use any configured provider for scaffolding, refactoring, test generation.

### Integration with ATIF

ATIF (Agent Trajectory Interchange Format) standardizes agent action logs:

```rust
pub struct Trajectory {
    pub schema_version: String,  // "ATIF-v1.4"
    pub session_id: String,
    pub agent: Agent,
    pub steps: Vec<Step>,
}

pub struct Step {
    pub step_id: u64,
    pub type_: StepType,         // Thought, Action, Observation
    pub tool_calls: Vec<ToolCall>,
    pub timestamp: DateTime<Utc>,
}
```

In Vibe:
- Agents write to `/logs/atif/trajectory.jsonl`
- Vibe's "Agent" panel displays trajectory
- Same visualization as Commander (code extracted to browser)

### Integration with Nostr

Nostr provides decentralized identity and publishing:

- **NIP-90:** Publish agents as DVM (Data Validation Machine)
- **NIP-28:** Channels for agent/project discussion
- **NIP-99:** Publish workspaces as events

From Vibe:
- Workspace can be published as Nostr event
- Agents can submit results to relays
- Marketplace discovers published workspaces

---

## Data Flow: End-to-End Example

```
User opens Vibe URL: /env/fullstack?template=react-rust
    ↓
┌────────────────────────────────────────────┐
│ Vibe Browser IDE                           │
│ ├─ File tree: /workspace                   │
│ ├─ Editor: main.tsx, routes.rs             │
│ ├─ Preview: live app                       │
│ └─ Terminal: OANIX PTY                     │
└────────────────────────────────────────────┘
    ↓
User clicks "Add API endpoint"
    ↓
┌────────────────────────────────────────────┐
│ Agent Job Submitted                        │
│ ├─ JobKind: AddEndpoint                    │
│ ├─ path: "/api/users"                      │
│ └─ method: "GET"                           │
└────────────────────────────────────────────┘
    ↓
┌────────────────────────────────────────────┐
│ OANIX Namespace                            │
│ ├─ /workspace → WorkspaceFs (project)     │
│ ├─ /logs → LogsFs (ATIF output)           │
│ └─ /cap/agents → AgentCapability          │
└────────────────────────────────────────────┘
    ↓
Agent runs in WASM sandbox
    ↓
┌────────────────────────────────────────────┐
│ Agent Actions                              │
│ 1. Read /workspace/backend/src/routes.rs   │
│ 2. Generate new endpoint code (LLM call)   │
│ 3. Write updated routes.rs                 │
│ 4. Generate TS client stub                 │
│ 5. Write ATIF to /logs/atif/               │
└────────────────────────────────────────────┘
    ↓
Dev runtime detects /workspace changes
    ↓
┌────────────────────────────────────────────┐
│ Rebuild                                    │
│ ├─ Recompile backend.wasm                  │
│ ├─ Update frontend bundle                  │
│ └─ HMR push to preview                     │
└────────────────────────────────────────────┘
    ↓
User sees:
├─ New endpoint works in preview
├─ ATIF trajectory in Agent panel
└─ Updated code in editor
```

---

## Architectural Decisions

### Decision: OANIX as Abstraction Boundary

**Question:** Where do we draw the line between "Vibe" and "platform"?

**Decision:** OANIX is the abstraction boundary. Vibe never touches raw filesystem, network, or process APIs. Everything goes through OANIX mounts.

**Rationale:**
- Same Vibe code works in browser (OANIX WASM) and native (OANIX with real FS)
- Capabilities are explicit via namespace mounts
- Security model is clear: what you mount is what you get

### Decision: Rust Backend, Not Node

**Question:** What runs `/api/*` routes in Vibe?

**Decision:** Rust compiled to WASM, not Node.js.

**Rationale:**
- No Node runtime to port to browser
- Smaller, faster WASM binaries
- Same language as OANIX kernel
- Deterministic execution for benchmarking

### Decision: Extract from Commander, Don't Rebuild

**Question:** How do we build the Vibe IDE?

**Decision:** Extract GPUI components from Commander, compile to WASM.

**Rationale:**
- GPUI is already high-performance (Zed-proven)
- Components already exist (trajectory viewer, file tree)
- "Native first, browser later" validates the model
- Less code duplication

### Decision: ATIF for All Agent Actions

**Question:** How do we track what agents did?

**Decision:** All agents write ATIF trajectories to `/logs/atif/`.

**Rationale:**
- Standardized format (already used in Commander/Gym)
- Enables replay, debugging, visualization
- Same viewer code in Commander and Vibe
- Publishable to Nostr for sharing

---

## Decisions Made

### Authentication

**Decision:** Same as rest of OpenAgents - Nostr keypair anchored to user's computer.

Flow:
1. First run: Generate Nostr keypair, store locally
2. User is "logged in" by virtue of having the keypair on their machine
3. Optional: Create OpenAgents account later by logging in with GitHub
4. Account links Nostr identity to GitHub for cross-device access

### State Persistence

**Decision:** All state persists locally on user's computer for v1.

- Workspace files live in real filesystem (e.g., `~/Projects/my-app/`)
- ATIF trajectories stored locally
- No cloud sync in first iteration
- Git for versioning (user's choice)

### Terminal-Bench Integration

**Decision:** Vibe does not integrate with Terminal-Bench.

- Vibe is for building products, not benchmarking
- Terminal-Bench workflows belong in Gym
- Agents trained in Gym can be used in Vibe, but Vibe doesn't run TB tasks

### Relationship to Commander

**Decision:** Vibe is a sibling screen in the same desktop app.

- Commander = StarCraft-style multi-agent control
- Vibe = Single-focus product building
- Both are tabs/screens in OpenAgents desktop app
- Commander may use Vibe components (editor, preview) in the future
- They share the same Nostr identity and local state

---

## Open Questions

### Q1: How do we handle Rust backend compilation?

The Rust backend needs to be compiled to WASM. Options:
- **Local cargo:** Run `cargo build --target wasm32` on user's machine
- **Spawned process:** Vibe spawns cargo as subprocess
- **Pre-compiled templates:** Only allow pre-built backends initially

Likely: Local cargo for native, since user has Rust toolchain. Browser version will need remote build service.

### Q2: What bundler for frontend?

Options:
- **Bun:** Already in our stack, fast
- **esbuild:** Fast, simple
- **Vite:** Popular, good DX
- **Custom:** Build our own on OANIX

Likely: Bun for native (matches CLAUDE.md guidance), custom WASM bundler for browser later.

### Q3: How does preview work?

Options:
- **WebView:** Embed browser view in GPUI
- **External browser:** Open localhost:3000 in Safari/Chrome
- **Native rendering:** Render HTML directly (limited)

Likely: WebView for tight integration, with fallback to external browser.

### Q4: Component sharing with Commander

Commander might want to use Vibe's editor or preview components. Questions:
- How do we structure crates for sharing?
- Does Commander embed Vibe, or do they share lower-level components?
- What's the right abstraction boundary?

To be determined as both products evolve.

---

## References

- [WANIX](https://github.com/tractordev/wanix) - WebAssembly runtime inspired by Plan 9
- [Plan 9 from Bell Labs](https://9p.io/plan9/)
- [Zed](https://zed.dev/) - High-performance editor
- [SWC](https://swc.rs/) - Fast Rust-based web compiler
