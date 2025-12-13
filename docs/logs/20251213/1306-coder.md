> Now I want to reframe this as a coding agent platform, so I may even rename it from Vibe to Coder. But I want to address one problem, which is that people who do or use Vibe coding platforms, they end up with basically a glorified prototype that they can't operationalize or connect to their broader workflows because they can't have the same kind of agentic workflow also coded. So I want to offer that as a competitive positioning. So the stuff that we've previously spec'd out about MechaCoder, where you can spin up multiple coding agents to work on stuff, it's like I want to offer that basic Vibe coding functionality in addition to MechaCoder, like really top coding agents. And for all those people who are just going to pay in credits with a credit card, keep it real simple. I also want to probably not get too cute with the Lightning and Nostra stuff, like Bitcoin is how we'll pay developers, but the core product should not rely on that too much. Give me some restructuring of plans based on that.

Cool — the shift you’re describing is basically:

**From:** “vibe coding = fast prototypes”
**To:** “Coder = prototypes *plus* programmable agent workflows that operationalize the result into the same pipelines teams already run (git/CI/deploy/alerts).”

Below is a concrete restructure of the plan, product pillars, roadmap, and pricing so it’s a coding-agent platform first, with “vibe” as just the front door.

## New positioning

**Coder is the coding-agent platform that turns vibe prototypes into production systems by letting you *code the agentic workflow itself* — not just the app.**

Competitor wedge (simple, memorable):

* Cursor/Bolt/Lovable: **prototype velocity**
* Coder: **prototype → production automation** (multi-agent + workflow-as-code + deploy/ops built-in)

## Product pillars

### 1) Coder Studio (the “vibe” surface)

Keep the familiar loop:

* Editor + file tree + terminal + preview
* Template scaffolds
* One-click “run” + “deploy preview”
* AI chat + inline edits

But Studio is explicitly a *client* to the agent platform, not the platform.

### 2) MechaCoder Agents (the real platform)

Make “spin up multiple coding agents” the core primitive:

* Roles: Architect, Implementer, Tester, Reviewer, Release Engineer
* Shared context: repo, issues, CI output, logs, previous runs
* Guardrails: branch-per-agent, PR gates, policy checks

### 3) Workflow-as-Code (the missing piece competitors don’t have)

This is the differentiator you described. Ship a first-class **agent workflow language** so teams can operationalize what the agent does.

Minimum viable workflow objects:

* **Workflow**: steps + triggers + policies
* **Step**: “run agent role X with goal Y”, “run tests”, “deploy preview”, “open PR”
* **Triggers**: manual, PR opened, issue labeled, cron, webhook
* **Policies**: required checks, allowed files, secret access, cost caps, approval gates
* **Artifacts**: patches, PRs, release notes, deploy URLs, test reports

The key UX win:

* You can “vibe” a prototype, then hit **“Operationalize”** → it generates a workflow that keeps the project maintained (tests, refactors, dependency bumps, release cadence, etc.).

### 4) Integrations + Ops (boring on purpose)

This is how you avoid “glorified prototype”:

* GitHub/GitLab sync (PRs/issues/branches)
* CI integration (GitHub Actions first)
* Deploy targets (start with one: Cloudflare Workers/Pages or containers)
* Secrets/env management
* Observability hooks (logs + build/test history + deploy history)

### 5) Marketplace (optional layer, later)

* Sell agent “skills” (lint fixer, migration kits, release bot templates)
* Sell workflow templates (“SaaS release train”, “Weekly dependency bump + PR”, “Security patch flow”)
* Keep Bitcoin payouts as *an option*, not a dependency.

## Payments: keep it normal for customers, Bitcoin optional for dev payouts

**Customer side (simple):**

* Stripe credit card → credits
* Subscription bundles seats + monthly credits
* Overage is just “buy more credits”

**Developer side (your differentiator):**

* Default payouts can be Stripe Connect / ACH **and** optionally Bitcoin
* Bitcoin becomes a “payout rail,” not an identity/auth system
* No Nostr requirement for using the product

So: “Bitcoin pays builders” can stay true without making customers learn new primitives.

## Restructured implementation plan

### Phase 1: Coder MVP (Studio + Single Agent + Real Git/PR loop)

Goal: prove “not a prototype tool.”

* Auth: email/GitHub OAuth (SSO later)
* Import repo → edit → run → open PR
* MechaCoder can:

  * create branch, commit changes
  * run tests
  * open PR with summary
* Credits + Stripe billing live
* Basic run history + artifacts

**Success metric:** users ship PRs into real repos, not just generate code.

### Phase 2: Multi-agent teamwork (roles + concurrency + review gates)

