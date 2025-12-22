# Deeper Audit Coverage (Pass 2)

Additional systems reviewed in this pass:
- Unified GUI routes + autopilot runner: `src/gui/routes/autopilot.rs`
- Claude usage quota fetch: `src/gui/state.rs`
- Codex Agent SDK transport + thread lifecycle: `crates/codex-agent-sdk/src/transport/process.rs`, `crates/codex-agent-sdk/src/thread.rs`
- Nostr relay server + DB layer: `crates/nostr/relay/src/server.rs`, `crates/nostr/relay/src/db.rs`
- Config merge helpers: `crates/config/src/loader.rs`
- Directive progress calculations: `crates/issues/src/directive.rs`
