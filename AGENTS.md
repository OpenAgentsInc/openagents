# Agent Instructions

You are helping on the OpenAgents desktop foundation repo.

## Tech Stack

- **Rust** with edition 2024
- **wry/tao** for the native webview shell
- **Actix + Maud + HTMX** for the local UI server
- **Nostr core** for NIP-90 types and events
- **claude-agent-sdk** for Claude Code integration

## UI Architecture (Desktop Shell)

The desktop app is a local Actix server rendered inside a native webview:

```
openagents/            → workspace root
crates/desktop         → desktop shell (wry/tao + Actix)
crates/ui              → Maud/HTMX components
crates/compute         → NIP-90 provider core
crates/nostr/core      → protocol types
```

Conventions:
- Inline-first CSS with custom properties
- No border radius (sharp corners)
- Keep UI server-rendered (no SPA)

---

## Git Conventions

**Safety:**
- NEVER `push --force` to main
- NEVER commit unless explicitly asked
- NEVER use `-i` flag (interactive not supported)
- NEVER use destructive git commands (`git reset --hard`, `git checkout -- .`, `git restore .`) without asking first

**CRITICAL: Do NOT discard other agents' work!**

Multiple agents may be working on this repo simultaneously. If you see uncommitted changes in files you didn't modify:
- **DO NOT** run `git restore` on those files
- **DO NOT** run `git checkout -- <file>` on those files
- **DO NOT** run `git stash` without checking what you're stashing
- **DO** use `git diff <file>` to understand what changed
- **DO** commit your own changes in separate files, or wait

If a file has changes that conflict with your work, ASK the user before discarding anything. Another agent may have spent significant time on that implementation.

Example of what NOT to do:
```bash
# WRONG - discards another agent's work without checking
git restore crates/frostr/src/ecdh.rs

# RIGHT - check what's there first
git diff crates/frostr/src/ecdh.rs
# Then ask user: "This file has uncommitted changes. Should I discard them?"
```

**Autopilot Commits:**
When running in autopilot mode (autonomous issue processing), include an additional co-author line to identify work done through the autopilot system:

```
Co-Authored-By: Autopilot <autopilot@openagents.com>
```

This should appear after the Claude co-author line in commit messages:

```
Your commit message here

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
Co-Authored-By: Autopilot <autopilot@openagents.com>
```

This makes it easy to identify commits that came through the autonomous autopilot workflow vs regular Claude Code usage.

---

## Agent Coordination (CRITICAL)

**Multiple agents working on the same repo WILL cause conflicts.**

When both a human-operated Claude Code session AND autopilot are running:
1. They share the same working directory
2. File changes can be overwritten
3. Git operations can conflict
4. Uncommitted work can be lost

**Best practices:**

1. **Commit frequently** - Don't let changes sit uncommitted
2. **Push after committing** - Get changes into remote ASAP
3. **Check git status before starting work** - See if other agents have uncommitted changes
4. **Use worktrees for parallel work** (recommended):
   ```bash
   # Create a worktree for autopilot
   git worktree add ../openagents-autopilot main

   # Now autopilot can work in ../openagents-autopilot/
   # while human works in ./openagents/
   ```

5. **Stop autopilot before making major UI changes** - If you're editing files that autopilot might touch

**If you see "rejected because remote contains work":**
```bash
git stash push -m "my-work"
git pull --rebase origin main
git stash pop
git push origin main
```

---

## Database Operations

**NEVER use raw sqlite3 commands to insert or modify data.** Always use the provided APIs:

- Use `cargo autopilot issue create` or `issue_create` MCP tool to create issues
- Use `cargo autopilot issue claim/complete/block` for issue state changes
- Direct sqlite3 commands bypass counters and triggers, causing data inconsistency

If you need to query data for debugging, read-only sqlite3 commands are fine:
```bash
sqlite3 autopilot.db "SELECT * FROM issues"  # OK - read only
sqlite3 autopilot.db "INSERT INTO ..."        # NEVER - use the API
```

---

## Rust Crates

All crates in `crates/` must use `edition = "2024"` in their `Cargo.toml`.

**Testing:** Tests go in their respective crates (e.g., `crates/foo/src/tests/`). Do NOT create separate test crates.

