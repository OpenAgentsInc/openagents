# Coder

**Coder is a coding agent platform that turns vibe prototypes into production software by letting you program the agentic workflow itself.**

Most "vibe coding" tools stop at a demo: you get a repo that *looks* done, but it's not operationalized into your real workflow (branches, PRs, CI, deploys, releases, on-call fixes). Coder is built around **MechaCoder**: a multi-agent system that can not only write code, but also **run repeatable workflows** that keep shipping.

---

## What Coder Is

Coder is two things:

### 1. Coder Studio (the vibe surface)
- Editor + file tree + terminal + preview
- Templates and scaffolds
- Chat + inline edits
- "Preview deploy" so you can share quickly

### 2. MechaCoder Platform (the real product)
- Multiple agents working in parallel (Architect, Implementer, Tester, Reviewer, Release Engineer)
- Git-native execution: branch-per-agent, commits, PRs, checks, approvals
- Workflow-as-code: triggers, policies, steps, artifacts, audit trail

**Core promise:** prototype → PR → CI → deploy → maintain, all with the same agentic machinery.

---

## What's Different vs Other Vibe Tools

| Feature | Competitors | Coder |
|---------|-------------|-------|
| **Workflows** | Generate code only | Generate and run the workflows that operate it |
| **Git/CI** | Weak integration | Everything results in branches, PRs, checks, deploys |
| **Multi-agent** | Single chat | Tasks decomposed and run concurrently with review gates |
| **Payments** | Vendor-specific | Credit card credits; optional Bitcoin payouts |

---

## Core Primitives

- **Project**: a repo + environment + secrets + deploy targets
- **Agent Run**: an execution with a goal, role, budget, and artifacts
- **Workflow**: steps + triggers + policies, stored as code (YAML/JSON)
- **Artifact**: patches, PR links, logs, test reports, deploy URLs, release notes

---

## Quick Start (MVP Target)

1. Connect GitHub
2. Import a repo
3. Open Studio and make changes
4. Run MechaCoder "Implement" → opens a PR
5. CI runs, Reviewer agent summarizes, you merge
6. Add an "Operationalize" workflow (dependency bumps, release train, bugfix bot)

---

## Roadmap

### Phase 1: Studio + Single Agent + PR Loop
- Import repo → edit → run → open PR
- Streamed agent run logs
- Stripe credits + basic billing

### Phase 2: Multi-Agent Teams
- Architect/Implementer/Tester/Reviewer roles
- Conflict-safe patch queues
- Cost caps + policy gates

### Phase 3: Workflow-as-Code (The Moat)
- Triggers: PR events, issue labels, cron, webhooks
- Policies: approvals, allowed paths, secret scopes, required checks
- Workflow run history with full artifacts

### Phase 4: Deploy/Ops Polish + Templates + Marketplace
- Preview deploy → promote to prod
- Release agent (version/changelog/tag)
- Workflow templates and skill packs

---

## Technical Stack

```
┌────────────────────────────────────────┐
│         USER INTERFACES                 │
│  Desktop (Dioxus) │ Web (WASM) │ API   │
├────────────────────────────────────────┤
│         APPLICATION LAYER               │
│  MechaCoder │ Projects │ Marketplace   │
├────────────────────────────────────────┤
│         RUNTIME (OANIX)                 │
│  Namespace │ Scheduler │ WASI          │
├────────────────────────────────────────┤
│         CLOUDFLARE EDGE                 │
│  Workers │ DOs │ R2 │ D1 │ AI          │
├────────────────────────────────────────┤
│         IDENTITY & PAYMENTS             │
│  Nostr │ Stripe │ Bitcoin (optional)   │
└────────────────────────────────────────┘
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [STUDIO.md](./STUDIO.md) | Editor and IDE features |
| [AGENTS.md](./AGENTS.md) | MechaCoder agent system |
| [WORKFLOWS.md](./WORKFLOWS.md) | Workflow-as-code specification |
| [INTEGRATIONS.md](./INTEGRATIONS.md) | Git, CI, deploy, secrets |
| [PRICING.md](./PRICING.md) | Pricing and billing |
| [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) | Execution roadmap |
| [TECHNICAL_ARCHITECTURE.md](./TECHNICAL_ARCHITECTURE.md) | System design |

---

## What We've Built

| Component | Location | Status |
|-----------|----------|--------|
| **OANIX** | `crates/oanix/` | 180+ tests, production-ready |
| **Cloudflare Relay** | `crates/cloudflare/` | Live, NIP-01/NIP-90 |
| **Coder UI** | `crates/coder/` | Dioxus integration |
| **MechaCoder** | `crates/mechacoder/` | Claude streaming working |
| **Dioxus App** | `crates/dioxus/` | Web UI functional |

---

## Getting Started

```bash
# Clone the repo
git clone https://github.com/openagents/openagents.git
cd openagents

# Run the Dioxus web app
cd crates/dioxus
dx serve

# Run Cloudflare Workers locally
cd crates/cloudflare
wrangler dev
```

### Key Files

| File | Purpose |
|------|---------|
| `crates/coder/src/lib.rs` | Coder crate entry |
| `crates/coder/src/screen.rs` | Main Coder UI |
| `crates/oanix/src/lib.rs` | OANIX runtime |
| `crates/cloudflare/src/lib.rs` | Worker entry |
| `crates/dioxus/src/main.rs` | Web app entry |

---

## Pricing (Principle)

- Simple: seats + monthly credits
- Credit card by default
- Optional marketplace payouts; Bitcoin is a payout rail, not a dependency

---

## Contact

For questions about Coder, reach out to the OpenAgents team.

---

*Last Updated: December 2025*
