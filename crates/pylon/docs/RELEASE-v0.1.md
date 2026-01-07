# Pylon v0.1 Release Notes

**Release Phase:** Regtest Alpha
**Release Date:** January 2026

## Overview

Pylon v0.1 is the first release of the node software that connects user compute to the global AI marketplace via Nostr. This release establishes the core infrastructure for Bitcoin-paid inference jobs.

## Key Features

### NIP-90 Data Vending Machine (DVM)

- **Provider mode:** Host inference backends and earn Bitcoin for completed jobs
- **Buyer mode:** Submit jobs to providers and pay via Lightning
- Supports NIP-90 job requests (kind:5xxx), results (kind:6xxx), and feedback (kind:7000)
- Broadcast job discovery (jobs without `#p` tag reach all providers)

### NIP-42 Authentication

- Automatic AUTH challenge/response for relays requiring authentication
- Auth key propagation across relay pools
- 200ms post-connect delay for AUTH handshake completion

### Inference Backends

- **Apple Foundation Models:** Zero-download inference on M-series Macs via FM Bridge
- **Ollama:** Supports local Ollama installations
- **llama.cpp:** GGUF model support (planned)

### Spark Wallet Integration

- Built-in Bitcoin/Lightning wallet using Spark protocol
- Regtest faucet funding support
- BOLT-11 invoice generation for job payments
- Auto-pay feature for seamless job submission

### Multi-Relay Support

- Connect to multiple relays simultaneously
- Default relays: nexus.openagents.com, relay.damus.io, nos.lol
- NIP-89 handler announcements for provider discovery

## CLI Commands

```bash
pylon init                    # Create Nostr identity
pylon wallet fund             # Get regtest sats from faucet
pylon wallet balance          # Check wallet balance
pylon start -f -m provider    # Start as provider (foreground)
pylon job submit "prompt" --auto-pay  # Submit job with auto-payment
pylon infer --prompt "Hello"  # Local inference test
pylon doctor                  # Check system status
```

## Technical Details

### Architecture

- Daemon-based architecture with Unix socket IPC
- SQLite persistence for jobs, invoices, and provider stats
- Tracing-based logging with log crate bridge
- Async Rust with Tokio runtime

### Key Files Changed

- `crates/pylon/src/provider.rs` - Provider mode implementation
- `crates/pylon/src/cli/job.rs` - Job submission with auto-pay
- `crates/nostr/client/src/dvm.rs` - DVM client (queue disabled for reliability)
- `crates/nostr/client/src/relay.rs` - NIP-42 AUTH handling

### Bug Fixes in v0.1

1. **Tracing filter** - Added `compute` and `nostr_client` crates to filter
2. **Auth key timing** - Removed premature relay connect before auth key set
3. **Log bridge** - Added tracing-log bridge for `log::` macro capture
4. **SQLite contention** - Disabled queue for DVM clients to fix multi-relay timeout
5. **Subscription ID length** - Shortened to < 64 chars for public relay compatibility

## Known Limitations

- Payment detection requires separate provider/buyer wallets
- No automatic backend selection (uses first available)
- No retry logic for failed job results
- Regtest only (mainnet support planned)

## Next Steps

- Complete E2E payment detection flow
- Add `pylon earnings` command for provider stats
- Implement result delivery after payment confirmation
- Add mainnet/testnet network support

## Dependencies

- Rust 2024 edition
- Tokio async runtime
- rusqlite for persistence
- breez-sdk-spark for Lightning
- nostr-client for relay communication
