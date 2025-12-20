# Marketplace Crate Starter Issues

## Goal
Create 1-3 starter issues to bootstrap `crates/marketplace/` in the openagents repo.

## Context
- **Repo**: `/Users/christopherdavid/code/openagents` (current workspace)
- **Docs**: `/Users/christopherdavid/code/platform/docs/marketplace/` (reference only)
- **Existing crates**: autopilot, issues, issues-mcp, recorder, claude-agent-sdk, compute, config, desktop, fm-bridge, nostr/core, ui, storybook
- **No marketplace crate exists yet**
- Uses `edition = "2024"`, workspace dependencies pattern

## Key Concepts from Docs

### Skills Marketplace (most concrete)
- Skills = composable domain expertise on top of MCP
- Schema: name, version, creator (Nostr pubkey), tools, dependencies, pricing
- Pricing models: free, per-call, per-token, hybrid
- Status flow: pending_review → approved → published

### Data Marketplace
- Crowdsource AI training data from Claude Code/Cursor sessions
- Anonymized session traces, workflow patterns, outcome signals
- User control, redaction, Lightning micropayments

### Protocol Stack
- Layer 0: MCP (connectivity)
- Layer 1: Bitcoin Lightning (payments)
- Layer 2: Nostr NIP-89/90 (identity & discovery)
- Layer 3: OpenAgents (coordination)

---

## Starter Issues

### Issue 1: Create crates/marketplace skeleton
**Type**: task | **Priority**: urgent

Create the marketplace crate with basic structure:
- `Cargo.toml` (add to workspace)
- `src/lib.rs` with module declarations
- `src/types.rs` with core enums: `MarketplaceItemType`, `ItemStatus`
- `src/skills/mod.rs` placeholder

### Issue 2: Define Skill types and manifest schema
**Type**: feature | **Priority**: high

Based on `docs/marketplace/02-skills-marketplace.md`:
- `Skill` struct: name, slug, version, creator_pubkey, description
- `SkillPricing`: free, per_call, per_token, hybrid
- `SkillDependency`: MCP server requirements
- `SkillManifest`: full JSON-serializable schema for publishing
- Validation for semver, required fields

### Issue 3: Add skill repository with SQLite storage
**Type**: feature | **Priority**: high

Database layer for skills:
- Schema: `skills`, `skill_versions` tables
- Repository trait: `create`, `get_by_slug`, `list`, `update_status`
- Use `sqlx` with SQLite (matches platform patterns)
