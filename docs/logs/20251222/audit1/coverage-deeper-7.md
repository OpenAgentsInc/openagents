# Deeper Audit Coverage (Pass 7)

Additional systems reviewed in this pass:
- Compute provider core + services: `crates/compute/src/app.rs`, `crates/compute/src/services/dvm_service.rs`, `crates/compute/src/services/relay_service.rs`, `crates/compute/src/services/ollama_service.rs`, `crates/compute/src/storage/secure_store.rs`
- Marketplace compute consumer + discovery: `crates/marketplace/src/compute/consumer.rs`, `crates/marketplace/src/data/discover.rs`, `crates/marketplace/src/skills/browse.rs`, `crates/marketplace/src/relay.rs`
- Autopilot GUI permissions routes: `crates/autopilot-gui/src/server/routes.rs`
- FM bridge client: `crates/fm-bridge/src/client.rs`
