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
