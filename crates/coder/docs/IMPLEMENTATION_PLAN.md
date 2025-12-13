# Coder Implementation Plan

Execution plan for the Coder coding agent platform.

---

## Guiding Principles

- Ship native + web + edge together; Coder Web (WASM) is the public entrypoint
- Treat every project as an OANIX namespace
- Git-native: everything results in branches, PRs, checks, deploys
- Workflow-as-code is the competitive moat
- Stripe credits for customers; Bitcoin payouts optional for builders

---

## Phase 1: Studio + Single Agent + PR Loop

**Goal:** Prove "not a prototype tool" — users ship PRs into real repos.

### Deliverables

- [ ] **Auth**: Email/GitHub OAuth (SSO later)
- [ ] **Import repo** → edit → run → open PR flow
- [ ] **MechaCoder single agent**:
  - Create branch, commit changes
  - Run tests
  - Open PR with summary
- [ ] **Streamed agent run logs**
- [ ] **Credits + Stripe billing live**
- [ ] **Basic run history + artifacts**

### Success Metric
Users ship PRs into real repos, not just generate code.

---

## Phase 2: Multi-Agent Teams

**Goal:** "Spin up a team" — multiple agents working in parallel.

### Deliverables

- [ ] **Role-based agents** working in parallel with a supervisor:
  - Architect
  - Implementer
  - Tester
  - Reviewer
- [ ] **Conflict handling**: merge strategy, rebase prompts, patch queues
- [ ] **Reviewer agent** that blocks unsafe changes (policies)
- [ ] **Cost caps + run budgets**

### Success Metric
Fewer human touches per merged PR.

---

## Phase 3: Workflow-as-Code

**Goal:** Operationalization — the competitive moat.

### Deliverables

- [ ] **Workflow DSL** (YAML/JSON) + UI builder on top
- [ ] **Triggers**: issue label + PR events + cron + webhooks
- [ ] **Policies**: approvals + secret access + allowed directories + required checks
- [ ] **Workflow runs** with full traceability (like CI, but agentic)
- [ ] **Artifacts**: patches, PR links, test reports, deploy URLs, release notes

### Success Metric
Teams adopt workflows for ongoing maintenance, not just initial build.

---

## Phase 4: Deploy/Ops Polish + Templates + Marketplace

**Goal:** "Production lane" — full operationalization.

### Deliverables

- [ ] **One-click preview deploy** + "promote to prod"
- [ ] **Release agent** (changelog/versioning/tagging)
- [ ] **Workflow templates per stack**:
  - "SaaS release train"
  - "Weekly dependency bump + PR"
  - "Security patch flow"
- [ ] **Marketplace**:
  - Agent "skills" (lint fixer, migration kits, release bot templates)
  - Workflow templates
  - Bitcoin payouts as optional payout rail

---

## Workstreams

| Workstream | Scope |
|------------|-------|
| **Web/Desktop Experience** | Dioxus app, router, editor, preview, auth UX |
| **Runtime & OANIX** | Namespace mounts, scheduler, WASI execution, logs |
| **AI & Agents** | MechaCoder chat + tools, agent orchestration, completions |
| **Workflows** | Workflow engine, triggers, policies, run store, artifacts |
| **Integrations** | Git sync, PR creation, CI integration, deploy targets |
| **Infra Resale** | Provisioning, metering, billing, customer dashboard |
| **Payments** | Stripe credits, optional Bitcoin payouts |
| **Marketplace** | Listings, payments/payouts, install to project |

---

## Immediate Backlog

- [ ] Rename complete: vibe → coder (crate, docs, UI)
- [ ] Wire deploy/log buttons to snapshot updates
- [ ] Define client auth state + placeholders for auth flows
- [ ] CI: ensure wasm32 target build command works
- [ ] Workflow schema types in `crates/coder/src/workflow/`

---

## Success Criteria (Short-term)

- Browser demo shows: projects, editor + agent feed, deploy/log actions, workflow list
- `cargo build` succeeds with coder crate
- Clear mapping from docs → UI/DTOs → endpoints

---

*Last Updated: December 2025*
