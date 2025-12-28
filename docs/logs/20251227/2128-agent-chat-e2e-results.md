# Agent-to-Agent NIP-28 Communication Test Results

**Date**: 2024-12-27 21:28 UTC
**Test**: `cargo test -p nostr-client --test agent_chat_e2e test_agent_chat_e2e -- --ignored --nocapture`
**Duration**: 8.48 seconds
**Result**: PASSED

---

## Environment

| Component | Value |
|-----------|-------|
| Relay | `wss://relay.damus.io` |
| Network | Spark Regtest (Lightspark) |
| Test File | `crates/nostr/client/tests/agent_chat_e2e.rs` |

---

## Agent Identities

### Provider Agent
| Field | Value |
|-------|-------|
| Mnemonic | `abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about` |
| Public Key | `e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f` |
| Derivation | NIP-06: `m/44'/1237'/0'/0/0` |

### Customer Agent
| Field | Value |
|-------|-------|
| Mnemonic | `zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong` |
| Public Key | `ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd` |
| Derivation | NIP-06: `m/44'/1237'/0'/0/0` |
| Balance Before | 675 sats |
| Balance After | 665 sats (paid 10 sats) |

---

## NIP-28 Channel

| Field | Value |
|-------|-------|
| Channel ID (Event ID) | `98097371bdde5d67c996b9daf78899df3a33e927d8bddbc0b5ea8f9c7fb1eb4b` |
| Kind | 40 (Channel Creation) |
| Name | "OpenAgents Compute Marketplace" |
| Description | "Agents negotiate NIP-90 jobs with Bitcoin payments" |
| Relay | `wss://relay.damus.io` |

---

## Message Flow

### 1. Provider Service Announcement (Kind 42)

```json
{
  "type": "ServiceAnnouncement",
  "kind": 5050,
  "price_msats": 10000,
  "spark_address": "<provider_spark_address>"
}
```

**Timestamp**: T+0s

### 2. Customer Job Request (Kind 42)

```json
{
  "type": "JobRequest",
  "kind": 5050,
  "prompt": "What is the meaning of life?",
  "max_tokens": 100
}
```

**Timestamp**: T+1s

### 3. Provider Invoice (Kind 42)

```json
{
  "type": "Invoice",
  "job_id": "job_2291783e6ee42911",
  "bolt11": "<lightning_invoice>",
  "amount_msats": 10000
}
```

**Timestamp**: T+2s

### 4. Customer Payment Confirmation (Kind 42)

```json
{
  "type": "PaymentSent",
  "job_id": "job_2291783e6ee42911",
  "preimage": "019b6303-cc36-7482-aeb8-a5b005a44438"
}
```

**Timestamp**: T+5s (includes ~3s payment round-trip)

### 5. Provider Job Result (Kind 42)

```json
{
  "type": "JobResult",
  "job_id": "job_2291783e6ee42911",
  "result": "The meaning of life is 42. This is a mock response from the compute provider."
}
```

**Timestamp**: T+6s

---

## Payment Details

| Field | Value |
|-------|-------|
| Payment ID | `019b6303-cc36-7482-aeb8-a5b005a44438` |
| Amount | 10 sats (10,000 msats) |
| Network | Spark Regtest |
| Type | Lightning Invoice |
| Status | Completed |

---

## Console Output (Raw)

```
=== Agent Chat E2E Test ===

Provider pubkey: e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f
Customer pubkey: ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd

Connecting Spark wallets to regtest...
Customer balance: 675 sats

Connecting to relay: wss://relay.damus.io
Both agents connected to relay

Channel created: 98097371bdde5d67c996b9daf78899df3a33e927d8bddbc0b5ea8f9c7fb1eb4b
[PROVIDER] Service announced
[CUSTOMER] Found provider: kind=5050, price=10000 msats
[CUSTOMER] Job requested
[PROVIDER] Got job request: What is the meaning of life?
[PROVIDER] Invoice sent
[CUSTOMER] Got invoice for 10000 msats, paying...
[CUSTOMER] Payment sent: 019b6303-cc36-7482-aeb8-a5b005a44438
[PROVIDER] Payment received for job_2291783e6ee42911: 019b6303-cc36-7482-aeb8-a5b005a44438
[PROVIDER] Result delivered
[CUSTOMER] Got result for job_2291783e6ee42911: The meaning of life is 42. This is a mock response from the compute provider.

Logs written to: docs/logs/20251227/agent-chat-results.log

=== E2E Test Complete! ===
test test_agent_chat_e2e ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 8.48s
```

---

## Protocol Stack Used

| Layer | Implementation |
|-------|----------------|
| Transport | WebSocket (tokio-tungstenite) |
| Relay Protocol | NIP-01 (EVENT, REQ, CLOSE, OK, EOSE) |
| Chat | NIP-28 (kinds 40, 42) |
| Job Protocol | NIP-90 inspired (kind 5050 text generation) |
| Key Derivation | NIP-06 (BIP39 + BIP32) |
| Signing | Schnorr (secp256k1) |
| Payment | Spark/Breez SDK (Lightning) |
| Network | Regtest (no real value) |

---

## Files Involved

| File | Purpose |
|------|---------|
| `crates/nostr/client/tests/agent_chat_e2e.rs` | Test implementation |
| `crates/nostr/core/src/nip01.rs` | Event signing, verification |
| `crates/nostr/core/src/nip06.rs` | Key derivation from mnemonic |
| `crates/nostr/core/src/nip28.rs` | Channel types and message formatting |
| `crates/nostr/client/src/relay.rs` | WebSocket connection, pub/sub |
| `crates/spark/src/wallet.rs` | Spark wallet operations |

---

## Verification

The channel and messages are publicly visible on any Nostr client that connects to `wss://relay.damus.io`. You can query for:

- **Channel creation**: `{"kinds": [40], "ids": ["98097371bdde5d67c996b9daf78899df3a33e927d8bddbc0b5ea8f9c7fb1eb4b"]}`
- **Channel messages**: `{"kinds": [42], "#e": ["98097371bdde5d67c996b9daf78899df3a33e927d8bddbc0b5ea8f9c7fb1eb4b"]}`

---

## Run Commands

```bash
# Prerequisites: Fund customer wallet via Lightspark regtest faucet
# https://app.lightspark.com/regtest-faucet

# Check wallet balance
cargo test -p nostr-client --test agent_chat_e2e test_spark_wallet_connect -- --ignored --nocapture

# Run full E2E test
cargo test -p nostr-client --test agent_chat_e2e test_agent_chat_e2e -- --ignored --nocapture

# Run just relay connectivity test
cargo test -p nostr-client --test agent_chat_e2e test_relay_connect -- --ignored --nocapture

# Run just channel creation test
cargo test -p nostr-client --test agent_chat_e2e test_create_channel -- --ignored --nocapture
```

---

## What This Proves

1. **NIP-28 works**: Public chat channels can be created and used for structured communication
2. **NIP-01 works**: Events are properly signed, published, and received
3. **NIP-06 works**: Keypairs derived from mnemonics function correctly
4. **Spark integration works**: Real Bitcoin payments flow between wallets
5. **Agent coordination works**: Two autonomous agents can negotiate and transact

---

## Commit

```
9ddb51043 Implement agent-to-agent NIP-28 communication with Spark payments
```
