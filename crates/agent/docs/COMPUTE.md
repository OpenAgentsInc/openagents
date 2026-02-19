# Compute Client

This document explains how sovereign agents request NIP-90 compute using the runtime DVM provider.

## Overview

Sovereign agents are **customers** in the NIP-90 compute marketplace. The agent runner uses
`openagents-runtime` to handle the full request lifecycle:

1. Query NIP-89 handler info (provider discovery)
2. Submit a NIP-90 job request with a USD-denominated max bid
3. Collect quotes, select the lowest acceptable provider
4. Pay the Lightning invoice via the wallet service
5. Return the job result

The key change: **the runtime DVM provider handles NIP-90 protocol details**, so the agent runner
only needs a `WalletService` and a budget in sats.

## Provider Discovery

The compute client uses the runtime DVM provider to query NIP-89 handler info events:

```rust
let providers = compute_client.discover_providers(3)?;
```

Each entry maps handler info into a lightweight view:

```rust
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub pubkey: String,
    pub name: String,
    pub channel_id: Option<String>,
    pub relay_url: String,
    pub price_msats: u64,
    pub models: Vec<String>,
}
```

The client can filter by the `network` tag (e.g., `regtest`) and picks a model from the
advertised list:

```rust
let model = ComputeClient::select_model(&providers);
```

## Requesting Inference

The compute client converts a sats budget into `max_cost_usd`, then submits a
`ComputeRequest` via the runtime DVM provider. The DVM provider handles quoting,
invoice settlement, and result delivery.

```rust
let inference = compute_client
    .request_inference("summarize this", 500, 1_000, &model)
    .await?;

let text = inference.text;
let cost_sats = inference.cost_sats;
```

## Wallet Integration

The compute client depends on the runtime `WalletService` trait. The default adapter
for Spark is `SparkWalletService`:

```rust
use openagents_runtime::{SparkWalletService, WalletService};
use openagents_spark::{SparkWallet, WalletConfig};
use std::sync::Arc;

let wallet = Arc::new(SparkWallet::new(signer, config).await?);
let wallet_service: Arc<dyn WalletService> = Arc::new(SparkWalletService::new(wallet)?);
```

`WalletService` provides:
- `balance_sats()` for budgeting
- `pay_invoice()` for Lightning settlement
- `fx_rate()` for USD/sats conversion

## Example: Agent Runner Integration

```rust
let compute_client = ComputeClient::new(
    UnifiedIdentity::from_mnemonic(identity.mnemonic(), "")?,
    relay.clone(),
    wallet_service.clone(),
    Some("regtest".to_string()),
)?;

let providers = compute_client.discover_providers(3)?;
let model = ComputeClient::select_model(&providers);

let inference = compute_client
    .request_inference(&prompt, 500, budget_sats, &model)
    .await?;
```

## Notes

- Channel-based NIP-28 coordination is not used by the current compute client.
- Budgets are enforced by `max_cost_usd` bids and wallet FX conversion.
- The runtime DVM provider chooses the lowest quote it receives within the bid.