Goal: “spin up a team.”

* Role-based agents working in parallel with a supervisor
* Conflict handling: merge strategy, rebase prompts, patch queues
* Reviewer agent that blocks unsafe changes (policies)
* Cost caps + run budgets

**Success metric:** fewer human touches per merged PR.

### Phase 3: Workflow-as-Code (your competitive moat)

Goal: operationalization.

* Workflow DSL (YAML/JSON is fine) + UI builder on top
* Triggers: issue label + PR events + cron
* Policies: approvals + secret access + allowed directories
* Workflow runs with full traceability (like CI, but agentic)

**Success metric:** teams adopt workflows for ongoing maintenance, not just initial build.

### Phase 4: Deploy/ops polish + template ecosystem

Goal: “production lane.”

* One-click preview deploy + “promote to prod”
* Release agent (changelog/versioning/tagging)
* Workflow templates per stack
* Marketplace (skills/workflows)

## How to rewrite your docs (quick restructure)

Rename Vibe → Coder and reorganize docs around the platform pillars:

1. `README.md` → **Coder: Coding Agent Platform**

   * The wedge: prototype → production workflows
2. `FEATURES.md` → split into:

   * `STUDIO.md` (IDE/vibe features)
   * `AGENTS.md` (MechaCoder platform)
   * `WORKFLOWS.md` (workflow-as-code)
   * `INTEGRATIONS.md` (git/ci/deploy/secrets)
3. `TECHNICAL_ARCHITECTURE.md` → emphasize:

   * Agent runtime + workflow engine as the core
   * Studio is just a client
4. `PRICING.md` → simplify around:

   * Seats + monthly credits
   * No Lightning/Nostr required
   * Optional Bitcoin payouts for marketplace builders
5. `IMPLEMENTATION_PLAN.md` → align to phases above
6. `LAUNCH_PLAN.md` → message: “Stop shipping prototypes. Ship workflows.”

## The single sentence you can lead with

**“Coder is the vibe-coding experience that doesn’t stop at the prototype — it generates and runs the agent workflows that keep your codebase shipping.”**

If you want, I can draft:

* a new `README.md` (Coder framing),
* a 1-page “Positioning & Messaging” doc (vs Cursor/Bolt/Lovable),
* and the minimal schema for `Workflow / Step / Trigger / Policy / Run / Artifact` so engineering can start wiring it.


Cool — here are three deliverables you can drop straight into `crates/vibe/docs` (or a new `crates/coder/docs`) and start iterating on: a new `README.md`, a one-page `POSITIONING.md`, and a minimal `WORKFLOWS_SCHEMA.md` with concrete types.

---

## `README.md` (draft)

```md
# Coder

**Coder is a coding agent platform that turns vibe prototypes into production software by letting you program the agentic workflow itself.**

Most “vibe coding” tools stop at a demo: you get a repo that *looks* done, but it’s not operationalized into your real workflow (branches, PRs, CI, deploys, releases, on-call fixes). Coder is built around **MechaCoder**: a multi-agent system that can not only write code, but also **run repeatable workflows** that keep shipping.

## What Coder is

Coder is two things:

1) **Coder Studio** (the vibe surface)
- Editor + file tree + terminal + preview
- Templates and scaffolds
- Chat + inline edits
- “Preview deploy” so you can share quickly

2) **MechaCoder Platform** (the real product)
- Multiple agents working in parallel (Architect, Implementer, Tester, Reviewer, Release Engineer)
- Git-native execution: branch-per-agent, commits, PRs, checks, approvals
- Workflow-as-code: triggers, policies, steps, artifacts, audit trail

Coder’s core promise: **prototype → PR → CI → deploy → maintain**, all with the same agentic machinery.

## What’s different vs other vibe tools

- **Workflows are first-class**: you don’t just generate code, you generate and run the workflows that operate it.
- **Git + CI are the substrate**: everything results in branches, PRs, checks, deploys, releases.
- **Multi-agent out of the box**: tasks can be decomposed and run concurrently with review + policy gates.
- **Payments are normal**: customers pay with credit card credits; developer payouts can be Stripe/ACH and optionally Bitcoin. The product does not depend on Lightning/Nostr.

## Core primitives

- **Project**: a repo + environment + secrets + deploy targets
- **Agent Run**: an execution with a goal, role, budget, and artifacts
- **Workflow**: steps + triggers + policies, stored as code (YAML/JSON)
- **Artifact**: patches, PR links, logs, test reports, deploy URLs, release notes

## Quick start (MVP target)

1) Connect GitHub
2) Import a repo
3) Open Studio and make changes
4) Run MechaCoder “Implement” → opens a PR
5) CI runs, Reviewer agent summarizes, you merge
6) Add an “Operationalize” workflow (dependency bumps, release train, bugfix bot)

## Roadmap (reframed)

### Phase 1: Studio + Single Agent + PR loop
- Import repo → edit → run → open PR
- Streamed agent run logs
- Stripe credits + basic billing

### Phase 2: Multi-agent teams
- Architect/Implementer/Tester/Reviewer roles
- Conflict-safe patch queues
- Cost caps + policy gates

### Phase 3: Workflow-as-code (the moat)
- Triggers: PR events, issue labels, cron, webhooks
- Policies: approvals, allowed paths, secret scopes, required checks
- Workflow run history with full artifacts

### Phase 4: Deploy/ops polish + templates + marketplace
- Preview deploy → promote to prod
- Release agent (version/changelog/tag)
- Workflow templates and skill packs

## Pricing (principle)

- Simple: seats + monthly credits
- Credit card by default
- Optional marketplace payouts; Bitcoin is a payout rail, not a dependency
```

