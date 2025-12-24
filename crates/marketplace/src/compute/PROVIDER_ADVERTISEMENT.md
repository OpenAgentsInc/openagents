# Provider Capability Advertisement

## Status

✅ **Implementation Complete** - Provider capability advertisement is fully implemented in `provider.rs`.

🚧 **Integration Pending** - CLI command and automatic advertisement on `online` not yet wired up.

## What's Implemented

### Core Advertisement Functionality

The `Provider` struct in `compute/provider.rs` provides complete NIP-89 capability advertisement:

```rust
pub async fn advertise(
    &mut self,
    relay: &MarketplaceRelay,
    secret_key: &str,
) -> Result<Event, RelayError>
```

This method:
1. Creates a `HandlerInfo` from provider config
2. Builds a NIP-89 event (kind:31990)
3. Signs the event with provider's secret key
4. Publishes to configured relays
5. Updates `last_advertised` timestamp

### Event Structure

The advertisement event (kind:31990) includes:

**Tags:**
- `d`: Handler identifier
- `k`: Supported NIP-90 job kinds
- `web`: Website URL
- `picture`: Icon URL
- `about`: Description
- Pricing tags (per NIP-89 spec)
- Capability tags

**Content (JSON):**
```json
{
  "name": "Provider Name",
  "description": "Provider description",
  "icon_url": "https://...",
  "website": "https://..."
}
```

### Re-advertisement Timing

Providers track when they last advertised and automatically re-advertise at configurable intervals:

```rust
pub fn needs_readvertisement(&self) -> bool
pub fn time_until_next_advertisement(&self) -> Option<Duration>
```

Default re-advertisement interval: 3600 seconds (1 hour)

## What's Missing

### 1. CLI Command Integration

The CLI has no direct `advertise` command. Need to add:

```bash
cargo marketplace provider advertise
```

This should:
- Load provider config
- Get secret key from wallet
- Create MarketplaceRelay instance
- Call `provider.advertise()`
- Report success/failure

### 2. Automatic Advertisement on `online`

When provider goes online via `cargo marketplace provider online`, it should automatically advertise. Currently the `online()` function in `cli/provider.rs` only:

```rust
fn online(&self, json: bool) -> anyhow::Result<()> {
    let config = load_provider_config()?;
    let mut provider = Provider::new(config);
    provider.go_online();
    save_provider_state(&provider)?;

    // MISSING: Call provider.advertise() here

    println!("Note: Relay connection and job monitoring not yet implemented.");
    Ok(())
}
```

Should be:

```rust
fn online(&self, json: bool) -> anyhow::Result<()> {
    let config = load_provider_config()?;
    let mut provider = Provider::new(config);
    provider.go_online();

    // Get secret key from wallet
    let secret_key = get_wallet_secret_key()?;

    // Connect to relays
    let relay = MarketplaceRelay::connect(&config.relays).await?;

    // Advertise capabilities
    let event = provider.advertise(&relay, &secret_key).await?;
    println!("Advertisement published: {}", event.id);

    save_provider_state(&provider)?;
    println!("Provider is now ONLINE and advertising");
    Ok(())
}
```

### 3. Background Re-advertisement

Providers should re-advertise periodically while online. Need a background task:

```rust
// In provider daemon or online mode
tokio::spawn(async move {
    loop {
        if let Some(delay) = provider.time_until_next_advertisement() {
            tokio::time::sleep(delay).await;
        }

        if provider.is_online() && provider.needs_readvertisement() {
            if let Err(e) = provider.advertise(&relay, &secret_key).await {
                eprintln!("Re-advertisement failed: {}", e);
            }
        }
    }
});
```

### 4. Wallet Integration for Secret Key

The `advertise()` method requires a secret key to sign events. This should come from the wallet (d-003):

```rust
fn get_wallet_secret_key() -> Result<String> {
    // Load wallet
    // Get Nostr identity secret key
    // Return as hex string
}
```

### 5. Relay Connection Management

The `MarketplaceRelay` is used but not fully wired up in CLI. Need:

```rust
async fn connect_to_relays(config: &ProviderConfig) -> Result<MarketplaceRelay> {
    MarketplaceRelay::connect(&config.relays).await
}
```

## Implementation Plan

To complete provider capability advertisement:

### Step 1: Add CLI `advertise` Command

In `cli/provider.rs`:

```rust
#[derive(Subcommand)]
pub enum ProviderCommands {
    // ... existing commands ...

    /// Advertise capabilities on Nostr relays
    Advertise {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}
```

Implement handler:

