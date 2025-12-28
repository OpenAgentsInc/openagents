# Two-Computer Pylon Test - SUCCESS

**Date:** 2025-12-28 03:55 CST  
**Status:** FULLY OPERATIONAL

---

## Summary

Successfully demonstrated two-computer AI agent communication over Nostr using NIP-90 Data Vending Machine protocol. A customer agent on one computer can discover providers, send job requests, and receive AI inference results from a provider running on a different computer.

---

## Architecture

```
┌─────────────────────┐                         ┌─────────────────────┐
│   Computer B        │                         │   Computer A        │
│   (Linux Desktop)   │                         │   (MacBook)         │
│                     │                         │                     │
│   agent-customer    │                         │   pylon provider    │
│   - Discovers       │      Nostr Relay        │   - Listens for     │
│     providers       │◄────────────────────────►     job requests    │
│   - Sends jobs      │   wss://relay.damus.io  │   - Runs ollama     │
│   - Receives        │                         │   - Returns results │
│     results         │                         │                     │
└─────────────────────┘                         └─────────────────────┘
```

---

## Test Results

### Job 1: Simple Math
```
Prompt: "What is 2+2?"
Result: "2 + 2 = 4."
Status: SUCCESS
```

### Job 2: Complex Conceptual Question
```
Prompt: "Explain the relationship between entropy in thermodynamics and 
         information theory. How did Shannon's work connect these two 
         seemingly different fields?"

Result: "The relationship between entropy in thermodynamics and information 
        theory was a crucial connection made by Claude Shannon, an American 
        mathematician and electrical engineer. In his seminal work 'A 
        Mathematical Theory of Communication' (1948), Shannon established 
        the fundamental link between the concepts of entropy in both fields.

        In thermodynamics, entropy is a measure of the disorder or randomness 
        of a system. It represents the amount of thermal energy unavailable 
        to do work in a system...

        In information theory, entropy is used to quantify the uncertainty 
        or randomness of a message...

        Shannon's work connected these two seemingly different fields by 
        recognizing that both entropies are measures of the amount of 
        uncertainty or randomness in a system..."

Status: SUCCESS
```

---

## Protocol Flow

```
Customer                         Relay                          Provider
   │                               │                               │
   │── Subscribe kind:31990 ──────►│                               │
   │◄── NIP-89 Handler Info ───────│◄── Publish kind:31990 ────────│
   │                               │                               │
   │   [Discovery: found pylon]    │                               │
   │                               │                               │
   │── kind:5050 Job Request ─────►│                               │
   │                               │──── Job Request ─────────────►│
   │                               │                               │
   │                               │         [ollama inference]    │
   │                               │                               │
   │                               │◄──── kind:6050 Result ────────│
   │◄── Result ────────────────────│                               │
   │                               │                               │
   │   [Job complete!]             │                               │
```

---

## Technical Details

### Provider (Computer A - Mac)

| Field | Value |
|-------|-------|
| Binary | `pylon` |
| npub | `npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7` |
| Hex pubkey | `16dd3cf45416ae3de31264256d944539cd25077d104beb2ded078928010dbeb6` |
| Price | 1000 msats (1 sat) |
| Backend | ollama |
| Model | llama3.2 |
| Network | regtest |
| Relay | wss://relay.damus.io |

### Customer (Computer B - Linux)

| Field | Value |
|-------|-------|
| Binary | `agent-customer` |
| Hex pubkey | `ed6b4c4479c2a9a74dc2fb0757163e25dc0a4e13407263952bfc6c56525f5cfd` |
| Wallet | disabled (--no-wallet) |
| Network filter | regtest |
| Relay | wss://relay.damus.io |

### NIP-90 Event Kinds

| Kind | Purpose |
|------|---------|
| 31990 | NIP-89 Handler Info (provider discovery) |
| 5050 | Job Request (text generation) |
| 6050 | Job Result |
| 7000 | Job Status/Feedback |

---

## Commands

### Start Provider (Computer A)
```bash
RUST_LOG=info cargo run -p pylon --bin pylon -- start -f --mode provider
```

### Run Customer (Computer B)
```bash
cargo run --bin agent-customer -- --prompt "Your question here" --no-wallet
```

### With Specific Provider
```bash
cargo run --bin agent-customer -- \
  --select npub1zmwneaz5z6hrmccjvsjkm9z988xj2pmazp97kt0dq7yjsqgdh6mqg95wu7 \
  --prompt "Your question" \
  --no-wallet
```

---

## What's Working

1. **NIP-89 Provider Discovery** - Customer discovers providers via kind:31990 events
2. **Network Filtering** - Only shows providers on matching network (regtest)
3. **Provider Selection** - Selects cheapest provider (1000 msats vs 10000 msats)
4. **Job Request Publishing** - Customer publishes kind:5050 job requests
5. **Cross-Computer Communication** - Events flow between machines via relay
6. **Real LLM Inference** - Pylon runs ollama with llama3.2
7. **Result Delivery** - Provider returns kind:6050 results
8. **Complex Prompts** - Handles multi-sentence conceptual questions

---

## Issues Resolved

### Kind Mismatch (Fixed)
- **Problem:** agent-customer sent kind:5050, pylon expected kind:5100
- **Solution:** Updated pylon to listen for kind:5050 (text-to-text generic)

### Provider Not Responding (Fixed)
- **Problem:** Pylon wasn't subscribed to job request events
- **Solution:** Added proper REQ subscription for kind:5050 events

---

## What's Next

1. **Real Payments** - Enable Lightning payments instead of --no-wallet
2. **Multiple Providers** - Test provider selection with 3+ providers
3. **Streaming Results** - Implement kind:7000 status updates during inference
4. **HTLC Escrow** - Trustless payment-for-result atomic swaps
5. **Model Selection** - Customer specifies preferred model in job request

---

## Conclusion

The OpenAgents two-computer agent communication infrastructure is fully operational. Agents can:
- Discover each other via NIP-89 on Nostr relays
- Exchange job requests and results via NIP-90 DVM protocol
- Run real AI inference on remote machines
- Filter by network and select by price

This is the foundation for a decentralized AI compute marketplace where agents negotiate and pay for services autonomously.
