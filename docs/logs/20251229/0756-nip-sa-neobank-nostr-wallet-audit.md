# Audit: NIP-SA, Neobank, Nostr, Wallet

Date: 2025-12-29
Scope: NIP-SA protocol, neobank treasury and exchange, nostr core/client/relay usage, wallet CLI and Spark integration.
Goal: Validate readiness for sovereign agents to go live on multiple relays, communicate via NIP-28 and DMs, and pay each other.

## Executive summary
- NIP-SA core types and encryption exist, and the runner publishes state and trajectories, but profile, schedule, and tick events are not published, and multi-relay is not wired.
- Nostr core/client/relay implementations are strong, including RelayPool, but NIP-SA and compute runners use single-relay connections.
- NIP-28 messaging works in compute flows and tests, but it is not used for general sovereign-agent communication.
- Wallet CLI supports Spark payments and NIP-04 DMs on multiple relays; NIP-SA runner cannot decrypt inbound DMs and zaps are stubbed.
- Neobank core has Cashu wallet and exchange logic, but settlement and relay integration are mock or cache-only, and Pylon daemon does not wire neobank commands.

## Findings (ordered by severity)

### Blockers
- Multi-relay is not wired for sovereign agent runner or compute components. The runner uses single `RelayConnection` instances. RelayPool exists but is unused. Evidence: `src/bin/agent_runner.rs`, `src/agents/runner/state.rs`, `src/agents/runner/compute.rs`, `src/agents/runner/scheduler.rs`, `src/agents/runner/trajectory.rs`, `crates/nostr/client/src/pool.rs`.
- DM reception is not usable for agents. The scheduler listens for kind 4 but the tick executor never decrypts DM content; only outbound NIP-04 send is implemented. Evidence: `src/agents/runner/scheduler.rs`, `src/agents/runner/tick.rs`.
- NIP-57 zaps in the sovereign agent runner are stubbed (no lnurl fetch, invoice creation, or payment). This blocks agent-to-agent payments via zap in NIP-SA. Evidence: `src/agents/runner/tick.rs`.
- Neobank is not available end-to-end. Pylon daemon returns "Neobank not initialized" for neobank commands, exchange uses mock settlement, relay fetch is cache-only, RFQ relay publishing is TODO. Evidence: `crates/pylon/src/cli/start.rs`, `crates/neobank/src/exchange.rs`, `crates/neobank/src/settlement.rs`, `crates/neobank/src/relay.rs`, `crates/neobank/src/rfq.rs`.

### High
- NIP-SA runner does not publish AgentProfile, AgentSchedule, or TickRequest/TickResult events. Only state and trajectory events are published. Schedule config is local and not driven by relay events. Evidence: `src/agents/runner/state.rs`, `src/agents/runner/trajectory.rs`, `src/agents/runner/scheduler.rs`.
- NIP-SA state encryption uses the agent private key directly; the spec describes threshold ECDH/marketplace signer gating, which is not integrated. Evidence: `src/agents/runner/state.rs`, `crates/nostr/core/src/nip_sa/state.rs`.
- NIP-28 channel messaging exists for compute flows but not for sovereign-agent communication. There is no NIP-28 join or message flow in the NIP-SA runner. Evidence: `src/agents/runner/compute.rs`, `src/bin/agent_provider.rs`, `src/bin/agent_customer.rs`.

### Medium
- Wallet DMs use NIP-04 only. NIP-44 DM is not exposed, and NIP-17 is not integrated into runtime flows. Evidence: `crates/wallet/src/cli/identity.rs`, `crates/nostr/core/src/nip04.rs`, `crates/nostr/core/src/nip44.rs`, `crates/nostr/core/src/nip17.rs`.
- NIP-SA wallet balance is not synced from Spark wallet. `update_wallet_balance` is never called, so lifecycle and budget decisions use stale balances. Evidence: `crates/nostr/core/src/nip_sa/wallet_integration.rs`, `src/agents/runner/tick.rs`.
- NIP-28 job payloads are plaintext JSON; encrypted payloads via NIP-04/44 are documented as TODO. Evidence: `docs/marketplace-v2.md`.

