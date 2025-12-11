# Vibe

**Build products at the speed of thought.**

Describe what you want. Get a working app. Ship it.

---

## What is Vibe?

Vibe is an AI-native development platform. You describe what you want to build in natural language, and an AI agent builds it—not mockups, not wireframes, but real, working, deployable applications.

```
"Build me a SaaS dashboard with user auth and Stripe subscriptions"
    ↓
[Agent builds in real-time, you watch the task feed]
    ↓
Working app with login, database, payments, polished UI
    ↓
One click → Live at myapp.vibe.dev
```

**Time from idea to deployed app: Minutes, not months.**

---

## Why Vibe?

### Instant Value
- **Under 2 minutes to first app** — No setup, no config, just describe and build
- **Real-time task feed** — Watch the agent work, feel momentum not mystery
- **Immediate preview** — Interactive app appears as agent builds

### Full-Stack Out of the Box
- **Database included** — PostgreSQL, automatically provisioned
- **Auth included** — Email, OAuth, magic links—just works
- **Payments included** — Subscriptions, one-time, usage-based
- **Hosting included** — One-click deploy to edge network

### Visual + Code Hybrid
- **Design Mode** — Click elements, adjust styles, no code needed
- **Code Mode** — Full editor when you want control
- **Agent Mode** — Let AI handle the heavy lifting

### Never Locked In
- **GitHub sync** — Two-way, real-time
- **Export anytime** — Download full codebase
- **Standard stack** — React, Tailwind, no proprietary magic

---

## OpenAgents Integration

Vibe is part of the OpenAgents ecosystem:

| Product | Purpose |
|---------|---------|
| **Commander** | Multi-agent orchestration, StarCraft-style control |
| **Gym** | Training & benchmarking agents |
| **Vibe** | Build products—websites, apps, SaaS |
| **Marketplace** | Discover and publish agents |
| **Wallet** | Bitcoin/Lightning payments |

### Unique to Vibe

- **Zero Friction** — No signup, no OAuth, just start building
- **Agent Transparency** — See every agent action, every file changed
- **Developer Revenue Share** — Earn when others use your templates
- **Desktop Speed + Browser Reach** — Native performance, zero-install sharing
- **Superior Architecture** — Plan 9-inspired, same code runs everywhere

---

## Architecture

### Native-First, Browser-Later

Vibe starts as part of the OpenAgents desktop app (GPUI), then extracts to browser via WASM. Same code, two targets.

```
Phase 1: Native Desktop          Phase 2: Browser
┌─────────────────────┐          ┌─────────────────────┐
│    Vibe IDE (GPUI)  │          │  Vibe IDE (WASM)    │
│         │           │          │         │           │
│    OANIX Kernel     │    →     │    OANIX Kernel     │
│         │           │          │         │           │
│  Real FS │ Wasmtime │          │ IndexedDB │ Browser │
└─────────────────────┘          └─────────────────────┘
```

### OANIX Namespace

Every project runs in an OANIX namespace (Plan 9-inspired):

```
/workspace     → Project files
/logs          → Build logs, ATIF trajectories
/cap/agents    → AI agent capabilities
/cap/net       → Network access
/cap/payments  → Bitcoin/Lightning
/db            → SQLite database
/preview       → Built assets
```

### Tech Stack

- **Frontend**: React/TypeScript (standard, no lock-in)
- **Backend**: Rust compiled to WASM (fast, secure)
- **IDE**: GPUI (GPU-accelerated, smooth as butter)
- **Runtime**: OANIX kernel (sandboxed, secure)
- **Database**: SQLite (simple, reliable)

---

## Documentation

| Document | Description |
|----------|-------------|
| [PRODUCT.md](docs/PRODUCT.md) | Full product specification, pricing, growth strategy |
| [FEATURES.md](docs/FEATURES.md) | Complete feature list with details |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Technical architecture, system design |

---

## Feature Highlights

### AI Agent System
- **Agent Mode**: Autonomous multi-step execution
- **Chat Mode**: Brainstorm without changing code
- **Task Feed**: Real-time visibility into agent work
- **Self-Healing**: Agent detects and fixes errors automatically

### Editor & IDE
- Full-featured code editor with IntelliSense
- File tree with Git status indicators
- Integrated terminal connected to OANIX
- ATIF trajectory viewer for agent transparency

### Design Mode
- Click any element to select and edit
- Theme system for global styling
- Spacing, typography, color controls
- Responsive preview (desktop, tablet, mobile)

### Backend & Database
- Visual database browser and SQL editor
- Schema management with migrations
- Auth dashboard with user management
- Edge functions for custom logic

### Deployment
- One-click publish to production
- Custom domains with automatic SSL
- Security scanning on every deploy
- Deployment history with rollback

### Collaboration
- Unlimited collaborators (even free tier)
- Real-time presence and sync
- Comment threads on code
- Role-based permissions

### Integrations
- Payments (Stripe-style)
- E-commerce
- Email
- Analytics
- Any REST API via agent

---

## Pricing

| Tier | Price | Credits | Key Features |
|------|-------|---------|--------------|
| **Free** | $0 | 5/day | Public projects, unlimited collaborators |
| **Pro** | $25/mo | 100 | Private projects, custom domains, GitHub sync |
| **Team** | $50/mo | 200 | SSO, roles/permissions, priority support |
| **Enterprise** | Custom | Unlimited | Dedicated support, on-premise option |

---

## Crate Structure

```
crates/vibe/
├── src/
│   ├── lib.rs          # Module exports
│   ├── error.rs        # Error types
│   ├── config.rs       # Configuration
│   ├── traits.rs       # Core traits (IdeFs, TerminalBackend, JobBackend)
│   ├── agent/          # AI agent system
│   ├── ide/            # Editor, file tree, terminal, preview
│   ├── design/         # Design mode, themes, properties
│   ├── backend/        # Rust backend runtime
│   ├── devrt/          # Frontend dev runtime (bundler, HMR)
│   ├── db/             # Database layer
│   ├── deploy/         # Deployment pipeline
│   ├── collab/         # Collaboration features
│   └── platform/       # Native/browser abstractions
└── docs/
    ├── PRODUCT.md
    ├── FEATURES.md
    └── ARCHITECTURE.md

crates/vibe-backend/     # Minimal Rust backend framework
├── src/
│   ├── lib.rs
│   ├── request.rs
│   ├── response.rs
│   └── router.rs
```

---

## Implementation Status

### Phase 1: Core IDE (Current)
- [ ] Vibe screen in Commander
- [ ] File tree and editor
- [ ] Terminal panel
- [ ] Basic preview

### Phase 2: Dev Runtime
- [ ] Bun integration for bundling
- [ ] Live preview in WebView
- [ ] File watcher and rebuild

### Phase 3: Agent Integration
- [ ] Agent panel in IDE
- [ ] Scaffolding commands
- [ ] ATIF trajectory viewer

### Phase 4: Full-Stack
- [ ] Rust backend templates
- [ ] Database dashboard
- [ ] Auth system

### Phase 5: Browser Extraction
- [ ] GPUI to WASM compilation
- [ ] IndexedDB storage
- [ ] Browser bundler

---

## References

- [OANIX](../oanix/README.md) — Plan 9-inspired agent OS
- [Plan 9 from Bell Labs](https://9p.io/plan9/)
- [WANIX](https://github.com/tractordev/wanix) — WebAssembly OS inspiration

---

**Build something Vibe.**
