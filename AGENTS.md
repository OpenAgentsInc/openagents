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

## Autopilot Daemon

The `autopilotd` daemon supervises autopilot worker processes, handling crashes and memory pressure.

**Starting the daemon:**
```bash
# Development
cargo daemon --workdir /path/to/project --project myproject

# Production (systemd)
systemctl --user start autopilotd
```

**Commands:**
```bash
autopilotd status         # Check daemon and worker status
autopilotd restart-worker # Restart worker without restarting daemon
autopilotd stop           # Stop daemon
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
