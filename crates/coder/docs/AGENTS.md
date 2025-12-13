# MechaCoder Agents

MechaCoder is the multi-agent system at the heart of Coder. It coordinates multiple AI agents working in parallel to implement, test, review, and release code.

---

## Agent Roles

| Agent | Purpose | Autonomy |
|-------|---------|----------|
| **Architect** | Design system structure, plan implementation | High |
| **Implementer** | Write code, make changes | High |
| **Tester** | Generate/run tests | High |
| **Reviewer** | Code review, safety checks | Medium |
| **Release Engineer** | Handle deploys, releases | Low |

---

## Core Capabilities

| Feature | Status | Description |
|---------|--------|-------------|
| Streaming Responses | MVP | Real-time token display |
| Code Generation | MVP | Generate code from prompts |
| Code Explanation | MVP | Explain selected code |
| Refactoring | MVP | Improve code quality |
| Bug Fixing | MVP | Identify and fix issues |
| Multi-file Awareness | MVP | Context from all files |
| Tool Use | MVP | Execute actions (edit, create) |
| Conversation History | MVP | Persistent chat threads |

---

## Supported Actions

Agents can execute these actions:

```
- Create file
- Edit file
- Delete file
- Run command
- Search codebase
- Read documentation
- Deploy project
- Generate tests
- Create branch
- Open PR
- Run CI checks
```

---

## Agent Features

| Feature | Status | Description |
|---------|--------|-------------|
| Background Execution | MVP | Run while you work |
| Progress Tracking | MVP | See agent activity |
| Checkpointing | Phase 2 | Save/restore state |
| Multi-agent | Phase 2 | Parallel agent work |
| ATIF Logging | MVP | Full trajectory capture |
| Interrupt/Cancel | MVP | Stop agent mid-task |
| Approval Gates | Phase 2 | Review before apply |
| Cost Estimation | MVP | Predict token usage |

---

## Multi-Agent Coordination (Phase 2+)

### How It Works

1. **Task decomposition**: Architect breaks work into subtasks
2. **Parallel execution**: Implementer, Tester work concurrently
3. **Conflict resolution**: Merge strategy, rebase prompts, patch queues
4. **Review gates**: Reviewer agent blocks unsafe changes
5. **Cost caps**: Budget enforcement per run

### Branch-per-Agent Model

Each agent works on its own branch:
- `coder/architect/design-auth-system`
- `coder/implementer/add-login-form`
- `coder/tester/auth-tests`

Changes merge through normal PR flow with review gates.

---

## Agent Marketplace (Phase 4)

| Feature | Status | Description |
|---------|--------|-------------|
| Browse Agents | MVP | Discover agents |
| Search | MVP | Find by keyword |
| Categories | MVP | Filter by type |
| Reviews | MVP | User ratings |
| Install | MVP | One-click add |
| Pricing | MVP | Free/paid agents |
| Revenue Share | MVP | Creator earnings |
| Private Agents | Phase 2 | Team-only agents |

**Agent Categories:**
- Scaffolding (create projects)
- Refactoring (improve code)
- Testing (generate tests)
- Documentation (write docs)
- DevOps (CI/CD, deployment)
- Security (vulnerability scanning)
- Performance (optimization)
- Design (UI/UX assistance)

---

*Last Updated: December 2025*
