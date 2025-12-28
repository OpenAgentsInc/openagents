# Pylon Two-Computer Test Instructions

## Overview

This test validates Pylon's provider mode by having one computer run as a provider (earning sats) and another computer send job requests as a customer.

---

## Computer A (Provider) - THIS COMPUTER

### 1. Start Ollama

```bash
# Start Ollama server
ollama serve

# In another terminal, pull a model
ollama pull llama3.2
```

### 2. Initialize Pylon

```bash
cargo run -p pylon --bin pylon -- init
```

### 3. Verify Setup

```bash
cargo run -p pylon --bin pylon -- doctor
```

Expected output:
```
Identity:      ✅ OK
               npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7

Backends:
  ✅ Ollama (localhost:11434)
```

### 4. Start Provider

```bash
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f --mode provider
```

Expected output:
```
Starting Pylon in foreground mode...
Identity: npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7
Mode: Provider
Database opened at "/Users/.../.pylon/pylon.db"
Listening for NIP-90 jobs...
```

---

## Computer B (Customer) - OTHER COMPUTER

### Option A: Use agent-customer binary

```bash
# Discover providers and send job
cargo run --bin agent-customer -- \
  --discover \
  --prompt "What is the capital of France?" \
  --no-wallet
```

### Option B: Use agent-customer with specific provider

```bash
# Target specific provider npub
cargo run --bin agent-customer -- \
  --provider npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7 \
  --prompt "Explain quantum computing in one sentence" \
  --no-wallet
```

### Option C: Send raw NIP-90 event

Using any Nostr client that supports NIP-90:

1. Connect to relay: `wss://relay.damus.io`
2. Create kind:5100 event (text generation request)
3. Tag the provider: `["p", "<provider_hex_pubkey>"]`
4. Set content to your prompt

---

## Protocol Details

| Field | Value |
|-------|-------|
| Provider npub | `npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7` |
| Provider hex | `16dd3cf45416ae3de31264256d944539cd25077d104beb2ded078928010dbeb6` |
| Relay | `wss://relay.damus.io` |
| Job Kind | `5100` (text generation) |
| Result Kind | `6100` (text generation result) |
| Status Kind | `7000` (job status/feedback) |
| Backend | `ollama` with `llama3.2` |

---

## Expected Results

### On Provider (Computer A)

When a job is received:
```
Received job: abc123def456 (kind:5100)
Customer: npub1xyz...
Processing with ollama (llama3.2)...
Job completed in 1.2s
Result published to relay
```

### On Customer (Computer B)

```
Discovering providers on wss://relay.damus.io...
Found provider: npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7
Sending job request...
Waiting for result...
Result: The capital of France is Paris.
```

---

## Verify Earnings

After jobs complete, check provider earnings:

```bash
cargo run -p pylon --bin pylon -- earnings
```

Expected:
```
Pylon Earnings
==============

Summary:
  Total earned: X sats (X msats)
  Jobs completed: N

Recent Earnings (last 10):
SATS     SOURCE     TIME AGO
50       job        2 minutes
...
```

---

## Troubleshooting

### Provider not receiving jobs

1. Check relay connectivity:
   ```bash
   cargo run -p pylon --bin pylon -- doctor
   ```

2. Verify Ollama is running:
   ```bash
   curl http://localhost:11434/api/tags
   ```

3. Check logs with debug level:
   ```bash
   RUST_LOG=debug cargo run -p pylon --bin pylon -- start -f --mode provider
   ```

### Customer not finding provider

1. Ensure both computers use the same relay
2. Check provider is running and connected
3. Try with explicit provider npub instead of discovery

### Job fails

Check provider logs for error messages. Common issues:
- Model not pulled (`ollama pull llama3.2`)
- Ollama not responding
- Network timeout

---

## Message Flow

```
Customer                         Relay                          Provider
   │                               │                               │
   │── kind:5100 Job Request ─────▶│                               │
   │                               │◀── Subscribed to kind:5xxx ───│
   │                               │                               │
   │                               │─── Job Request ──────────────▶│
   │                               │                               │
   │                               │◀── kind:7000 Status ──────────│
   │◀─ "processing" ───────────────│                               │
   │                               │                               │
   │                               │         [ollama processing]   │
   │                               │                               │
   │                               │◀── kind:6100 Result ──────────│
   │◀─ Result ─────────────────────│                               │
   │                               │                               │
```

---

## Cleanup

Stop provider with `Ctrl+C` or:

```bash
cargo run -p pylon --bin pylon -- stop
```

---

## URGENT: Run This Now (Computer B)

```bash
cargo run --bin agent-customer -- \
  --provider npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7 \
  --prompt "What is 2+2?" \
  --no-wallet
```

Provider is LIVE on Computer A right now. Use the exact npub above.

---

## Test Results (2025-12-28 03:20 CST)

### Customer Output (Computer B - Linux)

