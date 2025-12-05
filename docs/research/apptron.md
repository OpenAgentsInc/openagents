# Apptron + OpenAgents/MechaCoder: Research Notes

This document explores the intersection between Apptron (a browser-based Linux development platform) and OpenAgents/MechaCoder (an autonomous coding agent system). Despite being written in different languages (Go vs TypeScript), the philosophical alignment is striking.

## Executive Summary

Apptron and MechaCoder represent two complementary visions of the future of development:

| Aspect | Apptron | MechaCoder |
|--------|---------|------------|
| **Core Idea** | Linux workstation in browser | Autonomous coding agent |
| **Language** | Go + WASM | TypeScript + Effect |
| **Isolation** | v86 emulation + Wanix | Docker/Podman + git worktrees |
| **Persistence** | IndexedDB/OPFS + R2 | Git + .openagents/ |
| **Philosophy** | Local-first, cloud-free | Local-first, no cloud deps |

The key insight: **Apptron provides exactly the kind of sandboxed, reproducible execution environment that MechaCoder needs** - but without Docker dependencies.

---

## What is Apptron?

Apptron is a browser-based development platform that runs a full Linux environment (Alpine) inside a web tab. Key components:

### 1. v86 + Wanix Runtime
- **v86**: x86 emulator running in JavaScript/WASM
- **Wanix**: Custom runtime that wraps v86 with filesystem abstractions, DOM APIs, and native WASM binary support
- **Result**: Full 32-bit Linux running in browser with network access

```
Browser Tab
└── boot.go (WASM)
    └── Wanix Runtime
        └── v86 Emulator
            └── Alpine Linux (kernel + userspace)
                └── Your code + services
```

### 2. Virtual Networking
- DHCP-assigned session IPs on a virtual network
- Services bound to ports get public HTTPS URLs automatically (`tcp-8080-...apptron.dev`)
- Session IPs are routable to each other (cross-tab/device communication)
- Implemented via `progrium/go-netstack` + WebSocket tunneling

### 3. Filesystem Architecture
```
/
├── project/       (synced to R2, persisted)
├── home/$USER/    (synced to R2, persisted)
├── public/        (published static sites)
└── rest of /      (ephemeral unless in envbuild)
```
- IndexedDB/OPFS for local cache
- Cloudflare R2 for cloud sync
- `.apptron/envbuild` scripts define reproducible environments

### 4. VSCode Integration
- `wanix://` filesystem provider connects editor to guest
- Terminals stream via `#console/data`
- Preview panel for HTTP services

---

## MechaCoder Architecture Recap

MechaCoder is an autonomous coding agent with:

1. **Golden Loop v2**: orient → select → decompose → execute → verify → commit
2. **Two-tier agents**: Orchestrator (per-session) + Subagent (per-subtask)
3. **Sandbox options**: Docker, Podman, or host execution
4. **Task system**: `.openagents/tasks.jsonl` + progress tracking
5. **Crash recovery**: Checkpoints, two-phase commit safety

Current sandboxing limitations:
- Requires Docker/Podman installed locally
- Container startup adds latency (~1-2s per command)
- Not available on all platforms equally (Windows struggles)

---

## Synthesis: Potential Integrations

### 1. Browser-Native Sandbox for MechaCoder

**Concept**: Replace Docker with Apptron's v86 Linux for subagent execution.

```
Current Architecture:
  Orchestrator (host)
    └── Subagent (host or Docker)
        └── Tool calls (bash, edit, etc.)

Proposed Architecture:
  Orchestrator (host or browser)
    └── Subagent (Apptron v86)
        └── Tool calls (in isolated Linux)
```

**Benefits**:
- Zero Docker dependency
- Works on any platform with a browser
- True isolation (sandboxed emulator, not just containers)
- Reproducible via `.apptron/envbuild`

**Challenges**:
- v86 is slower than native execution (~10-50x for compute-heavy tasks)
- 32-bit limitation (v86 emulates x86, not x86_64)
- Memory constrained (~1GB default)

**When to use**:
- Untrusted code execution
- Educational/demo environments
- Cross-platform consistency
- When Docker is unavailable

### 2. Zero-Install Agent Execution

**Concept**: Run MechaCoder entirely in the browser, no local toolchain required.

