# Agent Instructions

OpenAgents desktop foundation repo.

## Tech Stack

- **Rust** edition 2024, **WGPUI** for GPU-rendered UI, **Nostr** for NIP-90, **claude-agent-sdk** for Claude Code
- Crates: `wgpui` (UI), `compute` (NIP-90), `nostr/core` (protocol)
- UI: Sharp corners, inline styling, **Vera Mono font ONLY**

## Git Rules

**Safety:**
- NEVER `push --force` to main
- NEVER commit unless asked
- NEVER use `-i` flag (interactive not supported)
- NEVER destructive commands (`reset --hard`, `checkout -- .`, `restore .`) without asking

**Multi-agent coordination:** Other agents may have uncommitted work. Before discarding changes in files you didn't modify, run `git diff <file>` and ASK first.

**Autopilot commits:** Add `Co-Authored-By: Autopilot <autopilot@openagents.com>` after Claude's co-author line.

**Conflict resolution:**
```bash
git stash push -m "my-work" && git pull --rebase origin main && git stash pop && git push
```

## Database

**NEVER raw sqlite3 for writes.** Use APIs:
- `cargo autopilot issue create` / `claim` / `complete` / `block`
- Read-only queries are fine for debugging

## Crates

- All crates: `edition = "2024"`
- Tests go in respective crates (`crates/foo/src/tests/`)

## Nostr

NIP specs are in `~/code/nips/`. Read locally, don't web search.

## CLI

```bash
cargo run --bin openagents -- <cmd>

openagents                          # GUI (default)
openagents wallet init|whoami|balance|send
openagents marketplace compute|skills|data|trajectories
openagents autopilot run|dashboard|replay
openagents daemon start|status|stop
```

## Autopilot Daemon

See [docs/autopilot/DAEMON.md](docs/autopilot/DAEMON.md).

**Known-good binary:** Daemon uses `~/.autopilot/bin/autopilot`. After successful builds:
```bash
cargo build -p autopilot && cp target/debug/autopilot ~/.autopilot/bin/
```

## Completion Standards

Issues are NOT done unless:
1. No stubs, mocks, TODOs, NotImplemented errors
2. Code actually works (tested)
3. SDK integrations are real, not stubbed

## Rules

- **No placeholder data** - Connect to real sources or show empty state
- **No GitHub workflows**
