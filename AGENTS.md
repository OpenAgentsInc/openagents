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
