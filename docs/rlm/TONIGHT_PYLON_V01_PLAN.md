# Pylon v0.1 Tonight Plan (CLI + Spark testnet)

Goal: ship a single `pylon` CLI binary for tomorrow's GitHub release, with NIP-90 provider mode working end-to-end and testnet payments via Spark (Breez SDK).

## Code Reality (verified)

- Pylon CLI surface: `crates/pylon/src/cli/mod.rs` (init/start/stop/status/doctor/agent/earnings/infer/compute/connect/neobank).
- Provider + Spark wallet: `crates/pylon/src/provider.rs` (initializes SparkWallet) + `crates/compute/src/services/dvm_service.rs` (invoice creation, payment monitoring, bolt11 tags).
- NIP-90 protocol + client: `crates/nostr/core/src/nip90.rs` and `crates/nostr/client/src/dvm.rs`.
- Runtime auto-pay path (for buyers using DVM container): `crates/runtime/src/containers.rs` + Spark adapter in `crates/runtime/src/wallet_spark.rs`.
- Spark network mapping: `crates/spark/src/wallet.rs` (Testnet/Signet map to Regtest).
- Web wallet uses Spark (for UI testing if needed): `crates/web/worker/src/routes/wallet.rs`.

## Tonight Checklist (v0.1)

### A) Wire Pylon config into DVM payments

- [ ] Set DVM config from Pylon config in `crates/pylon/src/provider.rs` (min_price_msats, require_payment, default_model, network).
- [ ] Keep `enable_payments` as the gate for `init_wallet()` and log clearly when running in free mode.
- [ ] Ensure bolt11 + amount are included in result events when payment is required (DVM already supports this).

### B) Spark testnet (regtest) setup

- [ ] Update `~/.config/pylon/config.toml` defaults or docs to surface:
  - `network = "testnet"` (maps to Breez regtest internally).
  - `enable_payments = true`.
  - `require_payment = true` for paid jobs.
- [ ] Confirm wallet storage dir is `data_dir()/wallet` and unique per network (no collisions).

### C) CLI release behavior (must be stable)

- [ ] `pylon init` writes identity + config and prints seed phrase.
- [ ] `pylon start --mode provider` starts DVM service, publishes NIP-89 handler info, and subscribes to job requests.
- [ ] `pylon status` shows jobs completed + earnings; `pylon earnings` shows payment totals; `pylon doctor` flags missing wallet/backends/relays.

### D) Payment loop test (testnet/regtest)

- [ ] Start provider with a local inference backend running.
- [ ] Submit a NIP-90 job using `nostr-client` DvmClient or the ignored test:
  - `cargo test -p nostr-client --test agent_chat_e2e -- --ignored --nocapture`
- [ ] Pay the bolt11 invoice using a Spark wallet (customer side).
- [ ] Confirm `DomainEvent::PaymentReceived` updates DB + `pylon earnings`.

### E) Release prep (GitHub v0.1)

- [ ] `cargo build --release -p pylon` and smoke-test the binary.
- [ ] Update Quickstart/Config docs to include payment + network settings.
- [ ] Draft release notes with CLI commands and payment flow.

## Notes / Risks

- Spark SDK only supports Mainnet + Regtest; "testnet" and "signet" map to Regtest internally.
- Pylon docs currently omit payment config fields; update `crates/pylon/docs/CONFIGURATION.md`.
- Wallet CLI (`crates/wallet`) is still stubbed for Spark; do not rely on it for v0.1 tests.