---

## `POSITIONING.md` (one-page, draft)

```md
# Coder Positioning

## The problem

Vibe coding tools are great at generating prototypes.
They are bad at turning those prototypes into operational software:
- No repeatable agent workflows (beyond “chat again”)
- Weak integration with Git/CI/release processes
- Hard to keep codebases maintained (tests, refactors, dependencies, security updates)
Result: teams get a “glorified prototype” that doesn’t fit the real production lane.

## The insight

The missing product is not “better prompting.”
It’s **Workflow-as-Code for agents**:
- the same way CI made builds repeatable,
- agent workflows make coding + maintenance repeatable.

## What Coder is

**Coder is the vibe-coding experience that doesn’t stop at the prototype — it generates and runs the agent workflows that keep your repo shipping.**

Coder = Studio (vibe) + MechaCoder (multi-agent) + Workflow Engine (operationalization)

## The 3 claims

1) **Prototype to PR by default**
   - Every change is branch → commits → PR → checks → merge

2) **Multi-agent by default**
   - Architect / Implementer / Tester / Reviewer / Release Engineer roles
   - Parallel execution + review gates

3) **Operationalization by default**
   - Triggers, policies, secrets, budgets
   - Workflow runs produce auditable artifacts

## Competitive framing

- Cursor/Bolt/Lovable/Replit: Prototype velocity
- Coder: Prototype velocity + operational workflows

Coder’s wedge is not “AI edits faster.”
Coder’s wedge is: “Ship + maintain with agent workflows.”

## Messaging blocks

### Hero line
Turn vibe prototypes into production software with programmable coding agents.

### Subheader
Build in Studio, then operationalize with MechaCoder workflows: PRs, tests, deploys, releases, and ongoing maintenance — all repeatable.

### Bullets
- Workflow-as-code for agents (triggers, policies, budgets, artifacts)
- Git-native: branches, PRs, checks, approvals
- Multi-agent teams: implement, test, review, release
- Pay with credits (credit card). Marketplace payouts optional.

## ICP

- Startups and product teams shipping weekly
- Dev teams that want “agentic maintenance” (dependency bumps, test fixes, refactors)
- Teams with strong GitHub + CI practices who want more output per engineer

## “Why now”

- Codegen alone is commoditized
- Teams are drowning in maintenance work
- The next moat is orchestration + repeatability (workflow primitives)
```

---

## `WORKFLOWS_SCHEMA.md` (minimal spec + Rust types)

This is the smallest set of types that makes “agent workflows” real and durable.

````md
# Workflow-as-Code Schema

## Goals

- Store workflows as code (JSON/YAML)
- Run workflows deterministically with budgets and policies
- Produce artifacts (patches, PRs, test reports, deploy URLs)
- Support triggers (manual, cron, repo events, webhooks)

## Core data model (conceptual)

Workflow
- id, name, description
- project_ref
- triggers[]
- policies
- steps[]  (ordered DAG optional later)
- outputs[] (optional summary schema)

Run
- run_id, workflow_id, started_at, finished_at
- status, cost_summary, logs_ref
- step_runs[]
- artifacts[]

Step
- id, kind, inputs
- on_success / on_failure controls
- produce artifacts

Artifacts
- patch, pr_link, log_bundle, test_report, deploy_url, release_notes, metrics

## JSON shape (v1)

