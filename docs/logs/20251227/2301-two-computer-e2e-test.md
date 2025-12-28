# Two-Computer Agent Communication E2E Test

**Date:** 2025-12-27 23:01 CST  
**Result:** SUCCESS

---

## Setup

| Computer | Role | Binary |
|----------|------|--------|
| A (Linux desktop) | Provider | `agent-provider` |
| B (MacBook) | Customer | `agent-customer` |

**Channel ID:** `a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6`  
**Relay:** `wss://relay.damus.io`

---

## Provider Startup (Computer A)

```
=== OpenAgents Provider Agent ===

[PROVIDER] Public key: e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f
[PROVIDER] Detecting inference backends...
[PROVIDER] Found backends: ["ollama"]
[PROVIDER] Available models: ["nemotron-3-nano:latest", "gpt-oss:120b", "gpt-oss:latest", "qwen3-coder:latest", "nomic-embed-text:latest", "qwen3:30b", "qwen3:latest"]
[PROVIDER] Default backend: ollama
[PROVIDER] Will use model: nemotron-3-nano:latest
[PROVIDER] Connecting to Spark wallet...
[PROVIDER] Wallet balance: 9649 sats
[PROVIDER] Connecting to relay: wss://relay.damus.io
[PROVIDER] Connected to relay
[PROVIDER] Joining channel: a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6
[PROVIDER] Service announced: kind=5050, price=10000 msats, network=regtest
[PROVIDER] Listening for job requests...
```

---

## Jobs Processed

### Job 1: Simple Geography Question

```
[PROVIDER] Got job request:
           Kind: 5050
           Prompt: What is the capital of France?
           Max tokens: 100
[PROVIDER] Invoice sent for job job_e51a1a58ebd3540a
[PROVIDER] Payment received for job_e51a1a58ebd3540a: mock-payment-id
[PROVIDER] Processing prompt: What is the capital of France?
[PROVIDER] Running inference with ollama...
[PROVIDER] Inference complete (31 tokens)
[PROVIDER] Result delivered for job_e51a1a58ebd3540a

[PROVIDER] Job complete! Waiting for more requests...
```

### Job 2: Information Theory Explanation

```
[PROVIDER] Got job request:
           Kind: 5050
           Prompt: Explain the relationship between entropy and information theory in 3 sentences
           Max tokens: 100
[PROVIDER] Invoice sent for job job_c42015d2d328ed47
[PROVIDER] Payment received for job_c42015d2d328ed47: mock-payment-id
[PROVIDER] Processing prompt: Explain the relationship between entropy and information theory in 3 sentences
[PROVIDER] Running inference with ollama...
[PROVIDER] Inference complete (142 tokens)
[PROVIDER] Result delivered for job_c42015d2d328ed47

[PROVIDER] Job complete! Waiting for more requests...
```

### Job 3: Complex Security Question

```
[PROVIDER] Got job request:
           Kind: 5050
           Prompt: We're building a decentralized compute marketplace where AI agents negotiate jobs and payments over Nostr using NIP-90 and Lightning. What are the main security considerations for preventing a provider from taking payment but not delivering results?
           Max tokens: 100
[PROVIDER] Invoice sent for job job_4cdf2124d724078f
[PROVIDER] Payment received for job_4cdf2124d724078f: mock-payment-id
[PROVIDER] Processing prompt: We're building a decentralized compute marketplace where AI agents negotiate jobs and payments over Nostr using NIP-90 and Lightning. What are the main security considerations for preventing a provider from taking payment but not delivering results?
[PROVIDER] Running inference with ollama...
[PROVIDER] Inference complete (256 tokens)
[PROVIDER] Result delivered for job_4cdf2124d724078f

[PROVIDER] Job complete! Waiting for more requests...
```

---

## Summary

| Job ID | Prompt | Tokens Generated | Status |
|--------|--------|------------------|--------|
| `job_e51a1a58ebd3540a` | What is the capital of France? | 31 | Delivered |
| `job_c42015d2d328ed47` | Entropy/information theory | 142 | Delivered |
| `job_4cdf2124d724078f` | Decentralized marketplace security | 256 | Delivered |

---

## Protocol Flow (Per Job)

```
Customer (Mac)                              Provider (Linux)
     │                                            │
     │                                            │── Service announced (kind=5050)
     │                                            │
     │──── JobRequest (prompt) ─────────────────▶│
     │                                            │
     │◀──── Invoice (10000 msats) ───────────────│
     │                                            │
     │──── PaymentSent ─────────────────────────▶│
     │                                            │
     │                                            │── ollama inference
     │                                            │
     │◀──── JobResult (response) ────────────────│
     │                                            │
```

---

## Technical Details

| Component | Value |
|-----------|-------|
| Provider Pubkey | `e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f` |
| Customer Pubkey | `ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd` |
| Inference Backend | ollama |
| Model | nemotron-3-nano:latest |
| Wallet Balance | 9649 sats |
| Job Kind | 5050 (NIP-90 text generation) |
| Price | 10000 msats (10 sats) |
| Network | regtest |
| Payment Mode | mock (regtest) |

---

## What Worked

1. **Nostr Channel Communication** - Both agents joined same channel via relay.damus.io
2. **Service Discovery** - Customer found provider's service announcement
3. **Job Request/Response** - Full NIP-90 DVM protocol working
4. **Invoice Generation** - Provider generated invoices for each job
5. **Payment Flow** - Mock payments processed correctly
6. **Real Inference** - ollama running nemotron-3-nano produced actual responses
7. **Result Delivery** - All 3 jobs completed and results delivered to customer

---

## Commands Used

### Computer A (Provider)
```bash
cargo run --bin agent-provider -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6
```

### Computer B (Customer)
```bash
cargo run --bin agent-customer -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --prompt "What is the capital of France?"

cargo run --bin agent-customer -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --prompt "Explain the relationship between entropy and information theory in 3 sentences"

cargo run --bin agent-customer -- --channel a7be6335515e15d3945619a227ab6cd3bfba3fd1b7d79d1708a06335a71112e6 --prompt "We're building a decentralized compute marketplace where AI agents negotiate jobs and payments over Nostr using NIP-90 and Lightning. What are the main security considerations for preventing a provider from taking payment but not delivering results?"
```

---

## Conclusion

Two-computer agent communication over Nostr with real local inference is fully operational. The NIP-90 DVM protocol enables agents on different machines to discover each other, negotiate jobs, exchange payments, and deliver AI inference results - all over a decentralized relay network.