```
User visits: https://mecha.openagents.com/project/my-repo
  └── Apptron boots with MechaCoder pre-installed
      └── Clones repo from GitHub
          └── Agent runs autonomously
              └── Commits pushed via virtual networking
```

**Use Cases**:
- "Try MechaCoder on your repo in 30 seconds"
- Mobile/tablet access to autonomous agents
- Shared debugging environments
- Training/onboarding

**Implementation Path**:
1. Create `.apptron/envbuild` that installs Bun + MechaCoder
2. Build custom Apptron environment with OpenAgents pre-baked
3. Expose MechaCoder orchestrator as a service
4. Use Apptron's port tunneling for external access

### 3. Portable Agent Environments (`.apptron/` meets `.openagents/`)

**Concept**: Define complete agent execution environments as Apptron projects.

```
my-project/
├── .openagents/
│   ├── project.json      # MechaCoder config
│   └── tasks.jsonl       # Work queue
├── .apptron/
│   ├── envbuild          # Agent runtime dependencies
│   └── envrc             # Agent environment variables
└── src/
    └── ...
```

**envbuild for a Go project**:
```bash
#!/bin/sh
source /etc/goprofile
apk add git make
go install github.com/openagents/mecha-cli@latest
```

**Benefits**:
- Version-controlled agent environments
- "Works on my Apptron" = "Works on any Apptron"
- Language-agnostic (just install what you need)
- Shareable via Apptron's publishing

### 4. Multi-Agent Browser Sessions

**Concept**: Leverage Apptron's virtual networking for parallel agent coordination.

Apptron's key insight: session IPs are routable across tabs and devices. This enables:

```
Tab 1: Agent A (task oa-1a2b)
  IP: 192.168.127.10

Tab 2: Agent B (task oa-3c4d)
  IP: 192.168.127.11

Tab 3: Orchestrator
  IP: 192.168.127.12
  Coordinates A and B via network calls
```

**vs. Current Worktree Approach**:
- Worktrees require git operations, merging complexity
- Apptron tabs share nothing except network
- Each tab is truly isolated (separate filesystem, kernel)
- Merge via git over SSH/HTTPS at the end

**Potential Protocol**:
```typescript
// Agent A publishes status
POST http://192.168.127.12:8080/agent/status
{ "taskId": "oa-1a2b", "phase": "verify", "result": "pass" }

// Orchestrator coordinates
GET http://192.168.127.10:8080/agent/result
{ "files_changed": ["src/foo.ts"], "commit": "abc123" }
```

### 5. Agent-Powered Apptron (Copilot Mode)

**Concept**: Embed MechaCoder as an Apptron extension.

```
User: "Add authentication to this app"
  └── Extension captures request
      └── Creates .openagents/task
          └── MechaCoder runs inside same Apptron instance
              └── Makes changes, runs tests
                  └── Presents diff for approval
```

**Implementation**:
1. Create `extension/agent` VSCode extension
2. Register `/agent/...` routes in Apptron worker
3. Stream MechaCoder events to extension panel
4. Bidirectional communication via `#console` or dedicated pipe

**Benefits**:
- AI assistant with full Linux access
- Can install packages, run servers, test end-to-end
- No "please run this command" - agent just does it
- Safe sandbox protects user's machine

### 6. Wasm-Native Tool Execution

**Concept**: Compile MechaCoder's tools (read, edit, bash) to WASM for native browser execution.

Apptron already supports this via `/bin/wexec`:
- Detects WASM binary type (WASI or Go-js)
- Allocates `/task/<pid>` entry
- Streams I/O via filesystem

**Potential Architecture**:
```
Tool Call: edit { path: "/src/foo.ts", ... }
  └── Compiled to WASM: tools.wasm
      └── Executed via wexec
          └── Native-speed string manipulation
              └── No v86 overhead for tool logic
```

**Hybrid Execution**:
- Heavy compute (parsing, diffing) → WASM tools
- System interaction (git, npm, test) → v86 Linux
- Best of both worlds

### 7. Federated Agent Network

**Concept**: Multiple users' Apptron sessions collaborating on a single codebase.

```
User A (SF):     Apptron → Agent A → Task oa-1
User B (NYC):    Apptron → Agent B → Task oa-2
User C (London): Apptron → Agent C → Task oa-3
                      ↓
              Shared Git Remote
                      ↓
              Merge Coordinator
```