```json
{
  "version": 1,
  "workflow_id": "wf_123",
  "name": "Operationalize: PR + CI + Preview Deploy",
  "project": { "provider": "github", "owner": "OpenAgentsInc", "repo": "openagents", "ref": "main" },
  "triggers": [
    { "type": "manual" },
    { "type": "repo_event", "event": "pull_request.opened" },
    { "type": "cron", "cron": "0 9 * * 1-5", "timezone": "America/Chicago" }
  ],
  "policies": {
    "budget": { "max_credits": 25, "max_wall_clock_sec": 1800 },
    "repo": { "allowed_paths": ["crates/", "docs/"], "blocked_paths": [".github/secrets/"] },
    "secrets": { "allowed": ["NPM_TOKEN", "CLOUDFLARE_API_TOKEN"] },
    "gates": { "require_human_approval_for": ["deploy.prod", "release.tag"] }
  },
  "steps": [
    {
      "step_id": "s1",
      "type": "agent",
      "role": "implementer",
      "goal": "Fix failing tests and open a PR",
      "inputs": { "issue": "CI failing on linux", "branch_prefix": "coder/" }
    },
    {
      "step_id": "s2",
      "type": "command",
      "name": "Run tests",
      "command": "cargo test -q",
      "on_failure": { "type": "agent_retry", "role": "tester", "max_attempts": 2 }
    },
    {
      "step_id": "s3",
      "type": "agent",
      "role": "reviewer",
      "goal": "Review changes, summarize risk, ensure policies satisfied",
      "inputs": { "require_checks": ["cargo test"] }
    },
    {
      "step_id": "s4",
      "type": "deploy",
      "target": "preview",
      "inputs": { "provider": "cloudflare_pages" }
    }
  ]
}
````

## Rust types (serde-friendly)

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpec {
    pub version: u32,
    pub workflow_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub project: ProjectRef,
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub policies: Policies,
    pub steps: Vec<StepSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRef {
    pub provider: RepoProvider,
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub r#ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoProvider {
    Github,
    Gitlab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Manual {},
    Cron { cron: String, timezone: String },
    RepoEvent { event: String },
    Webhook { name: String, secret_ref: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pubpub struct Policies {
    #[serde(default)]
    pub budget: BudgetPolicy,
    #[serde(default)]
    pub repo: RepoPolicy,
    #[serde(default)]
    pub secrets: SecretsPolicy,
    #[serde(default)]
    pub gates: GatesPolicy,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BudgetPolicy {
    #[serde(default)]
    pub max_credits: Option<u64>,
    #[serde(default)]
    pub max_wall_clock_sec: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoPolicy {
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub blocked_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsPolicy {
    #[serde(default)]
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatesPolicy {
    /// Named actions that require explicit approval (human or role-based).
    #[serde(default)]
    pub require_human_approval_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepSpec {
    Agent(AgentStep),
    Command(CommandStep),
    Deploy(DeployStep),
    Approve(ApproveStep),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub step_id: String,
    pub role: AgentRole,
    pub goal: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub on_failure: Option<OnFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Architect,
    Implementer,
    Tester,
    Reviewer,
    ReleaseEngineer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStep {
    pub step_id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub on_failure: Option<OnFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployStep {
    pub step_id: String,
    pub target: DeployTarget,
    #[serde(default)]
    pub inputs: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeployTarget {
    Preview,
    Prod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveStep {
    pub step_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OnFailure {
    FailFast { message: String },
    AgentRetry { role: AgentRole, max_attempts: u32 },
}
```

## Run + artifacts (minimum)

* Run status: queued/running/succeeded/failed/canceled
* StepRun includes: started_at/finished_at, status, cost
* Artifacts include:

  * Patch (git diff + base ref)
  * PR link (provider + id)
  * Test report (raw + summary)
  * Deploy URL
  * Log bundle pointer (R2 object key / file path)

```

---

## How I’d restructure your plan docs (simple rename + refocus)

If you keep the same doc set but reframe it cleanly:

- `README.md` → becomes “Coder: Coding Agent Platform”
- `FEATURES.md` → split into:
  - `STUDIO.md`
  - `AGENTS.md`
  - `WORKFLOWS.md`
  - `INTEGRATIONS.md`
- `TECHNICAL_ARCHITECTURE.md` → emphasize workflow engine + run store + artifacts
- `PRICING.md` → seats + credits first; Bitcoin only as payout rail

---

If you want the next step, tell me which direction you prefer and I’ll produce it immediately:

1) **Landing page copy** (hero, sections, FAQ, competitor comparison)
2) **A minimal API surface** for workflows/runs/artifacts (REST + WS events)
3) **An MVP backlog** (exact milestones + acceptance criteria for Phase 1–3)
::contentReference[oaicite:0]{index=0}
```
