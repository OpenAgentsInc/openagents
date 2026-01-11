# Development Guide

Instructions for coding agents and contributors working on the OpenAgents codebase.

**START HERE:** Read [SYNTHESIS_EXECUTION.md](../SYNTHESIS_EXECUTION.md) first. It explains how Pylon, Nexus, Runtime, Autopilot, and WGPUI fit together with data flow diagrams, key paths, and build commands.

## Tech Stack

- **Rust** edition 2024, **WGPUI** for GPU-rendered UI, **Nostr** for NIP-90
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

**Commits:** When committing, stage only your own files explicitly. Do not assume all changes in the worktree should be committed.

**Autopilot commits:** Add `Co-Authored-By: Autopilot <autopilot@openagents.com>` after Codex's co-author line.

## Database

**NEVER raw sqlite3 for writes.** Use APIs:
- `cargo autopilot issue create` / `claim` / `complete` / `block`
- Read-only queries are fine for debugging

## Crates

- All crates: `edition = "2024"`
- Tests go in respective crates (`crates/foo/src/tests/`)

## Nostr

NIP specs are in `~/code/nips/`. Read locally, don't web search.

## Completion Standards

Issues are NOT done unless:
1. No stubs, mocks, TODOs, NotImplemented errors
2. Code actually works (tested)
3. SDK integrations are real, not stubbed

## Onyx Development (macOS)

Build and install Onyx.app:

```bash
./script/bundle-mac --sign --install
xattr -cr /Applications/Onyx.app
open /Applications/Onyx.app
```

**Prerequisites:**
- `cargo install cargo-bundle --git https://github.com/zed-industries/cargo-bundle.git --branch zed-deploy`
- App icons in `crates/onyx/resources/`

## Web Deployment

**ALWAYS use Cloudflare Workers, NEVER Cloudflare Pages.**

```bash
cd crates/web
bun run deploy          # Production
bun run deploy:preview  # Preview
```

## WASM Compatibility

**NEVER use `std::time::Instant` in wgpui or web client code** - it doesn't work in WASM.

Use `web_time::Instant` instead (from the `web-time` crate).

## Rules

- **No placeholder data** - Connect to real sources or show empty state
- **No GitHub workflows**

## Design Philosophy

Dense. Give me the information. Dial down your whitespace. Dial up your contrast.

Fast. Local-first or optimistic writes on the client. Few if any animations.

Present. Do not navigate me if I don't need to be navigated. Allow multiple panes.

File over app. Open file format that many apps can view.

Moldable. Decompose your app into lower primitives that can be loosely recomposed.

BYOAI. Let me bring my own services and information.
