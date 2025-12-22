# Deeper Audit Coverage

Additional systems reviewed in this pass:
- Plan mode + MCP entrypoint: `crates/autopilot/src/planmode.rs`, `crates/issues-mcp/src/main.rs`
- Autopilot GUI permissions: `crates/autopilot-gui/src/views/permissions.rs`, `crates/autopilot-gui/src/server/routes.rs`, `crates/autopilot-gui/src/storage/permissions.rs`
- Wallet identity + Nostr client: `crates/wallet/src/core/identity.rs`, `crates/wallet/src/core/client.rs`
- Spark wallet integration: `crates/spark/src/wallet.rs`
- FROSTR keygen/SSS: `crates/frostr/src/keygen.rs`
- FM bridge client: `crates/fm-bridge/src/client.rs`
- Claude MCP manifest (missing sources noted): `crates/claude-mcp/Cargo.toml`
