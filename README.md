# OpenAgents

Your agent command center.

Work in progress. First release ETA December 22.

## Tech stack

- Rust
- HTMX
- Tailwind
- Maud
- Nostr

## Structure

- `crates/autopilot` - autonomous task runner with trajectory logging
- `crates/claude-agent-sdk` - Claude Code integration
- `crates/compute` - NIP-90 provider core
- `crates/config` - shared configuration
- `crates/desktop` - desktop shell (wry/tao + local server)
- `crates/fm-bridge` - Apple Foundation Models client
- `crates/nostr/core` - Nostr protocol types (NIP-90)
- `crates/recorder` - session recorder format + CLI
- `crates/ui` - Maud/HTMX component library
