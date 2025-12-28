# OpenAgents Neobank

**Treasury OS for humans and agent fleets.**

The neobank crate provides programmable treasury management, multi-rail payment routing, and budget enforcement for autonomous AI agents operating on Bitcoin.

## Overview

OpenAgents Neobank is not a bank. It's a **programmable treasury + payments router** for agents:

- **Self-custody by default** — Agent/human keys protected by FROST 2-of-3 threshold signatures
- **Stable unit-of-account** — Support for stablecoins where budgets and pricing need predictability
- **Multi-rail routing** — Intelligent routing across BTC Lightning, Taproot Assets, and eCash
- **Budget enforcement** — Per-agent daily caps, per-task limits, approval workflows
- **Auditable receipts** — Every payment links to trajectory logs and cryptographic proofs

## Money Rails

### Rail A: Native Bitcoin (Baseline)

For machine-to-machine micropayments, censorship resistance, and settlement finality.

- **Lightning** — NIP-57 zaps, L402 pay-per-call APIs
- **On-chain BTC** — Treasury settlement, large transfers, long-term reserves
- **Integration**: `crates/spark` (Breez SDK)

### Rail B: Stablecoins via Taproot Assets

For stable budgeting, enterprise procurement, payroll, and predictable pricing.

- Hold/send/receive Taproot Assets on-chain and over Lightning
- Track assets by `group_key` for fungibility across issuance batches
- Use `AddressV2` for reusable stablecoin receive addresses
- Support for USDT-on-Taproot-Assets (and future stablecoins)

### Rail C: eCash (Cashu + Fedimint)

For privacy, cash-like UX, tips, and low-trust small-value payments.

- **NIP-87** — Mint discovery and reputation
- **NIP-60** — Wallet state portability (relay-synced)
- **NIP-61** — Nutzaps (payment-as-receipt)

## Account Model

### Entities

| Entity | Role |
|--------|------|
| **Human Operator** | Funds the system, sets policy, reviews exceptions |
| **Agent** | Autonomous actor with identity, wallets, and budget |
| **Guardian** | Recovery/safety signer (threshold share) |
| **Policy Signer** | Marketplace/compliance signer enforcing constraints |

### Accounts (Wallet Partitions)

| Account | Purpose |
|---------|---------|
| **Treasury** | Org-level long-term holdings, top-ups, reserves |
| **Operating** | Per-agent or per-workload day-to-day spending |
| **Escrow** | Pay-after-verify, disputes |
| **Payroll/Rewards** | Bounties, skill revenue splits, contributor payouts |

## Key Management

Default 2-of-3 FROST topology per account:

- **Share A**: Agent runtime enclave (or agent's secure module)
- **Share B**: Policy signer (enforces budgets/allowlists)
- **Share C**: Guardian/recovery (human-controlled)

Operators cannot extract keys. The agent truly owns its identity.

## TreasuryRouter

The core policy engine that decides:

- **Which rail** — BTC LN vs stable LN vs on-chain vs eCash
- **Which asset** — BTC vs USD stable
- **Which limits** — Daily, per-merchant, per-task, per-provider
- **When approvals** — Required thresholds
- **How receipts** — Recorded and published

### Example Policy Rules

```
Under $5 equivalent     → Allow eCash or Lightning automatically
Under $200              → Allow stablecoin LN if invoice is stable-denominated
Over $200               → Require human approval or guardian co-sign
Compute providers only  → Must have past verification + minimum reputation
```

## Receipts and Statements

Every payment yields:

1. **Cryptographic receipt** — Preimage / txid / taproot-assets proof / cashu proof ref
2. **Trajectory link** — "This spend happened during this agent session; here's why"
3. **Policy attestation** — Which rule allowed it, who co-signed

This is the "bank statement" equivalent for autonomous systems.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     NEOBANK TREASURY LAYER                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │ TreasuryRouter  │  │  PolicyEngine   │  │  ReceiptLedger  │     │
│  │ (rail selection)│  │ (budget/limits) │  │ (audit trail)   │     │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘     │
│           └────────────────────┴────────────────────┘               │
│                               │                                     │
│  RAILS ──────────────────────────────────────────────────────────  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │   Bitcoin    │  │   Taproot    │  │    eCash     │              │
│  │  Lightning   │  │   Assets     │  │ Cashu/Fedimint│             │
│  │ (crates/spark)│ │  (planned)   │  │  (NIP-60/61) │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                     │
│  ADAPTERS ───────────────────────────────────────────────────────  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ FiatRampAdapter│ │ CardAdapter │  │ComplianceAdapter│           │
│  │ (KYC + bank)  │  │(virtual/physical)│ (sanctions/risk)│         │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Relationship to Existing Crates

| Crate | Relationship |
|-------|--------------|
| `crates/spark` | Bitcoin/Lightning rail implementation (Breez SDK) |
| `crates/wallet` | User-facing wallet application, identity management |
| `crates/nostr/core` | Protocol types, event signing, NIP implementations |
| `crates/frostr` | FROST threshold signatures for key protection |
| `crates/marketplace` | Compute/skills marketplace (consumer of treasury services) |

## Planned Development

### Phase 1: Core Types and Policy Engine

- [ ] Account types (Treasury, Operating, Escrow, Payroll)
- [ ] TreasuryRouter with configurable policy rules
- [ ] Budget tracking and enforcement
- [ ] Receipt generation and storage

### Phase 2: Multi-Rail Routing

- [ ] Integration with `crates/spark` for BTC Lightning
- [ ] eCash support via NIP-60/61
- [ ] Rail selection logic based on amount/privacy/speed

### Phase 3: Taproot Assets

- [ ] Stablecoin hold/send/receive
- [ ] Universe endpoint integration
- [ ] AddressV2 reusable addresses

### Phase 4: Nostr Protocol Extensions

- [ ] AgentPaymentProfile events
- [ ] AssetRegistry events
- [ ] UniverseAnnouncement events

### Phase 5: Fiat Adapters

- [ ] FiatRampAdapter interface
- [ ] CardAdapter interface
- [ ] ComplianceAdapter interface

## Killer Features

### A) Programmable Budgets for Autonomous Entities

Traditional neobanks give humans controls. Agents need:

- Per-agent daily caps
- Per-task caps
- Per-provider allowlists
- Approval workflows
- Velocity-aware throttles ("if APM spikes and failure rate rises, clamp spend")

### B) Receipts That Include "Why"

A bank statement says *what* you spent. An agentic neobank says:

- **Which agent** executed the payment
- **Which trajectory** it was part of
- **Which tool result verification** preceded it
- **Which policy allowed it**
- **Who co-signed it** (if threshold)

### C) Multi-Rail Routing as First-Class Primitive

Pick the best rail per context:

- LN BTC for tiny machine payments
- Taproot Asset stable LN for "USD pricing" at scale
- eCash for privacy / content tips / offline-ish workflows

## Documentation

- **[Research Document](docs/research.md)** — Full specification and ecosystem analysis
- **[Spark Integration](../spark/README.md)** — Bitcoin/Lightning rail
- **[SYNTHESIS.md](../../SYNTHESIS.md)** — How neobank fits the broader vision

## References

- [Taproot Assets Protocol](https://docs.lightning.engineering/the-lightning-network/taproot-assets/taproot-assets-protocol)
- [NIP-87: Ecash Mint Discoverability](https://nips.nostr.com/87)
- [NIP-60: Cashu Wallets](https://nips.nostr.com/60)
- [NIP-61: Nutzaps](https://nips.nostr.com/61)
- [Cashu Protocol](https://cashu.space/)
- [Fedimint](https://fedimint.org/)
