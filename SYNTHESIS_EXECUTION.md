# OpenAgents: Execution Guide

Practical guide to the current implementation. For the full vision, see [SYNTHESIS.md](./SYNTHESIS.md).

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OPENAGENTS STACK (Current)                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PRODUCTS                                                                    │
│  ┌───────────────────┐  ┌───────────────────┐  ┌────────────────────────┐   │
│  │     Autopilot     │  │       Onyx        │  │   openagents.com       │   │
│  │  (coding agent)   │  │ (markdown editor) │  │   (web dashboard)      │   │
│  └─────────┬─────────┘  └─────────┬─────────┘  └───────────┬────────────┘   │
│            │                      │                        │                 │
│            └──────────────────────┴────────────────────────┘                 │
│                                   │                                          │
│  RUNTIME                          │                                          │
│  ┌────────────────────────────────┴─────────────────────────────────────┐   │
│  │  crates/runtime - Agent execution environment                         │   │
│  │  Tick model │ Filesystem abstraction │ /compute │ /containers │ /claude│   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                   │                                          │
│  INFRASTRUCTURE                   │                                          │
│  ┌─────────────────┐  ┌───────────┴───────────┐  ┌────────────────────────┐ │
│  │      Pylon      │  │        Nexus          │  │     WGPUI              │ │
│  │  (local node)   │  │  (Nostr relay)        │  │  (GPU UI)              │ │
│  │  Provider/Host  │  │  NIP-90 job market    │  │  wgpu rendering        │ │
│  └────────┬────────┘  └───────────┬───────────┘  └────────────────────────┘ │
│           │                       │                                          │
│  PROTOCOLS│                       │                                          │
│  ┌────────┴───────────────────────┴─────────────────────────────────────┐   │
│  │  NIP-90 (compute jobs) │ NIP-42 (auth) │ NIP-89 (handlers)           │   │
│  │  Spark/Lightning (payments) │ Nostr (transport)                      │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Pylon

**What it is:** Single binary that runs on your device. Two modes, can run simultaneously.

| Mode | Purpose | How it works |
|------|---------|--------------|
| **Provider** | Earn Bitcoin by selling compute | Listens for NIP-90 jobs, runs inference, gets paid |
| **Host** | Run your own agents | Manages agent lifecycle, wallets, tick scheduling |

**Key paths:**
- `crates/pylon/src/cli/` — CLI commands (init, start, stop, status, doctor)
- `crates/pylon/src/provider.rs` — NIP-90 job processing
- `crates/pylon/src/host/` — Agent subprocess management
- `crates/pylon/src/daemon/` — Background process lifecycle

**Data directory:** `~/.openagents/pylon/`
- `config.toml` — Configuration
- `identity.mnemonic` — BIP-39 seed (chmod 600!)
- `pylon.db` — SQLite (jobs, earnings, agents)
- `control.sock` — IPC socket

**Build and run:**
```bash
cargo build --release -p pylon
./target/release/pylon init
./target/release/pylon start -f -m provider  # Foreground, provider mode
```

**Inference backends (auto-detected):**
- Apple Foundation Models (macOS + Apple Silicon)
- Ollama (any platform, port 11434)
- llama.cpp (any platform, port 8080)

---

## Nexus

**What it is:** Nostr relay optimized for agent job coordination. Runs on Cloudflare Workers.

**Key NIPs supported:**
- NIP-90: Data Vending Machines (job requests/results)
- NIP-89: Handler discovery
- NIP-42: Authentication

**Event flow:**
```
Buyer → kind:5050 (job request) → Nexus → Provider
Provider → kind:7000 (invoice) → Nexus → Buyer
[Buyer pays Lightning invoice]
Provider → kind:6050 (result) → Nexus → Buyer
```

**Key paths:**
- `crates/nexus/worker/` — Cloudflare Worker implementation
- `crates/nexus/docs/MVP.md` — Protocol spec

**Deploy:**
```bash
cd crates/nexus/worker
bun install
bun run deploy
```

**Live instance:** `wss://nexus.openagents.com`

---

## Runtime

