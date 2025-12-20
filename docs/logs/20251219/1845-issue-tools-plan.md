# Autopilot: Local Issue Tools + Skill

**Goal:** Expose `crates/issues/` as tools for autopilot + create issue-workflow skill.

---

## Background

Per Anthropic's agent patterns (from `agent-algorithms.md`):
- **Tools** = Atomic operations (single op, no state)
- **Skills** = Portable expertise packages (SKILL.md + scripts)

Architecture: Tools provide connectivity. Skills provide expertise.

---

## Implementation

### 1. Issue MCP Server

The Claude Agent SDK uses MCP servers for custom tools. Create an MCP server that exposes `crates/issues/` operations:

| Tool | Input | Output |
|------|-------|--------|
| `issue_list` | `{status?: "open"\|"in_progress"\|"done"}` | Array of issues |
| `issue_create` | `{title, description?, priority?, type?}` | Created issue |
| `issue_get` | `{number}` | Single issue or null |
| `issue_claim` | `{number, run_id}` | Success/failure |
| `issue_complete` | `{number}` | Success/failure |
| `issue_block` | `{number, reason}` | Success/failure |
| `issue_ready` | `{}` | Next claimable issue or null |

**MCP server setup in autopilot:**
```rust
let options = QueryOptions::new()
    .mcp_server("issues", McpServerConfig::Stdio {
        command: "cargo".to_string(),
        args: Some(vec!["run".to_string(), "-p".to_string(), "issues-mcp".to_string()]),
        env: Some(HashMap::from([("ISSUES_DB".to_string(), db_path)])),
    });
```

### 2. Issue-Workflow Skill

Create `~/.openagents/skills/issue-workflow/SKILL.md`:

```markdown
---
name: issue-workflow
description: Best practices for managing issues during autonomous work
triggers:
  - working on issues
  - planning tasks
  - breaking down work
---

# Issue Workflow

## When to Create Issues
- Task will take >15 minutes
- Task has clear acceptance criteria
- Task should be trackable

## Priority Guidelines
- urgent: Production broken, security issue
- high: Blocks other work, user-facing bug
- medium: Normal feature work (default)
- low: Nice-to-have, polish

## Issue Lifecycle
1. Create with clear title + description
2. Claim before starting work
3. Complete when done, block if stuck
4. Never leave issues claimed without progress

## Breaking Down Work
- Epic → multiple tasks (1-2 hours each)
- One issue per logical unit of work
- Link related issues in description

## Commit Messages
- Reference issue: "Fix auth timeout (#42)"
- Use conventional commits when possible
```

---

## Files to Create

| File | Purpose |
|------|---------|
| `crates/issues-mcp/Cargo.toml` | MCP server crate |
| `crates/issues-mcp/src/main.rs` | MCP stdio server with 7 tools |
| `~/.openagents/skills/issue-workflow/SKILL.md` | Issue management expertise |

---

## Implementation Steps

1. Create `crates/issues-mcp/` as new workspace member
2. Implement MCP stdio server with JSON-RPC protocol
3. Add 7 issue tools (list, create, get, claim, complete, block, ready)
4. Update autopilot to register the MCP server
5. Create skill folder and SKILL.md
6. Test: run autopilot and verify it can list/create issues

---

## Deferred (Future)

- Taskmaster integration (repos with `.openagents/taskmaster.db`)
- Platform API integration (remote coordination)
- Additional skills (taskmaster-sync, platform-agent)
