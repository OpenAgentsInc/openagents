# Directives

Directives are high-level goals that set the direction for the project. They represent epics like "Implement 100% of Nostr Protocol" or "Add comprehensive test coverage".

## Format

Each directive is a Markdown file with YAML frontmatter:

```markdown
---
id: "d-001"
title: "Implement 100% of Nostr Protocol"
status: active  # active | paused | completed
priority: high  # urgent | high | medium | low
created: 2025-12-20
updated: 2025-12-20
---

## Goal

Fully implement the Nostr protocol in Rust for both client and relay functionality.

## Success Criteria

- [ ] All NIPs implemented in crates/nostr/core
- [ ] Relay passes all protocol tests
- [ ] Client can connect to public relays

## Notes

Additional context, links to specs, etc.
```

## Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique identifier (e.g., `d-001`) |
| `title` | Yes | Short descriptive title |
| `status` | No | `active`, `paused`, or `completed` (default: `active`) |
| `priority` | No | `urgent`, `high`, `medium`, or `low` (default: `medium`) |
| `created` | Yes | Date created (YYYY-MM-DD) |
| `updated` | No | Date last updated (auto-set on save) |

## CLI Commands

```bash
# List all directives
cargo autopilot directive list

# List only active directives
cargo autopilot directive list --status active

# Show directive details
cargo autopilot directive show d-001

# Create a new directive
cargo autopilot directive create d-002 "Add Test Coverage"

# Pause a directive
cargo autopilot directive pause d-001

# Complete a directive
cargo autopilot directive complete d-001

# Resume a paused directive
cargo autopilot directive resume d-001
```

## Linking Issues to Directives

When creating issues, use the `directive_id` parameter to link them:

```bash
# Via MCP tool
issue_create title="Implement NIP-01" directive_id="d-001"

# Linked issues appear in directive progress
cargo autopilot directive show d-001
```

## How Autopilot Uses Directives

When no ready issues are available, autopilot loads active directives and prompts the agent to create concrete issues to advance them. The agent should:

1. Review the directive's goal and success criteria
2. Identify the next actionable steps
3. Create 1-3 specific issues linked to the directive
4. Continue working on the new issues

Progress is automatically tracked based on completed/total linked issues.

## Best Practices for Writing Directives

### Structure

A good directive body should include:

1. **Goal** - Clear, concise statement of what success looks like
2. **Background** - Context, motivation, and relevant technical details
3. **Architecture** - Diagrams or descriptions of the system design
4. **Success Criteria** - Phased checklist of concrete deliverables
5. **Key Files** - Table of files to create or modify
6. **Dependencies** - External and internal crate dependencies
7. **Testing Strategy** - How the work will be validated
8. **Notes** - Additional context, references, caveats

### Guidelines

- **Be specific** - Vague goals lead to scope creep
- **Phase the work** - Break large efforts into sequential phases
- **Link references** - Point to external specs, repos, or docs
- **Track progress** - Use checkbox lists that can be updated
- **Keep it current** - Update the directive as understanding evolves

### Naming Convention

- Use sequential IDs: `d-001`, `d-002`, etc.
- File name matches ID: `d-001.md`
- Titles should be action-oriented: "Implement X" not "X Implementation"

## Current Active Directives

| ID | Title | Focus Area |
|----|-------|------------|
| d-001 | Integrate Breez Spark SDK for Bitcoin Payments | Payments |
| d-002 | Implement 100% of Nostr Protocol | Protocol |
| d-003 | OpenAgents Wallet - Complete Identity & Payment Solution | Application |
| d-004 | Continual Constant Improvement of Autopilot | Meta/Infrastructure |
| d-005 | Build Nostr GitHub Alternative (AgentGit) | Agent Infrastructure |
| d-006 | Operationalize NIP-SA (Sovereign Agents Protocol) | Agent Infrastructure |

View details with `cargo autopilot directive show <id>`