**What it is:** Pluggable execution environment for agents. Plan 9-inspired filesystem abstraction.

**The tick model:**
```
WAKE → LOAD → PERCEIVE → THINK → ACT → REMEMBER → SCHEDULE → SLEEP
```

Works across: Browser (WASM), Cloudflare (DO), Local (SQLite), Server (Docker/K8s).

**Agent filesystem (what agents see):**
```
/                           # Agent's root
├── ctl                     # control: tick, hibernate, wake
├── status                  # agent state
├── inbox/                  # incoming messages
├── outbox/                 # emitted events
├── goals/                  # active goals
├── memory/                 # conversations, patterns
├── identity/               # pubkey, sign, verify, encrypt, decrypt
├── wallet/                 # balance, pay
├── compute/                # LLM inference jobs
│   ├── providers/          # available backends
│   ├── new                 # submit job
│   └── jobs/<id>/          # status, result, stream
├── containers/             # sandboxed code execution
├── claude/                 # Claude Agent SDK sessions
└── hud/                    # streaming events for UI
```

**Key paths:**
- `crates/runtime/src/agent.rs` — Agent trait
- `crates/runtime/src/tick.rs` — Tick execution
- `crates/runtime/src/compute.rs` — /compute implementation
- `crates/runtime/src/containers.rs` — /containers implementation
- `crates/runtime/src/claude.rs` — /claude implementation
- `crates/runtime/src/services/` — Filesystem services (hud, wallet, logs, etc.)

**Mount points:**
| Mount | Purpose | Stateful |
|-------|---------|----------|
| `/compute` | Stateless inference (LLM calls) | No |
| `/containers` | Sandboxed code execution | Session |
| `/claude` | Claude Agent SDK sessions with tool use | Yes |

---

## Autopilot

**What it is:** The product. An autonomous coding agent that uses Claude SDK.

**Two modes:**
| Mode | Command | Cost | Where it runs |
|------|---------|------|---------------|
| Tunnel (free) | `pylon connect` | Free | Your machine |
| Container (paid) | Web UI | Credits | Cloudflare edge |

**Key paths:**
- `crates/autopilot/src/` — Core logic (preflight, runner, Claude SDK integration)
- `crates/autopilot-service/` — Background daemon
- `crates/autopilot-container/` — HTTP wrapper for Cloudflare Containers
- `crates/autopilot-shell/` — Interactive shell
- `crates/claude-agent-sdk/` — Rust SDK for Claude Code CLI

**How it connects:**

```
Autopilot ─────► Runtime ─────► Pylon ─────► Nexus
   │                │              │            │
   │                │              │            └── Nostr relay
   │                │              └── Local compute / provider
   │                └── /claude sessions, /compute calls
   └── Claude SDK queries, tool execution
```

**Run:**
```bash
cargo autopilot run "Fix the failing tests"
```

---

## WGPUI

**What it is:** GPU-accelerated UI rendering library. WebGPU/Vulkan/Metal/DX12 via wgpu.

**Why:** HTML hits limits for performance-critical surfaces:
- Streaming markdown at 100+ tokens/sec
- Virtual scrolling 10k+ messages
- Real-time syntax highlighting

**Key paths:**
- `crates/wgpui/src/renderer.rs` — wgpu pipelines
- `crates/wgpui/src/text.rs` — cosmic-text integration
- `crates/wgpui/src/layout.rs` — Taffy (CSS Flexbox)
- `crates/wgpui/src/markdown/` — Streaming markdown
- `crates/wgpui/src/components/` — Atomic design (atoms → molecules → organisms)

**Design constraints:**
- Sharp corners only (no border-radius)
- Tailwind-aligned tokens
- Vera Mono font only

**Build:**
```bash
cargo build -p wgpui                                    # Web (default)
cargo build -p wgpui --features desktop --no-default-features  # Desktop
cargo build -p wgpui --target wasm32-unknown-unknown    # WASM
```

---

## Key Crates