---

## Nostr Protocol Development

When implementing NIPs (Nostr Implementation Possibilities):

**IMPORTANT: NIP specifications are in `~/code/nips/` directory.**
- Read specs from local files: `~/code/nips/09.md` for NIP-09, etc.
- Do NOT web search for NIP specifications
- Reference implementations in `~/code/nostr` and `~/code/nostr-rs-relay` (for study only)

---

## Unified OpenAgents Binary

All OpenAgents functionality is available through a single `openagents` binary.

**Running the binary:**
```bash
# During development (from workspace root)
cargo run --bin openagents -- <subcommand>

# Or build first, then run
cargo build --bin openagents --release
./target/release/openagents <subcommand>

# After installing globally
cargo install --path .
openagents <subcommand>
```

**Note:** `cargo openagents` is NOT valid syntax. Cargo subcommands require a `cargo-` prefix package.

**Available commands:**
```bash
# Launch GUI (default)
openagents

# Wallet commands
openagents wallet init          # Initialize wallet
openagents wallet whoami        # Show identity
openagents wallet balance       # Show balance
openagents wallet send <addr> <amt>

# Marketplace commands
openagents marketplace compute providers
openagents marketplace skills browse
openagents marketplace data search
openagents marketplace trajectories contribute

# Autopilot commands
openagents autopilot run "task"
openagents autopilot dashboard
openagents autopilot replay <file>

# AgentGit commands
openagents agentgit gui

# Daemon commands
openagents daemon start --workdir /path --project myproject
openagents daemon status
openagents daemon stop
```

**Note:** The legacy separate binaries (`wallet`, `marketplace`, `autopilot`, `autopilotd`, `agentgit`) have been deprecated. Use the unified `openagents` binary instead.

---

## Autopilot Daemon

The `openagents daemon` (formerly `autopilotd`) supervises autopilot worker processes, handling crashes and memory pressure.

**Starting the daemon:**
```bash
# Development
openagents daemon start --workdir /path/to/project --project myproject

# Production (systemd)
systemctl --user start openagents-daemon
```

**Commands:**
```bash
openagents daemon status         # Check daemon and worker status
openagents daemon restart-worker # Restart worker without restarting daemon
openagents daemon stop           # Stop daemon
```

**Viewing logs:**
```bash
# Worker logs are in the standard rlog location
tail -f docs/logs/$(date +%Y%m%d)/*.rlog

# Daemon logs (when running with systemd)
journalctl --user -u autopilotd -f
```

**Memory management:**
- Daemon monitors system memory every 5 seconds
- Kills node processes >500MB when memory is low (<2GB)
- Force-restarts worker when memory is critical (<1GB)
- Automatic exponential backoff on crashes (1s → 5min max)

---

## Directive Completion Standards (CRITICAL)

**A directive is NOT complete just because issues are marked "done".**

Before marking any issue as "done", you MUST verify:

1. **d-012 compliance** - No stubs, no mocks, no TODOs, no NotImplemented errors
2. **Code actually works** - Run it, test it, verify it does what it claims
3. **Real integrations** - If the issue references an SDK/library, it must be INTEGRATED not stubbed

### Spark SDK Integration (d-001)

The Spark SDK at `~/code/spark-sdk` is the reference for Breez SDK integration:
- Public API: `crates/breez-sdk/core/src/sdk.rs` (BreezSdk struct)
- Builder: `crates/breez-sdk/core/src/sdk_builder.rs`
- Models: `crates/breez-sdk/core/src/models/mod.rs`
- Examples: `docs/breez-sdk/snippets/rust/src/`

**You MUST integrate this SDK directly.** Do NOT:
- Return "requires Breez SDK integration" errors
- Comment out code with "BLOCKED" notes
- Mark Phase 1 complete when Phase 2+ are all stubbed
- Say "will integrate later" - integrate NOW or don't claim it's done

### Verification Before Marking Done

For any payment/wallet related issue:
- Does `cargo test -p spark` pass?
- Can you actually call the function and get real data back?
- Is there a dependency on `breez-sdk-spark` in Cargo.toml?
- Are the actual SDK functions being called (not mocked)?

**If ANY of these fail, the issue is NOT done.**
