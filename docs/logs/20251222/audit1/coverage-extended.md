# Extended Audit Coverage

Additional systems reviewed in this pass:
- Compute provider: `crates/compute/src/app.rs`, `crates/compute/src/storage/secure_store.rs`, `crates/compute/src/services/dvm_service.rs`, `crates/compute/src/services/relay_service.rs`, `crates/compute/src/services/ollama_service.rs`
- Marketplace: `crates/marketplace/src/core/payments.rs`, `crates/marketplace/src/compute/pricing.rs`, `crates/marketplace/src/skills/install.rs`, `crates/marketplace/src/compute/db.rs`
- AgentGit: `crates/agentgit/src/server.rs`, `crates/agentgit/src/git/clone.rs`
- Nostr client: `crates/nostr/client/src/relay.rs`, `crates/nostr/client/src/outbox.rs`
- Recorder: `crates/recorder/src/lib.rs`

This is in addition to the autopilot-focused review captured in findings.md.
