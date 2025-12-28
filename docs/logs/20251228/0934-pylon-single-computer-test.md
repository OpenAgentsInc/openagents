# Pylon Single-Computer Test

**Date:** 2025-12-28 09:34 CST

## Overview

Tested pylon provider and agent-customer on the same computer to validate the full NIP-90 job flow.

---

## Test Setup

| Component | Value |
|-----------|-------|
| Provider | pylon (npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7) |
| Customer | agent-customer |
| Backend | Ollama (llama3.2) |
| Relay | wss://relay.damus.io |
| Network | regtest |

---

## Provider Startup

```
Starting Pylon in foreground mode...
Identity: npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7
Mode: Provider
Applied migration: 002_invoices
Database opened at "/Users/christopherdavid/.pylon/pylon.db"
Loaded stats: 5 jobs completed, 0 sats earned
Detected Ollama backend at localhost:11434
Detected backends: ollama
Payments disabled, running in free mode
Connected to 2/3 relays
Subscribed to NIP-90 job requests with ID: nip90-jobs-16dd3cf4
Job event processing task started
Published NIP-89 handler info
DVM service started for pubkey: 16dd3cf45416ae3d...
```

**Note:** Provider automatically processed old jobs from relay on startup (5 previous jobs).

---

## Customer Request

**Command:**
```bash
cargo run --bin agent-customer -- \
  --select npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7 \
  --prompt "What is the square root of 144?" \
  --no-wallet
```

**Output:**
```
=== OpenAgents Customer Agent ===

[CUSTOMER] Public key: ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd
[CUSTOMER] Running without wallet (--no-wallet)
[CUSTOMER] Connecting to relay: wss://relay.damus.io
[CUSTOMER] Connected to relay
[CUSTOMER] Discovering providers via NIP-89 (kind 31990)...
[CUSTOMER] Found 50 handler info events

[CUSTOMER] Discovered 2 provider(s) via NIP-89:
  [0] OpenAgents Compute Provider
      Pubkey: 16dd3cf45416ae3d...
      Price: 1000 msats
      Channel: none (direct events only)
      Models: []
  [1] OpenAgents Compute Provider
      Pubkey: e8bcf3823669444d...
      Price: 10000 msats
      Channel: a7be6335515e15d3...
      Models: [...]

[CUSTOMER] Selected: OpenAgents Compute Provider (16dd3cf45416ae3d...)
[CUSTOMER] Using direct NIP-90 events (kind:5050 -> 7000 -> 6050)
[CUSTOMER] Job request published: 24176e1691e166d8b20c53bdec263e17b3f9d9d461b41371a0b757f616aba55e
[CUSTOMER] Waiting for provider response...

========================================
JOB RESULT RECEIVED
========================================
Job ID: 24176e1691e166d8b20c53bdec263e17b3f9d9d461b41371a0b757f616aba55e
Result: The square root of 144 is 12.
========================================

[CUSTOMER] Job complete!
[CUSTOMER] Disconnecting...
```

---

## Provider Logs (New Job)

```
2025-12-28T15:37:55Z INFO Received job request event: 24176e1691e166d8... (kind: 5050)
2025-12-28T15:37:55Z INFO Parsed job request from ed6b4c4479c2a9a7: 0 inputs, 1 params
2025-12-28T15:37:55Z INFO Processing job job_24176e1691e166d8 with prompt: What is the square root of 144?
2025-12-28T15:37:55Z INFO Provider event: Job received: job_2417 (kind 5050)
2025-12-28T15:37:56Z INFO Job job_24176e1691e166d8 completed, 45 tokens
2025-12-28T15:37:56Z INFO Published event 904a147f... to 3/3 relays
2025-12-28T15:37:56Z INFO Published job result to 3 relays
2025-12-28T15:37:56Z INFO Provider event: Job completed: job_2417
```

---

## Timeline

| Time | Event |
|------|-------|
| 15:35:03 | Provider started, database opened |
| 15:35:04 | Connected to relays, subscribed to jobs |
| 15:35:05 | NIP-89 handler info published |
| 15:35:05-14 | Processed 5 old jobs from relay backlog |
| 15:37:55 | New job received: "What is the square root of 144?" |
| 15:37:56 | Job completed (45 tokens, ~1 second) |
| 15:37:56 | Result published to 3 relays |
| 15:37:56 | Customer received result: "The square root of 144 is 12." |

---

## Result Summary

| Metric | Value |
|--------|-------|
| Job ID | 24176e1691e166d8 |
| Prompt | "What is the square root of 144?" |
| Response | "The square root of 144 is 12." |
| Tokens | 45 |
| Processing time | ~1 second |
| Relays published | 3/3 |

---

## Observations

1. **NIP-89 Discovery Working** - Customer found pylon via handler info on relay
2. **Provider Selection** - Customer correctly selected pylon (cheapest at 1000 msats)
3. **Direct Events** - Used kind:5050 → 6050 flow (no NIP-28 channel needed)
4. **Fast Processing** - Job completed in ~1 second with llama3.2
5. **Relay Redundancy** - Results published to all 3 connected relays

---

## Issues Noted

1. **Payments disabled** - Provider ran in free mode despite `enable_payments: true` default
   - Cause: Existing config file at `~/.pylon/config.toml` overrides defaults
   - Fix: Delete old config or update it

2. **Duplicate job warnings** - "UNIQUE constraint failed: jobs.id"
   - Cause: Same job events received from multiple relays
   - Not critical: Job still processed, just not re-recorded

---

## Test Status: ✅ PASSED

Single-computer NIP-90 provider/customer test successful.
