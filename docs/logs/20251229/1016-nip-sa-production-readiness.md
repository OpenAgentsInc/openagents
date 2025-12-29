# NIP-SA Production Readiness Plan

Date: 2025-12-29
Owner: OpenAgents team
Scope: Sovereign agent runtime (NIP-SA), Nostr relay connectivity, DM/channel comms, payments, and neobank wiring.

## Goal
Make the NIP-SA flow safe and reliable for production: multi-relay operation, agent communication, and payments with strong correctness and operational guardrails.

## Required work (must-do)

### 1) Relay + protocol hardening
- Multi-relay support for all binaries that participate in NIP-90 or NIP-SA (agent runner, provider, customer).
- Unique subscription IDs for all subscriptions.
- EOSE handling, timeouts, and retry/backoff (per relay).
- NIP-65 relay list support and dynamic relay set updates.
- Confirm publish success across a quorum of relays (define minimum confirmations).

### 2) DM and channel messaging
- Correct public key handling for NIP-04/NIP-44 encryption and decryption.
- Outbound NIP-44 support (with explicit selection or capability detection).
- Channel message ingestion for agent triggers + observation context.
- Structured DM format for invoices and payment confirmations.

### 3) NIP-SA event lifecycle
- Publish AgentProfile and AgentSchedule on startup (done in runner).
- Publish TickRequest and TickResult events for every tick (done in runner).
- Consume schedule/profile updates from relays (not just local config).
- Add NIP-SA history queries (tick timeline aggregation) for debugging.

### 4) Payment flow correctness
- NIP-57 zap flow: LNURL discovery, invoice request, pay, and verify receipt.
- Explicit invoice request/response flow between agents (DM or channel).
- Record payment confirmations into state (budget + lifecycle).
- Avoid double-payments with idempotency and event IDs.

### 5) Security and key management
- Encrypt mnemonic at rest (user passphrase or OS keystore).
- NIP-42 relay auth for private relays.
- Separate keys for identity vs payments if required by policy.
- Log redaction for sensitive data (in prompts and trajectories).

### 6) Observability + resilience
- Structured logging with relay context.
- Metrics for tick duration, publish latency, payment attempts, relay failures.
- Circuit breakers and automatic relay failover.
- Crash recovery and graceful shutdown.

### 7) Testing + validation
- Multi-relay integration tests for DMs, channels, and tick events.
- End-to-end tests for invoice request + pay + confirmation.
- Simulation tests for relay failure and delayed relays.
- Documented manual playbooks for production smoke testing.

## Nice-to-have (after must-do)
- Support NIP-17 (if required by product).
- Add NIP-59/secure group workflows for channel comms.
- Support NIP-44-only relays/clients detection.
- Multi-agent coordination tests across separate worktrees.

## Release checklist
- All must-do items above complete and verified.
- `cargo check -p openagents` passes.
- At least one full end-to-end test run across multiple relays.
- Logging + metrics validated on staging relay set.