```
=== OpenAgents Customer Agent ===

[CUSTOMER] Public key: ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd
[CUSTOMER] Running without wallet (--no-wallet)
[CUSTOMER] Connecting to relay: wss://relay.damus.io
[CUSTOMER] Connected to relay
[CUSTOMER] Discovering providers via NIP-89 (kind 31990)...
[CUSTOMER] Found 50 handler info events
[CUSTOMER] Skipping provider on unknown (we need regtest)
[CUSTOMER] Skipping provider on unknown (we need regtest)

[CUSTOMER] Discovered 1 provider(s) via NIP-89:
  [0] OpenAgents Compute Provider
      Pubkey: e8bcf3823669444d...
      Price: 10000 msats
      Channel: a7be6335515e15d3...
      Models: ["nemotron-3-nano:latest", "gpt-oss:120b", "gpt-oss:latest", "qwen3-coder:latest", "nomic-embed-text:latest", "qwen3:30b", "qwen3:latest"]

[CUSTOMER] Selected: OpenAgents Compute Provider (e8bcf3823669444d...)
[CUSTOMER] Using direct NIP-90 events (kind:5050 -> 7000 -> 6050)
[CUSTOMER] Job request published: cdda745147bfbc26b4cf0368fcd664571b8f71e842b06f72e605f74b0fa41486
[CUSTOMER] Waiting for provider response...
```

### Observations

1. **NIP-89 Discovery Working** - Customer found 50 handler info events on relay
2. **Network Filtering Working** - Skipped providers not on `regtest` network
3. **Found agent-provider** - Discovered the OpenAgents agent-provider (e8bcf38...) from earlier tests
4. **Did NOT find pylon provider** - The pylon provider (npub1zmwneaz...) was not discovered, likely because:
   - Pylon not running on Computer A
   - Pylon not announcing on `regtest` network
5. **Job Published** - Customer successfully published NIP-90 job request
6. **Timed Out** - No response because agent-provider wasn't running at the time

### Next Steps

To complete the pylon test, ensure pylon is running on Computer A:

```bash
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f --mode provider
```

Then run customer again:

```bash
cargo run --bin agent-customer -- --prompt "What is the capital of France?" --no-wallet
```

---

## Live Test Log (2025-12-28 03:35 CST)

### Provider Status (Computer A)

Provider is running:
- PID: 91735
- npub: `npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7`
- Relays: wss://relay.damus.io, wss://nos.lol
- Backend: ollama
- Network: regtest
- Handler info published: `238860fce5fcb82998252ee1620017e18107053bd9b451698c2828fd42720bc6`

Waiting for job requests...

### Instructions for Computer B

Run this command NOW:

```bash
cargo run --bin agent-customer -- \
  --provider npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7 \
  --prompt "What is 2+2?" \
  --no-wallet
```

---

## Test Run 2 (2025-12-28 03:45 CST)

### Customer Output (Computer B)

```
=== OpenAgents Customer Agent ===

[CUSTOMER] Public key: ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd
[CUSTOMER] Running without wallet (--no-wallet)
[CUSTOMER] Connecting to relay: wss://relay.damus.io
[CUSTOMER] Connected to relay
[CUSTOMER] Discovering providers via NIP-89 (kind 31990)...
[CUSTOMER] Found 50 handler info events
[CUSTOMER] Skipping provider on unknown (we need regtest)
[CUSTOMER] Skipping provider on unknown (we need regtest)

[CUSTOMER] Discovered 2 provider(s) via NIP-89:
  [0] OpenAgents Compute Provider
      Pubkey: 16dd3cf45416ae3d...  ← PYLON
      Price: 1000 msats
      Channel: none (direct events only)
      Models: []
  [1] OpenAgents Compute Provider
      Pubkey: e8bcf3823669444d...  ← agent-provider
      Price: 10000 msats
      Channel: a7be6335515e15d3...
      Models: ["nemotron-3-nano:latest", "gpt-oss:120b", ...]

[CUSTOMER] Selected: OpenAgents Compute Provider (16dd3cf45416ae3d...)
[CUSTOMER] Using direct NIP-90 events (kind:5050 -> 7000 -> 6050)
[CUSTOMER] Job request published: a6115aee9bb108ca45844c57866f6cc9a9b9cd321a48af7d24641032a5a46daa
[CUSTOMER] Waiting for provider response...
[TIMED OUT]
```

### Analysis

**What Worked:**
1. NIP-89 discovery found pylon provider (16dd3cf45416ae3d...)
2. Network filtering correctly identified regtest providers
3. Customer selected pylon (cheaper at 1000 msats vs 10000 msats)
4. Job request successfully published to relay

**What Failed:**
- Pylon did not respond to job request
- Customer timed out waiting for response

**Root Cause - Kind Mismatch:**

| Component | Job Kind | Result Kind |
|-----------|----------|-------------|
| agent-customer | 5050 | 6050 |
| pylon (per doc) | 5100 | 6100 |

The agent-customer sends `kind:5050` events, but pylon is listening for `kind:5100` events. This is a NIP-90 kind mismatch.

**NIP-90 Kind Reference:**
- 5050: Text-to-text (generic)
- 5100: Text generation (LLM inference)

**Fix Options:**
1. Update agent-customer to send kind:5100 for LLM jobs
2. Update pylon to also listen for kind:5050
3. Add kind negotiation based on provider's advertised capabilities

### Next Steps

Align event kinds between agent-customer and pylon, then retest.
