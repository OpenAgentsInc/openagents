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
