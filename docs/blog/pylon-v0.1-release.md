# Pylon v0.1: Regtest Alpha

Today we're releasing Pylon v0.1, the first version of node software that connects your compute to the global AI marketplace via Nostr.

## What is Pylon?

Pylon is node software that makes your compute available on an open market. It uses Nostr's NIP-90 protocol for job coordination and includes a built-in Bitcoin wallet (Spark/Lightning) so you get paid for inference jobs.

The vision: millions of edge devices running local inference, connected into a decentralized compute network where anyone can buy or sell AI processing power.

## The v0.1 Flow

Here's what works today:

```
Terminal 1 (Provider):
$ pylon init                           # create Nostr identity
$ pylon wallet fund                    # get regtest sats from faucet
$ pylon start -f -m provider           # run as inference provider

Terminal 2 (Buyer):
$ pylon job submit "What is 2+2?" --auto-pay
```

The provider receives the job request, creates a Lightning invoice, publishes payment-required feedback. The buyer auto-pays the invoice via Spark wallet. Payment flows through the network.

## Key Features

### NIP-90 Data Vending Machine

Pylon implements the NIP-90 DVM protocol:
- **Provider mode:** Host inference backends and earn Bitcoin
- **Buyer mode:** Submit jobs and pay via Lightning
- Broadcast job discovery across multiple relays

### Inference Backends

Pylon auto-detects backends at startup. Works on **any platform** with Ollama or llama.cpp:

| Backend | Platform | Notes |
|---------|----------|-------|
| **Ollama** | Any (Linux, macOS, Windows) | Run `ollama serve` on :11434 |
| **llama.cpp** | Any | Run `llama-server` on :8080 |
| **Apple FM** | macOS only | Auto-starts FM Bridge if available |
| **GPT-OSS Metal** | macOS only | Embedded, needs model.bin |

### Multi-Relay Architecture

Pylon connects to multiple Nostr relays simultaneously:
- `wss://nexus.openagents.com` (our agent-centric relay)
- `wss://relay.damus.io`
- `wss://nos.lol`

Jobs submitted to any relay reach all connected providers.

### NIP-42 Authentication

Automatic authentication with relays that require it. Nexus requires NIP-42 auth for all operations - Pylon handles the AUTH challenge/response automatically.

## What's Nexus?

Nexus is our agent-centric Nostr relay, optimized for machine-speed coordination. While any NIP-90 compatible relay works with Pylon, Nexus is designed specifically for the high-frequency event patterns of AI agent commerce.

It's deployed at `nexus.openagents.com` and requires authentication. See the [Nexus v0.1 release notes](/blog/nexus-v0.1-release) for details.

## CLI Reference

```bash
pylon init                    # Create Nostr identity
pylon wallet fund             # Get regtest sats
pylon wallet balance          # Check balance
pylon start -f -m provider    # Start as provider
pylon job submit "prompt"     # Submit a job
pylon job submit "prompt" --auto-pay  # Submit and auto-pay
pylon rlm "query"             # Run RLM query across swarm
pylon rlm "query" --local-only  # Run RLM locally
pylon infer --prompt "Hello"  # Local inference test
pylon doctor                  # Check system status
```

## RLM: Recursive Queries Across the Network

The killer feature: run recursive language model queries that fan out to providers.

```bash
# Simple query - fans out to swarm providers
pylon rlm "What is the best approach for implementing auth?"

# Analyze a document - chunks it and processes in parallel
pylon rlm "Summarize the key points" --file paper.pdf --fanout 20

# Local only (no network, uses your Apple FM/Ollama)
pylon rlm "Explain this code" --file main.rs --local-only
```

This is the [RLM paper](https://openagents.com/recursive-language-models) in practice: one coordinating model breaks down tasks, fans out to the swarm, and synthesizes results.

Run as a provider to earn sats. Submit queries to use other providers. The more people running pylon, the more powerful the network becomes.

## What's Next

This is a regtest alpha. Real money support (testnet, then mainnet) is coming. We're also working on:

- Payment detection and result delivery
- `pylon earnings` command for provider stats
- Automatic backend selection
- Retry logic for failed jobs

## Get Started

Clone the repo and build:

```bash
git clone https://github.com/OpenAgentsInc/openagents
cd openagents
cargo build -p pylon
./target/debug/pylon init
./target/debug/pylon wallet fund
./target/debug/pylon start -f -m provider
```

## Why This Matters

This is infrastructure for the next phase of AI. Instead of a few cloud providers controlling compute access, we're building a decentralized marketplace where:

- Anyone can become a compute provider
- Inference runs on the edge, close to users
- Payment is instant via Lightning
- No rate limits, no centralized gatekeepers

Combined with recursive language models (see [Episode 202](/recursive-language-models)), this architecture enables a new class of AI applications that can fan out to thousands of providers simultaneously.

The swarm compute network starts here.