```rust
async fn advertise(&self, json: bool) -> anyhow::Result<()> {
    let config = load_provider_config()?;
    let mut provider = Provider::new(config.clone());

    // Get secret key from wallet
    let secret_key = get_wallet_secret_key()?;

    // Connect to relays
    let relay = MarketplaceRelay::connect(&config.relays).await?;

    // Advertise
    let event = provider.advertise(&relay, &secret_key).await?;

    // Save state (updates last_advertised timestamp)
    save_provider_state(&provider)?;

    if json {
        println!("{}", serde_json::json!({
            "event_id": event.id,
            "relays": config.relays,
            "next_advertisement": provider.time_until_next_advertisement()
                .map(|d| d.as_secs()),
        }));
    } else {
        println!("✓ Advertisement published");
        println!("Event ID: {}", event.id);
        println!("Relays: {}", config.relays.join(", "));
        if let Some(next) = provider.time_until_next_advertisement() {
            let secs = next.as_secs();
            println!("Next advertisement in: {}h {}m", secs / 3600, (secs % 3600) / 60);
        }
    }

    Ok(())
}
```

### Step 2: Auto-advertise on `online`

Modify `online()` to call `advertise()` internally.

### Step 3: Add Background Re-advertisement

Either:
- **Option A**: Expect daemon to handle (preferred)
- **Option B**: Spawn background task in `online()` mode

### Step 4: Integrate with Wallet

Add wallet dependency and implement `get_wallet_secret_key()`.

## Testing

### Unit Tests

Already exist in `provider.rs`:

```rust
#[test]
fn test_provider_config_default()
#[test]
fn test_provider_builder()
#[test]
fn test_provider_state_transitions()
// etc.
```

### Integration Tests

Need to add:

```rust
#[tokio::test]
async fn test_advertise_publishes_to_relay() {
    // Create test provider
    // Mock relay
    // Call advertise()
    // Verify event published
}

#[tokio::test]
async fn test_readvertisement_timing() {
    // Advertise
    // Check needs_readvertisement() is false
    // Fast-forward time
    // Check needs_readvertisement() is true
}
```

### CLI Tests

```bash
# Configure provider
cargo marketplace provider config --name "Test Provider" \
  --capabilities llama3,mistral \
  --price-input 10 --price-output 20

# Advertise
cargo marketplace provider advertise

# Go online (should auto-advertise)
cargo marketplace provider online

# Check status (should show last advertisement time)
cargo marketplace provider status
```

## Dependencies

### Blocking:
- **d-003 (Wallet)** - Need Nostr secret key from wallet
- **Relay connection** - MarketplaceRelay needs to be fully functional

### Optional:
- **d-006 (NIP-SA)** - For agent-owned provider identities
- **d-007 (FROSTR)** - For threshold-protected provider keys

## Configuration

Provider config (`~/.openagents/marketplace.toml`):

```toml
[compute]
enabled = true
name = "My Compute Provider"
description = "Offering Llama3 and Mistral inference"
region = "us-west"
models = ["llama3", "mistral"]
pricing.per_1k_input_sats = 10
pricing.per_1k_output_sats = 20
schedule = "always"
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol"
]
readvertise_interval_secs = 3600  # 1 hour
```

## Example Flow

### Provider Setup

```bash
# 1. Configure provider
cargo marketplace provider config \
  --name "OpenAgents Provider" \
  --description "Fast Llama3 inference" \
  --region "us-west" \
  --capabilities llama3 \
  --price-input 10 \
  --price-output 20 \
  --relay wss://relay.damus.io \
  --relay wss://nos.lol

# 2. Go online (auto-advertises)
cargo marketplace provider online
# Output:
# Advertisement published: abc123...
# Provider is now ONLINE and advertising
# Next advertisement in: 1h 0m

# 3. Check status
cargo marketplace provider status
# Output:
# Status: ONLINE
# Capabilities: llama3
# Relays: 2
# Next advertisement in: 59m 45s
```

### Manual Advertisement

```bash
# Advertise without going online
cargo marketplace provider advertise

# View as JSON
cargo marketplace provider advertise --json
# {"event_id":"abc123...","relays":[...],"next_advertisement":3600}
```

### Consumer Discovery

```bash
# Consumers query for providers
cargo marketplace compute providers --model llama3
# Output:
# Provider: OpenAgents Provider (npub1...)
# Models: llama3
# Pricing: 10 sats/1k in, 20 sats/1k out
# Region: us-west
# Trust score: N/A (new provider)
```

## Related Files

- `crates/marketplace/src/compute/provider.rs` - Core implementation
- `crates/marketplace/src/cli/provider.rs` - CLI commands
- `crates/marketplace/src/relay.rs` - Relay connection
- `crates/marketplace/src/core/discovery.rs` - Consumer-side discovery
- `crates/nostr/core/src/nip89.rs` - NIP-89 event types
