# Agent Instructions

OpenAgents desktop repo.

## Tech Stack

- **Rust** edition 2024, **WGPUI** for GPU-rendered UI, **Nostr** for NIP-90, **claude-agent-sdk** for Claude Code
- Crates: `wgpui` (UI), `compute` (NIP-90), `nostr/core` (protocol)
- UI: Sharp corners, inline styling, **Vera Mono font ONLY**

## Git Rules

**Safety:**
- NEVER `push --force` to main
- NEVER use `-i` flag (interactive not supported)
- NEVER destructive commands (`reset --hard`, `checkout -- .`, `restore .`) without asking
- NEVER use `git stash` - it interferes with other agents' uncommitted work

**Commit Often:**
- COMMIT working code frequently (every 15-30 minutes of work)
- Don't let code sit uncommitted - it can be lost or cause merge conflicts
- Small, frequent commits are better than large, infrequent ones
- If you've made progress that works, commit it immediately

**Multi-agent coordination:** Other agents may have uncommitted work. Before discarding changes in files you didn't modify, run `git diff <file>` and ASK first.
**Clarification:** You can always continue work in your own files without asking permission, even if other files are dirty. You do not need permission to ignore other agents' changes.
**Commits:** When committing, stage only your own files explicitly. Do not assume all changes in the worktree should be committed.

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

## crates/web Deployment

**ALWAYS use Cloudflare Workers, NEVER Cloudflare Pages.**

```bash
cd crates/web
bun run deploy          # Production deploy to Workers
bun run deploy:preview  # Preview environment
```

This runs `bun run build && bun run build:worker && npx wrangler deploy`. Check `package.json` for scripts.

**NEVER run:**
- `npx wrangler pages deploy` - WRONG, we use Workers not Pages
- `npx wrangler deploy` directly without build steps

## WASM Compatibility

**NEVER use `std::time::Instant` in wgpui or web client code** - it doesn't work in WASM.

Use `web_time::Instant` instead (from the `web-time` crate). This provides cross-platform time that works on both native and WASM.

Example: `wgpui::animation::AnimationController` uses `web_time::Instant` for delta time calculations.

## Rules

- **No placeholder data** - Connect to real sources or show empty state
- **No GitHub workflows**

## Design Philosophy

Dense. Give me the information. Dial down your whitespace. Dial up your contrast. Allow text to span more than 56 characters, allow spacing below 1.7, allow black borders between columns, scroll bars, tables with alternating background tints.

Fast. Local-first or optimistic writes on the client. Few if any animations. Smart use of fonts, images, and assets to prioritize quick loads and transitions.

Present. Do not navigate me if I don’t need to be navigated, but orient your design from the start for multiple views. Allow multiple panes. Look at high-pressure applications (trading, IDEs, medical, emergency, military) and take inspiration. Don’t take me away from my task.

File over app. Open file format that many apps can view. Skip the spec until things harden, but keep it open anyway. Let my bots read it.

Moldable. Decompose your app into lower primitives that can be loosely recomposed by the user to form many types of documents, dashboards, reports, etc. Start me at a good set of constructions. I’ll make the software into what I need it to be whether you like it or not, so make it easy for me and my bots.

BYOAI. Let me bring my own services and information, either because the application is open enough for the bots to interact with directly (see File over app) or via an exposed host / tunnel or some other dangerous route.

Above all remember that making software is getting easier so businesses will be making software on top of your software. Design for this!