### Low
- Some integration tests cover NIP-SA and NIP-28 flows but are local-relay only or ignored by default. Evidence: `crates/nostr/core/tests/nip_sa_e2e.rs`, `crates/nostr/tests/integration/nip_sa.rs`, `crates/nostr/client/tests/agent_chat_e2e.rs`.

## Capability status vs go-live requirements

1) Multiple relays
- Have: RelayPool and a multi-relay wallet client. Evidence: `crates/nostr/client/src/pool.rs`, `crates/wallet/src/core/client.rs`.
- Missing: NIP-SA runner and compute client use single `RelayConnection`. Evidence: `src/bin/agent_runner.rs`, `src/agents/runner/compute.rs`.

2) Agent communication via NIP-28
- Have: NIP-28 types and helpers in core; compute flows can use channels. Evidence: `crates/nostr/core/src/nip28.rs`, `src/agents/protocol.rs`, `src/bin/agent_provider.rs`, `src/bin/agent_customer.rs`.
- Missing: NIP-SA runner does not join or publish to channels for general agent chat. Evidence: `src/agents/runner/*`.

3) Agent communication via DMs
- Have: NIP-04 encrypt/decrypt in core; wallet CLI can send/list/read DMs on multiple relays. Evidence: `crates/nostr/core/src/nip04.rs`, `crates/wallet/src/cli/identity.rs`.
- Missing: NIP-SA runner cannot decrypt inbound DMs; no NIP-44 DM path; no NIP-17 integration. Evidence: `src/agents/runner/tick.rs`.

4) Agent payments
- Have: Spark wallet integration used by compute flows and wallet CLI. Evidence: `crates/spark/src/wallet.rs`, `src/agents/runner/compute.rs`, `crates/wallet/src/cli/bitcoin.rs`.
- Missing: NIP-SA runner has no direct agent-to-agent payment flow; zap flow is stub and there is no invoice exchange or payment confirmation in sovereign-agent logic. Evidence: `src/agents/runner/tick.rs`.
- Missing: Neobank exchange and settlement are mock or cache-only; not wired to Pylon daemon. Evidence: `crates/neobank/src/exchange.rs`, `crates/neobank/src/settlement.rs`, `crates/pylon/src/cli/start.rs`.

## What works today
- Sovereign agent runner can publish encrypted NIP-SA state and NIP-SA trajectory events on a single relay and pay for NIP-90 compute using Spark. Evidence: `src/bin/agent_runner.rs`, `src/agents/runner/state.rs`, `src/agents/runner/trajectory.rs`, `src/agents/runner/compute.rs`.
- NIP-28 channels and NIP-90 compute flow are usable via `agent-provider` and `agent-customer` binaries (single relay) with optional channel messaging. Evidence: `src/bin/agent_provider.rs`, `src/bin/agent_customer.rs`.
- Wallet CLI supports Nostr profile management, NIP-04 DMs, Lightning send/receive/invoice, zaps, and NWC using Spark. Evidence: `crates/wallet/src/cli/*`.

## Open questions / assumptions
- I assumed "DM spec" refers to NIP-04 or NIP-44. If you require NIP-17, it is not wired into any runtime paths yet.

## Changes implemented after audit
- Added a multi-relay RelayHub/RelayApi abstraction and updated protocol helpers + runner components to use shared relay pools with unique subscriptions.
- Agent runner now accepts multiple `--relay` values, optional `--channel`, and publishes AgentProfile/AgentSchedule on startup via the shared relay hub (including channel trigger when configured).
- Tick executor now refreshes wallet balance, publishes NIP-SA tick request/result events, decrypts inbound DMs (NIP-04 + NIP-44), and formats channel/DM observations in prompts.
- Added action parsing/execution for DMs, channel messages, invoice payments, and payment requests; implemented NIP-57 zap flow (LNURL lookup + invoice pay).
- Compute client now exposes wallet balance refresh and invoice create/pay helpers for agent-to-agent payments.
- Pylon daemon now initializes neobank service on demand and wires balance/pay/send/receive/status commands.