**Apptron enables this because**:
- Sessions have public HTTPS endpoints
- No server infrastructure needed
- Each user provides their own compute
- Git is the synchronization mechanism

---

## Technical Deep Dives

### A. How Apptron's Filesystem Works

```go
// boot.go - simplified
inst := wanixruntime.Instance()
k := wanix.New()
k.AddModule("#web", web.New(k))
k.AddModule("#vm", vm.New())
k.AddModule("#ramfs", &ramfs.Allocator{})

// Mount bundle (Alpine rootfs)
bundleFS := tarfs.From(tar.NewReader(bundleBytes))
root.Namespace().Bind(bundleFS, ".", "#bundle")

// Mount persistent storage
opfs := idbfs.New("apptron-rev1")
root.Namespace().Bind(opfs, ".", "web/idbfs/apptron")

// Mount project (synced to R2)
localProjectFS := opfs.Sub("env/" + envUUID + "/project")
remoteProjectFS := httpfs.New(origin + "/data/env/" + envUUID + "/project")
sfs := syncfs.New(localProjectFS, remoteProjectFS, 5*time.Second)
root.Namespace().Bind(sfs, ".", "project")
```

Key abstractions:
- `tarfs`: Read-only filesystem from tar archive
- `memfs`: In-memory read-write filesystem
- `idbfs`: IndexedDB-backed persistence
- `httpfs`: HTTP-backed remote filesystem
- `syncfs`: Bidirectional sync between local and remote

**Lesson for OpenAgents**: Could adopt similar layered FS abstraction for tool isolation.

### B. Apptron's Networking Model

```go
// worker/cmd/worker/main.go - simplified
vn := netstack.New("192.168.127.0/24")
http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
    if isWebSocketUpgrade(r) {
        if strings.HasPrefix(r.Host, "tcp-") {
            // Proxy to session's internal port
            sessionIP, port := parsePortHost(r.Host)
            conn := vn.Dial(sessionIP, port)
            websocket.Relay(w, r, conn)
        } else {
            // Connect browser VM to virtual network
            vn.AcceptQemu(w, r)
        }
    }
})
```

**Key insight**: The virtual network is real enough for TCP/IP but isolated from host networking. Services "just work" with public URLs.

### C. Environment Customization

```bash
# .apptron/envbuild
#!/bin/sh
set -e
apk add --no-cache nodejs npm python3 make gcc
npm install -g bun
```

On boot, if `.apptron/envbuild` exists:
1. `rebuild` script runs in chroot
2. Output filesystem is snapshotted to IndexedDB
3. Future boots use cached snapshot
4. Only rebuilds when envbuild changes

**Parallel to OpenAgents**:
- `.openagents/init.sh` runs preflight checks
- Could adopt envbuild pattern for full environment setup

---

## Speculative Futures

### "Apptron for Agents" - A New Product

```
┌─────────────────────────────────────────────────────┐
│                   AgentTron                         │
│  "Autonomous Coding Agents in Your Browser"        │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌───────────────┐  ┌───────────────────────────┐  │
│  │   Task List   │  │     Agent Workspace       │  │
│  │   ─────────   │  │     ───────────────       │  │
│  │   oa-1 [done] │  │     [VSCode Editor]       │  │
│  │ > oa-2 [run]  │  │                           │  │
│  │   oa-3 [pend] │  │     [Terminal/Logs]       │  │
│  │   oa-4 [pend] │  │                           │  │
│  └───────────────┘  │     [Preview Panel]       │  │
│                     └───────────────────────────┘  │
│                                                     │
│  [Model: Claude] [Branch: feat/auth] [Tests: Pass] │
└─────────────────────────────────────────────────────┘
```

**Features**:
1. Import any GitHub repo
2. Define tasks in UI or import from issues
3. Watch agents work in real-time
4. Approve/reject changes
5. Push commits directly to GitHub

