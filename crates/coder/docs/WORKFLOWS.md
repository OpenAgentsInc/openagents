# Workflow-as-Code

Workflow-as-code is Coder's competitive moat. While other tools just generate code, Coder lets you define, execute, and track repeatable agent workflows that operationalize your codebase.

---

## The Problem

Vibe coding tools are great at generating prototypes. They are bad at turning those prototypes into operational software:
- No repeatable agent workflows (beyond "chat again")
- Weak integration with Git/CI/release processes
- Hard to keep codebases maintained (tests, refactors, dependencies, security updates)

Result: teams get a "glorified prototype" that doesn't fit the real production lane.

---

## The Solution

**Workflow-as-Code for agents** — the same way CI made builds repeatable, agent workflows make coding + maintenance repeatable.

---

## Core Primitives

### Workflow
A workflow is a sequence of steps with triggers and policies, stored as code (YAML/JSON).

```yaml
version: 1
workflow_id: "wf_release_train"
name: "Weekly Release Train"
project:
  provider: github
  owner: OpenAgentsInc
  repo: openagents
  ref: main
triggers:
  - type: cron
    cron: "0 9 * * 1"  # Every Monday 9am
    timezone: "America/Chicago"
  - type: manual
policies:
  budget:
    max_credits: 50
    max_wall_clock_sec: 3600
  repo:
    allowed_paths: ["crates/", "docs/"]
    blocked_paths: [".github/secrets/"]
  gates:
    require_human_approval_for: ["deploy.prod"]
steps:
  - step_id: "s1"
    type: agent
    role: implementer
    goal: "Update dependencies and fix any breaking changes"
  - step_id: "s2"
    type: command
    name: "Run tests"
    command: "cargo test"
  - step_id: "s3"
    type: agent
    role: reviewer
    goal: "Review changes and summarize risks"
  - step_id: "s4"
    type: deploy
    target: preview
```

### Triggers

| Type | Description |
|------|-------------|
| `manual` | Triggered by user action |
| `cron` | Scheduled execution |
| `repo_event` | GitHub/GitLab events (PR opened, issue labeled) |
| `webhook` | External webhook call |

### Policies

| Policy | Purpose |
|--------|---------|
| `budget` | Credit limits, wall-clock time limits |
| `repo` | Allowed/blocked file paths |
| `secrets` | Which secrets the workflow can access |
| `gates` | Actions requiring human approval |

### Steps

| Type | Description |
|------|-------------|
| `agent` | Run an agent with a role and goal |
| `command` | Execute a shell command |
| `deploy` | Deploy to preview or production |
| `approve` | Wait for human approval |

### Artifacts

Workflows produce artifacts:
- **Patch**: git diff + base ref
- **PR Link**: provider + PR ID + URL
- **Test Report**: passed/failed/skipped counts
- **Deploy URL**: preview or production URL
- **Log Bundle**: full execution logs
- **Release Notes**: version + changelog

---

## Rust Types

The workflow schema is defined in `crates/coder/src/workflow/schema.rs`:

```rust
pub struct WorkflowSpec {
    pub version: u32,
    pub workflow_id: String,
    pub name: String,
    pub project: ProjectRef,
    pub triggers: Vec<Trigger>,
    pub policies: Policies,
    pub steps: Vec<StepSpec>,
}

pub enum Trigger {
    Manual {},
    Cron { cron: String, timezone: String },
    RepoEvent { event: String },
    Webhook { name: String, secret_ref: String },
}

pub enum StepSpec {
    Agent(AgentStep),
    Command(CommandStep),
    Deploy(DeployStep),
    Approve(ApproveStep),
}

pub enum AgentRole {
    Architect,
    Implementer,
    Tester,
    Reviewer,
    ReleaseEngineer,
}
```

---

## Example Workflows

### Dependency Bump Bot
```yaml
name: "Weekly Dependency Bump"
triggers:
  - type: cron
    cron: "0 9 * * 1"
steps:
  - type: agent
    role: implementer
    goal: "Update all dependencies to latest compatible versions"
  - type: command
    command: "cargo test"
  - type: agent
    role: reviewer
    goal: "Review dependency changes for security issues"
```

### PR Review Bot
```yaml
name: "Auto-Review PRs"
triggers:
  - type: repo_event
    event: "pull_request.opened"
steps:
  - type: agent
    role: reviewer
    goal: "Review the PR, check for security issues, suggest improvements"
```

### Release Train
```yaml
name: "Weekly Release"
triggers:
  - type: cron
    cron: "0 14 * * 5"  # Friday 2pm
steps:
  - type: agent
    role: release_engineer
    goal: "Prepare release notes and version bump"
  - type: command
    command: "cargo build --release"
  - type: deploy
    target: preview
  - type: approve
    reason: "Approve production deploy"
  - type: deploy
    target: prod
```

---

## The UX Win

You can "vibe" a prototype, then hit **"Operationalize"** → Coder generates a workflow that keeps the project maintained (tests, refactors, dependency bumps, release cadence, etc.).

---

*Last Updated: December 2025*