| Crate | Purpose |
|-------|---------|
| `pylon` | Node software (provider + host) |
| `nexus` | Nostr relay for job market |
| `runtime` | Agent execution environment |
| `autopilot` | Coding agent product |
| `wgpui` | GPU-rendered UI |
| `spark` | Lightning wallet (Breez SDK) |
| `compute` | NIP-90 DVM primitives |
| `claude-agent-sdk` | Rust SDK for Claude Code |
| `frostr` | FROST threshold signatures |

---

## For Coding Agents

### Git Rules

```
NEVER: push --force to main, git stash, destructive commands without asking
ALWAYS: Commit working code every 15-30 minutes, small frequent commits
```

Stage only your own files. Other agents may have uncommitted work.

### Commit Format

```bash
git commit -m "$(cat <<'EOF'
Short description of change

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
Co-Authored-By: Autopilot <autopilot@openagents.com>
EOF
)"
```

### Build Commands

```bash
# Pylon
cargo build --release -p pylon

# Nexus (Cloudflare Worker)
cd crates/nexus/worker && bun run deploy

# Autopilot
cargo autopilot run "your prompt"

# WGPUI tests
cargo test -p wgpui

# Full workspace
cargo build --workspace
```

### Database Access

**NEVER raw sqlite3 for writes.** Use APIs:
```bash
cargo autopilot issue create
cargo autopilot issue claim
cargo autopilot issue complete
```

Read-only queries OK for debugging.

### Nostr

NIP specs are local at `~/code/nips/`. Read from there, don't web search.

### Completion Standards

Issues are NOT done unless:
1. No stubs, mocks, TODOs, NotImplemented
2. Code actually works (tested)
3. SDK integrations are real, not stubbed

---

## Data Flow: End-to-End

**User runs Autopilot locally:**
```
1. User: `cargo autopilot run "Fix tests"`
2. Autopilot: Preflight checks (config, auth, repo)
3. Autopilot: Creates Claude SDK session via Runtime /claude
4. Runtime: Routes to local Claude tunnel or cloud API
5. Claude: Reads files, makes edits, runs tests
6. Autopilot: Streams results to terminal/HUD
```

**Autopilot needs inference from swarm:**
```
1. Autopilot: Writes job to Runtime /compute/new
2. Runtime: Publishes NIP-90 kind:5050 to Nexus
3. Nexus: Broadcasts to subscribed Pylons
4. Pylon (provider): Picks up job, runs inference
5. Pylon: Publishes kind:7000 (invoice), waits for payment
6. Autopilot: Pays Lightning invoice via /wallet/pay
7. Pylon: Publishes kind:6050 (result)
8. Runtime: Receives result, returns to Autopilot
```

**Provider earns Bitcoin:**
```
1. Pylon: `pylon start -m provider`
2. Pylon: Connects to Nexus, subscribes to kind:5050
3. Pylon: Detects inference backends (Ollama, Apple FM, etc.)
4. Buyer: Submits job to Nexus
5. Pylon: Receives job, sends invoice
6. Buyer: Pays invoice
7. Pylon: Runs inference, publishes result
8. Pylon: Sats deposited to embedded Spark wallet
```

---

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| Pylon CLI | v0.1 | Provider mode working, host mode partial |
| Pylon Wallet | Working | Spark/Lightning, regtest + mainnet |
| Nexus | v0.1 | NIP-90, NIP-42, NIP-89 |
| Runtime | In progress | Tick engine, filesystem, /compute, /containers, /claude |
| Autopilot | Alpha | Claude SDK integration, tunnel mode |
| WGPUI | Phase 16 | 377 tests, full component library |
| RLM | Experimental | Recursive Language Model queries |

**Bitcoin network:** Default is `regtest` for testing. Mainnet available.

---

## Quick Reference

### Start Provider (earn sats)
```bash
pylon init
pylon start -f -m provider
```

### Run Autopilot
```bash
cargo autopilot run "Implement feature X"
```

### Deploy Nexus
```bash
cd crates/nexus/worker && bun run deploy
```

### Check Wallet
```bash
pylon wallet balance
pylon wallet fund  # regtest only
```

### Run Tests
```bash
cargo test -p pylon
cargo test -p runtime
cargo test -p wgpui
```