**Why Apptron base is ideal**:
- No server costs (runs in user's browser)
- No Docker setup (v86 emulation)
- Instant sharing (public URLs)
- Full Linux power (real git, npm, etc.)

### "MechaCoder Cloud" - Enterprise Offering

```
┌──────────────────────────────────────────────────────┐
│                    Architecture                      │
├──────────────────────────────────────────────────────┤
│                                                      │
│   Customer Browser              OpenAgents Cloud     │
│   ─────────────────             ────────────────     │
│   ┌─────────────┐               ┌──────────────┐    │
│   │   Apptron   │ ────API────→  │  Orchestrator │    │
│   │  Instance   │               │    Fleet      │    │
│   └─────────────┘               └──────────────┘    │
│         ↑                              │            │
│         │                              ↓            │
│         │                       ┌──────────────┐    │
│         └───── Push results ─── │   Git Proxy  │    │
│                                 └──────────────┘    │
│                                        │            │
│                                        ↓            │
│                                 ┌──────────────┐    │
│                                 │ Customer Git │    │
│                                 └──────────────┘    │
│                                                      │
└──────────────────────────────────────────────────────┘
```

**Model**:
- Customer runs Apptron (their compute, their data)
- OpenAgents provides orchestration intelligence
- Results flow directly to customer's git (never stored)
- Pay per task, not per compute

---

## Immediate Opportunities

### Short-term (Could build now)

1. **Apptron Environment for MechaCoder Development**
   - Create `.apptron/envbuild` that sets up Bun + Effect + dev dependencies
   - Developers can work on MechaCoder in browser without local setup
   - Perfect for contributors with non-standard setups

2. **MechaCoder-in-Browser Demo**
   - Fork Apptron, pre-install MechaCoder
   - Host at `demo.openagents.com`
   - Let users try agents on sample repos

3. **Cross-Pollinate Patterns**
   - Adopt Apptron's `syncfs` for `.openagents/` remote sync
   - Adopt Apptron's envbuild for init.sh replacement
   - Study Wanix's namespace/binding system for tool isolation

### Medium-term (Would require design)

4. **Apptron Sandbox Backend**
   - Implement `src/agent/orchestrator/sandbox-runner.ts` for Apptron
   - Detect if running in browser, use v86 instead of Docker
   - Graceful fallback chain: Docker → Podman → Apptron → host

5. **Apptron MCP Server**
   - Expose Apptron as an MCP server
   - Tools: `apptron_bash`, `apptron_read`, `apptron_write`
   - Claude Code could execute in isolated Apptron environment

### Long-term (Would be transformative)

6. **Unified Agent Platform**
   - Merge Apptron + OpenAgents into single product
   - Browser-native development + autonomous agents
   - "Your AI team in a tab"

---

## Open Questions

1. **Performance tradeoffs**: When is v86 emulation "fast enough"?
   - Editing files: Always fast enough
   - Running TypeScript: Probably fine
   - Running tests: Depends on test suite size
   - Compiling Go: Likely too slow

2. **Memory constraints**: 1GB default, can we fit larger codebases?
   - Could stream files on-demand (httpfs already does this)
   - Could mount external storage (WebUSB, S3 gateway)

3. **32-bit limitation**: What breaks without x86_64?
   - Node.js: 32-bit builds exist
   - Bun: Needs investigation
   - Go: Works fine
   - Python: Works fine

4. **Security model**: Is v86 truly isolated?
   - Emulator bugs could theoretically escape
   - But vastly smaller attack surface than Docker
   - Spectre/Meltdown mitigations apply differently

---

## Conclusion

Apptron and MechaCoder are natural complements:

- **Apptron provides**: Isolated execution, zero-install environments, reproducible builds, cross-platform support
- **MechaCoder provides**: Intelligent orchestration, task decomposition, verification pipelines, crash recovery

Together they could enable a new paradigm: **autonomous agents running in browser sandboxes**, accessible anywhere, controlled by users, and truly local-first.

The Go/TypeScript language barrier is surmountable via:
1. CLI interfaces (Apptron's `aptn` already exists)
2. Filesystem protocols (both systems already do this)
3. Network APIs (Apptron's port tunneling is perfect)

Next steps should focus on the **MechaCoder-in-Browser Demo** as proof of concept, then evaluate v86 performance for typical coding tasks.

---

*Research conducted: December 2024*
*Apptron repo: /Users/christopherdavid/code/apptron*
*OpenAgents repo: /Users/christopherdavid/code/openagents*
