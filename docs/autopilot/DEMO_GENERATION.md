# Autopilot Demo Generation Workflow

## Overview

This document describes the workflow for generating high-quality autopilot demo sessions for the d-027 demo gallery and dogfooding funnel.

## Current Architecture

### Issue Management

Autopilot uses a **local SQLite database** for issue tracking, NOT direct GitHub integration:

```
crates/issues/           → Issue data models and database layer
crates/issues-mcp/       → MCP server exposing issue tools
autopilot.db             → SQLite database (per project)
```

**Important:** GitHub issues created via `gh issue create` are separate from the local autopilot database. They serve as a public backlog but must be manually imported into the local database for autopilot processing.

### Autopilot Execution Flow

1. **Preflight** - Verify environment, auth, git status
2. **Issue Selection** - Query MCP for next ready issue
3. **Analysis** - GPT-OSS analyzes codebase context
4. **Planning** - Claude generates implementation plan
5. **Execution** - Claude executes plan with tools
6. **Review** - Claude reviews work for completeness
7. **Verification** - Run tests, check build, validate d-012 compliance
8. **Commit** - Create branch, commit changes, push
9. **PR Creation** - Generate PR with receipt
10. **Report** - Generate after-action report

### Session Logging

All sessions are logged in `.rlog` format:

```
docs/logs/YYYYMMDD/HHMMSS-<description>.rlog
```

Each `.rlog` file contains:
- YAML frontmatter (metadata, model, tokens, timing)
- Event stream (agent messages, tool calls, outputs)
- Complete conversation history for replay

## Demo Generation Workflow

### Phase 1: Create Well-Scoped Issues (✅ Complete)

Created 10 GitHub issues (#1525-#1534) covering:
- Wallet error handling
- Marketplace fuzzy search
- Replay viewer controls
- FROSTR logging
- WGPUI performance
- NIP-58 badges
- CLI help examples
- Autopilot metrics
- Relay connection pooling
- Spark integration tests

**Status:** These issues exist on GitHub but are NOT in the local autopilot database.

### Phase 2: Import Issues to Local Database

**TODO:** Implement GitHub → SQLite sync

Options:
1. Manual import via `issues-mcp` tools
2. Create import script using `issues` crate API
3. Implement GitHub webhook → local DB sync

**For now:** Use existing local database issues or create issues directly in SQLite.

### Phase 3: Run Autopilot Sessions

#### Option A: Daemon Mode (Recommended for production)

**Status:** Daemon implementation exists in `agent/001` worktree branch but not merged to main.

Location: `.worktrees/agent-001/crates/autopilot/src/bin/autopilotd.rs`

Features:
- Continuous operation with crash recovery
- Memory pressure monitoring
- Stall detection and restart
- Known-good binary system
- Exponential backoff on failures

**TODO:** Merge daemon implementation to main branch.

#### Option B: Manual Execution (Current approach)

Run individual sessions manually:

```bash
# Set model (optional)
export AUTOPILOT_MODEL=sonnet  # or opus

# Run autopilot on specific task
cargo run --bin autopilot

# Sessions are logged to docs/logs/YYYYMMDD/
```

The GUI will:
1. Check auth (OpenCode → OpenAgents)
2. Run preflight checks
3. Query for next ready issue
4. Execute full workflow
5. Log session to .rlog file

### Phase 4: Session Quality Assessment

Review generated `.rlog` files for demo suitability:

**Quality Criteria:**
- ✅ Completes successfully (not blocked/errored)
- ✅ Shows interesting coding (not trivial changes)
- ✅ Demonstrates multiple tools (Read, Edit, Bash, etc.)
- ✅ Has clear narrative flow (analysis → plan → execute → verify)
- ✅ Duration 15-60 minutes (not too short/long)
- ✅ Ends with passing tests and working code

**Scoring Metrics:** (see issue #1532)
- Tasks completed vs attempted
- Files modified
- Tests passed / tests run
- Build errors encountered and fixed
- Code quality indicators

### Phase 5: Convert to Replay Bundles

For each selected session:

```bash
# Bundle includes:
# - .rlog file (session events)
# - .rlog.meta.json (metadata, metrics)
# - Git diff (changes made)
# - Test results
# - CI status (if available)

# TODO: Implement bundling script
# Inputs: .rlog file path
# Outputs: .replay.tar.gz with all artifacts
```

### Phase 6: Deploy to Demo Gallery

```bash
# Upload bundles to hosting
# Update gallery index
# Deploy to openagents.com/demos

# TODO: Document deployment infrastructure
```

## Current Status (2025-12-27)

### Completed ✅
- Created 10 well-scoped GitHub issues
- Documented autopilot architecture
- Identified gap: GitHub issues ↔ local DB sync
- Committed permission bypass changes for autonomous operation

### In Progress 🚧
- Documenting demo generation workflow (this file)
- Selecting best existing sessions for demos

### Blocked ⛔
- Continuous autopilot operation (waiting for daemon merge OR manual session execution)
- GitHub issue sync to local database

### Next Actions

**Immediate (can do now):**
1. Review existing `.rlog` sessions in `docs/logs/`
2. Select 5 best sessions based on quality criteria
3. Create replay bundles from selected sessions
4. Set up demo gallery deployment infrastructure
5. Deploy selected demos to openagents.com

**Near-term (need implementation):**
1. Create GitHub → SQLite import tool
2. Merge daemon from agent/001 branch
3. Implement session metrics and scoring
4. Build replay bundling automation
5. Set up CI for automatic demo generation

**Future (post-launch):**
1. Automated demo refresh (weekly autopilot runs)
2. User feedback integration
3. Demo rating and curation
4. Payment infrastructure for marketplace

## Files and References

- Architecture: `SYNTHESIS.md`
- Plan: `~/.claude/plans/vivid-floating-bengio.md`
- Issues crate: `crates/issues/src/`
- Autopilot core: `crates/autopilot/src/`
- Daemon (branch): `.worktrees/agent-001/crates/autopilot/src/bin/autopilotd.rs`
- Session logs: `docs/logs/YYYYMMDD/*.rlog`
- GitHub issues: https://github.com/OpenAgentsInc/openagents/issues/1525-1534

## Notes

- The GitHub issues created (#1525-#1534) are valuable as a public backlog and will be useful once GitHub sync is implemented
- For immediate demo generation, we should work with existing `.rlog` sessions or create new issues directly in the local database
- The daemon implementation is production-ready in the agent/001 branch and should be prioritized for merge
- Demo gallery infrastructure needs definition (hosting, CDN, domain setup)

---

**Last Updated:** 2025-12-27
**Status:** Documentation in progress
**Next Review:** After first demo deployment
