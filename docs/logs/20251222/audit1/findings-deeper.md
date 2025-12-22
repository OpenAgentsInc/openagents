# Deeper Findings

## High
- D-H-1 Plan mode accepts a slug from MCP without sanitization and uses it directly in a join, so `../` or absolute paths can write outside `~/.claude/plans` (arbitrary file write). Evidence: `crates/issues-mcp/src/main.rs:700`, `crates/autopilot/src/planmode.rs:83`, `crates/autopilot/src/planmode.rs:90`.

## Medium
- D-M-1 `claude-mcp` declares a binary at `src/main.rs` but the source file is missing, so workspace builds fail when this crate is included. Evidence: `crates/claude-mcp/Cargo.toml:10`, `crates/claude-mcp/Cargo.toml:12`.
- D-M-2 UnifiedIdentity derives Nostr public keys from compressed secp256k1 serialization (33 bytes), returns a placeholder `npub` string, and hardcodes profile reads to `None`; this breaks Nostr interoperability and hides cached metadata. Evidence: `crates/wallet/src/core/identity.rs:85`, `crates/wallet/src/core/identity.rs:86`, `crates/wallet/src/core/identity.rs:141`, `crates/wallet/src/core/identity.rs:149`, `crates/wallet/src/core/identity.rs:157`.
- D-M-3 Spark network mapping collapses Testnet/Signet/Regtest to `SdkNetwork::Regtest`, and `get_spark_address` is documented as a placeholder returning a public key hex string instead of a real address. Evidence: `crates/spark/src/wallet.rs:39`, `crates/spark/src/wallet.rs:43`, `crates/spark/src/wallet.rs:188`, `crates/spark/src/wallet.rs:194`.
- D-M-4 Autopilot GUI permissions UI is incomplete: edit/add actions are stubbed and routes re-open an ad-hoc `autopilot-permissions.db` instead of using shared app state, so persistence/configuration is inconsistent. Evidence: `crates/autopilot-gui/src/views/permissions.rs:218`, `crates/autopilot-gui/src/views/permissions.rs:223`, `crates/autopilot-gui/src/server/routes.rs:246`, `crates/autopilot-gui/src/server/routes.rs:281`.
- D-M-5 Permission rules advertise patterns like `Edit:*.rs`, but `check_pattern` only checks exact matches and `tool:*`, so wildcard/glob patterns never match. Evidence: `crates/autopilot-gui/src/storage/permissions.rs:18`, `crates/autopilot-gui/src/storage/permissions.rs:92`, `crates/autopilot-gui/src/storage/permissions.rs:113`.
- D-M-6 FROSTR Shamir interpolation can panic on duplicate share indices because GF(256) division/inversion uses `panic!` on zero denominators and there is no input validation. Evidence: `crates/frostr/src/keygen.rs:95`, `crates/frostr/src/keygen.rs:107`, `crates/frostr/src/keygen.rs:230`, `crates/frostr/src/keygen.rs:235`.

## Low
- D-L-1 Wallet Nostr fetch path only queries the first relay, ignoring additional configured relays and risking partial data. Evidence: `crates/wallet/src/core/client.rs:79`, `crates/wallet/src/core/client.rs:81`.
- D-L-2 FM bridge client construction uses `expect`, panicking instead of returning an error if HTTP client initialization fails. Evidence: `crates/fm-bridge/src/client.rs:28`, `crates/fm-bridge/src/client.rs:31`.
