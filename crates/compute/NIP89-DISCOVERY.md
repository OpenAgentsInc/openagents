# NIP-89 Handler Discovery for Compute Providers

This document explains how OpenAgents compute providers use NIP-89 for social discovery on Nostr.

## Overview

NIP-89 enables compute providers to advertise their capabilities and be discovered through social trust signals. When a compute provider starts, it publishes a handler information event (kind 31990) to configured Nostr relays.

## Handler Information

The compute provider publishes the following information:

- **Handler Type**: `compute_provider`
- **Capabilities**:
  - `text-generation` - AI text generation using Ollama
  - `nip90-kind-5050` - NIP-90 text generation jobs
- **Pricing**: Configurable millisats per request (default: 1000 msats)
- **Metadata**:
  - Name: "OpenAgents Compute Provider"
  - Description: "AI inference provider using Ollama for NIP-90 data vending machine jobs"
  - Website: https://openagents.com

## Publishing Flow

1. **Startup**: When `DvmService::start()` is called, the service:
   - Connects to configured Nostr relays
   - Subscribes to job requests for the provider's pubkey
   - Calls `publish_handler_info()` to advertise capabilities

2. **Handler Info Event**: The service creates a NIP-89 kind 31990 event with:
   - Tags describing handler type, capabilities, and pricing
   - JSON content with metadata
   - Signed with the provider's Nostr keypair

3. **Publishing**: The signed event is published to all connected relays

4. **Discovery**: Clients can discover compute providers by:
   - Querying for kind 31990 events with `handler` tag = `compute_provider`
   - Filtering by capabilities (e.g., `nip90-kind-5050`)
   - Ranking by social trust scores based on recommendations

## Social Trust

NIP-89 supports handler recommendations (kind 31989) that create social trust signals:

- **Direct follows**: weight 1.0
- **Follow-of-follows**: weight 0.5
- **Two degrees separation**: weight 0.25
- **Unknown recommenders**: weight 0.1

Clients can calculate trust scores for compute providers based on the social graph of recommenders.

## Example Event

```json
{
  "id": "...",
  "pubkey": "...",
  "created_at": 1734700000,
  "kind": 31990,
  "tags": [
    ["handler", "compute_provider"],
    ["capability", "text-generation"],
    ["capability", "nip90-kind-5050"],
    ["price", "1000", "per-request", "sats"]
  ],
  "content": "{\"name\":\"OpenAgents Compute Provider\",\"description\":\"AI inference provider using Ollama for NIP-90 data vending machine jobs\",\"website\":\"https://openagents.com\"}",
  "sig": "..."
}
```

## Configuration

Handler publishing is automatic but can be customized via `DvmConfig`:

```rust
let mut config = DvmConfig::default();
config.min_price_msats = 5000;  // Set pricing
config.default_model = "llama3.2";  // Default inference model
dvm_service.set_config(config);
```

## Implementation

See `crates/compute/src/services/dvm_service.rs`:
- `publish_handler_info()` - Creates and publishes the handler info event
- `start()` - Calls `publish_handler_info()` during startup

The implementation uses types from `crates/nostr/core/src/nip89.rs`:
- `HandlerInfo` - Handler information builder
- `HandlerMetadata` - Name, description, website
- `HandlerType::ComputeProvider` - Handler type enum
- `PricingInfo` - Pricing details

## Future Enhancements

- Periodic re-publishing of handler info (e.g., every 24 hours)
- Dynamic capability updates based on available Ollama models
- Support for handler recommendations to build social trust
- Geographic/relay-specific handler advertising
