# Two-Computer Agent Communication Instructions

## ACTIVE CHANNEL

**Channel ID:** `a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6`

**Relay:** `wss://relay.damus.io`

---

## Computer A (Provider) - THIS COMPUTER

Run:
```bash
cargo run --bin agent-provider -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6
```

---

## Computer B (Customer) - OTHER COMPUTER

Run:
```bash
cargo run --bin agent-customer -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --prompt "What is the meaning of life?"
```

---

## What Each Computer Does

### Provider (Computer A)
1. Joins the channel
2. Announces service (kind=5050, price=10000 msats)
3. Waits for job requests
4. Sends invoice when job requested
5. Delivers result after payment

### Customer (Computer B)
1. Joins the channel
2. Finds provider's service announcement
3. Sends job request with prompt
4. Pays invoice
5. Receives result

---

## Protocol Details

| Field | Value |
|-------|-------|
| Channel ID | `a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6` |
| Relay | `wss://relay.damus.io` |
| Provider Pubkey | `e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f` |
| Customer Pubkey | `ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd` |
| Job Kind | 5050 (NIP-90 text generation) |
| Price | 10000 msats (10 sats) |
| Network | `regtest` (Lightning regtest) |

### Network Field (NIP-89 Extension)

The `network` field in `ServiceAnnouncement` follows NIP-89 conventions for service
provider discoverability. This allows customers to filter providers by Lightning network
before requesting jobs, rather than discovering network mismatch only when parsing bolt11.

Valid networks: `mainnet`, `testnet`, `signet`, `regtest`

The customer validates that the provider's announced network matches expectations before
sending job requests. This prevents wasted effort on network mismatches.

---

## Options

### With Spark Wallet (real Bitcoin on regtest)
```bash
# Provider
cargo run --bin agent-provider -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6

# Customer
cargo run --bin agent-customer -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --prompt "Your question"
```

### Without Wallet (mock payments for testing)
```bash
# Provider
cargo run --bin agent-provider -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --no-wallet

# Customer
cargo run --bin agent-customer -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --prompt "Your question" --no-wallet
```

### With Streaming (real-time token delivery)
```bash
# Provider with streaming enabled
cargo run --bin agent-provider -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --no-wallet --stream

# Customer sees tokens as they arrive
cargo run --bin agent-customer -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --prompt "Write a haiku" --no-wallet
```

### With NIP-89 Global Discovery (no channel ID needed!)
```bash
# Provider creates channel and publishes NIP-89 handler info
cargo run --bin agent-provider -- --create-channel --no-wallet

# Customer discovers providers automatically via NIP-89 (kind 31990)
cargo run --bin agent-customer -- --discover --prompt "What is 2+2?" --no-wallet

# Customer options for provider selection
cargo run --bin agent-customer -- --discover --prompt "Question" --select cheapest --no-wallet
cargo run --bin agent-customer -- --discover --prompt "Question" --max-price 20000 --no-wallet
```

The provider publishes a NIP-89 handler info event (kind 31990) that includes:
- Handler type: `compute_provider`
- Capabilities: `text-generation`
- Pricing: 10000 msats per request
- Channel ID and relay URL for direct connection
- Available models

The customer queries for these events and automatically connects to discovered providers.

### With HTLC Escrow (trustless conditional payments - experimental)
```bash
# Provider (handles HtlcLocked and PreimageRelease messages)
cargo run --bin agent-provider -- --channel <ID> --no-wallet

# Customer with HTLC escrow mode
cargo run --bin agent-customer -- --channel <ID> --prompt "Question" --no-wallet --htlc
```

HTLC escrow flow:
1. Customer generates preimage and computes payment_hash
2. Customer sends HTLC payment (funds locked in escrow)
3. Customer sends `HtlcLocked` message to provider
4. Provider verifies HTLC and processes job
5. Provider delivers `JobResult`
6. Customer releases preimage via `PreimageRelease` message
7. Provider claims payment using preimage

This provides trustless payments - funds only release when the result is delivered.

---

## Wallet Mnemonics

| Agent | Mnemonic |
|-------|----------|
| Provider | `abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about` |
| Customer | `zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong` |

---

## Message Flow

```
Provider                          Customer
   │                                 │
   │──── ServiceAnnouncement ──────▶│
   │                                 │
   │◀──── JobRequest ───────────────│
   │                                 │
   │──── Invoice ──────────────────▶│
   │                                 │
   │◀──── PaymentSent ──────────────│
   │                                 │
   │──── JobResult ────────────────▶│
   │                                 │
```
