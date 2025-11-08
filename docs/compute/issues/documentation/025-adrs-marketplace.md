# ADRs: Marketplace Architecture

**Phase:** Cross-Cutting
**Component:** Documentation
**Priority:** P1 (High - Documents decisions)
**Estimated Effort:** 1-2 weeks

## Summary

Create Architecture Decision Records (ADRs) documenting all marketplace architectural decisions, following the ADR process established in ADR-0001.

## Motivation

The marketplace introduces significant architectural decisions that must be documented:
- Nostr as marketplace protocol (vs custom P2P, blockchain, centralized)
- Lightning for payments (vs on-chain, credit cards, traditional rails)
- iOS coordination-only (vs on-device compute worker)
- Multi-backend model routing (Foundation Models + MLX + Ollama)
- Policy enforcement approach (classifier vs rules)

ADRs provide:
- **Context**: Why we made these choices
- **Alternatives**: What we considered and rejected
- **Consequences**: Trade-offs and implications
- **Reference**: Future contributors understand reasoning

## ADRs to Create

### ADR-0007: Nostr for Compute Marketplace
**Decision**: Use Nostr (NIP-90 + NIP-57) as marketplace protocol

**Context**:
- Need decentralized, censorship-resistant marketplace
- Buyers/sellers must discover each other without central server
- Payments must be peer-to-peer (Lightning)

**Alternatives Considered**:
1. Centralized marketplace (OpenAgents-hosted)
   - ❌ Single point of failure, censorship risk
   - ❌ Against open-source ethos
2. Blockchain-based (Ethereum, Bitcoin)
   - ❌ High fees, slow settlement
   - ❌ Poor UX (wallet setup, gas, etc.)
3. Custom P2P protocol (libp2p)
   - ❌ Reinvents wheel, no existing clients/relays
   - ❌ Bootstrap/discovery problem

**Consequences**:
- ✅ Leverage existing Nostr infrastructure (relays, clients)
- ✅ Truly decentralized (no OpenAgents server required)
- ✅ Interoperability (any Nostr client can participate)
- ⚠️  Relay dependencies (need reliable relays)
- ⚠️  Privacy: Events are public (unless encrypted via NIP-04)

---

### ADR-0008: Lightning for Marketplace Payments
**Decision**: Use Lightning Network (BOLT11 invoices + NIP-57 zaps) for payments

**Alternatives**:
1. On-chain Bitcoin
   - ❌ High fees, slow confirmations
   - ❌ Impractical for micropayments (<$1)
2. Stablecoins (USDC on Ethereum/Solana)
   - ❌ Centralization risk (Circle, Tether)
   - ❌ Regulatory uncertainty
3. Traditional payments (credit cards, ACH)
   - ❌ High fees (2-3% + fixed)
   - ❌ Chargeback risk, KYC/AML burden

**Consequences**:
- ✅ Instant settlement (<1 second)
- ✅ Low fees (sub-penny for small payments)
- ✅ Permissionless (no KYC required)
- ✅ Native integration with Nostr (NIP-57 zaps)
- ⚠️  Liquidity management (channels, LSPs)
- ⚠️  UX complexity (invoice generation, routing)

---

### ADR-0009: iOS as Coordination Layer Only (No Worker Compute)
**Decision**: iOS app handles coordination, wallet, identity; macOS runs compute workers

**Context**:
- Apple's App Store Review Guidelines prohibit background worker processes (ASRG 2.4.2)
- Crypto mining analogy: "apps may not run unrelated background processes"
- Background execution severely limited (only short tasks, user-initiated)

**Alternatives**:
1. iOS as compute worker (background mode)
   - ❌ **Violates ASRG 2.4.2** (background mining analogy)
   - ❌ Thermal/battery concerns
   - ❌ Likely App Store rejection
2. iOS foreground-only worker (user starts each job)
   - ⚠️  Borderline compliance
   - ❌ Poor UX (user must keep app open)
   - ❌ Not economical (users want passive earnings)

**Consequences**:
- ✅ **App Store compliant** (iOS = coordination only)
- ✅ macOS workers can run 24/7 (plugged in, menu bar app)
- ✅ Thermal/power management on macOS (better suited)
- ⚠️  Requires macOS device to be compute provider
- ⚠️  iOS-only users can buy but not sell compute

---

### ADR-0010: Multi-Backend Model Routing
**Decision**: Support Foundation Models, MLX, Ollama with intelligent routing

**Alternatives**:
1. Foundation Models only
   - ❌ Limited to Apple's models
   - ❌ No custom models or fine-tunes
2. Cloud APIs (OpenAI, Anthropic)
   - ❌ Privacy concerns (data leaves device)
   - ❌ Recurring costs
   - ❌ Against on-device philosophy
3. Single open-source backend (llama.cpp)
   - ⚠️  Less flexibility than multi-backend

**Consequences**:
- ✅ Best of all worlds (speed, privacy, custom models)
- ✅ Fallback options (if Foundation Models unavailable)
- ✅ Marketplace differentiation (offer variety)
- ⚠️  Complexity (3 backends to maintain)
- ⚠️  Testing burden (each backend needs validation)

---

### ADR-0011: Foundation Models AUP Enforcement via Classifier
**Decision**: Use Foundation Models itself to classify prompts for AUP violations

**Alternatives**:
1. Keyword/regex-based filtering
   - ⚠️  Fast but low accuracy (false positives/negatives)
2. External API (e.g., OpenAI Moderation API)
   - ❌ Privacy violation (sends prompts to cloud)
   - ❌ Latency, cost
3. No enforcement (trust users)
   - ❌ **Violates DPLA** (AUP is mandatory)

**Consequences**:
- ✅ High accuracy (LLM understands context)
- ✅ Privacy-preserving (on-device)
- ✅ Self-policing (FM enforces its own AUP)
- ⚠️  Latency (extra FM call before job execution)
- ⚠️  Fallback needed (keyword classifier when FM unavailable)

## Deliverables

For each ADR:
- [ ] Markdown file in `docs/adr/`
- [ ] Follow template (Context, Decision, Alternatives, Consequences)
- [ ] Reference apple-terms-research.md for compliance decisions
- [ ] Link to related issues
- [ ] Review by team/community

## Dependencies

- **ADR-0001**: Adopt ADRs (process)
- **AGENTS.md**: ADR writing guidelines for AI agents
- **apple-terms-research.md**: Compliance research

## Success Metrics

- [ ] 5 ADRs created (0007-0011)
- [ ] All major decisions documented
- [ ] ADRs reviewed and accepted
- [ ] Published in docs/adr/

## Reference

- **ADR Template**: `docs/adr/template.md`
- **ADR Guidelines**: `docs/adr/AGENTS.md`
- **Existing ADRs**: `docs/adr/0001-*.md` through `0006-*.md`
