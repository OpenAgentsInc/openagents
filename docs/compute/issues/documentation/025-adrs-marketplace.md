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
- **Breez Spark SDK** for Bitcoin/Lightning payments (vs manual Lightning implementation, on-chain, traditional rails)
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

### ADR-0008: Breez Spark SDK for Marketplace Payments
**Decision**: Use **Breez Spark SDK** (Layer 2 Bitcoin protocol) instead of manually implementing Lightning Network

**Context**:
- Need fast, low-fee Bitcoin payments for compute micropayments
- **Spark is NOT Lightning** - it's a Layer 2 Bitcoin protocol using **statechain technology**
- Uses threshold signatures (FROST): users hold one key, Spark Operators hold another
- Self-custodial: users control Bitcoin, can exit to L1 anytime via pre-signed timelocked transactions
- Provides BOLT11 compatibility (can send/receive Lightning invoices)
- **User directive**: "We're not going to be manually implementing Bolt 11... We're going to basically use Spark via Breez for all of our Bitcoin and Lightning stuff"

**Alternatives Considered**:
1. **Manual Lightning implementation** (BOLT11 parser/generator, LND/Core Lightning integration)
   - ❌ **Rejected by user**: "We're not going to be manually implementing Bolt 11"
   - ❌ High complexity: 10-13 weeks effort for wallet + Lightning integration
   - ❌ Channel management burden (liquidity, routing, watchtowers)
   - ❌ Ongoing maintenance (Lightning protocol changes)

2. On-chain Bitcoin only
   - ❌ High fees ($5-50 per transaction)
   - ❌ Slow confirmations (10-60 minutes)
   - ❌ Impractical for micropayments (<$1)

3. Stablecoins (USDC on Ethereum/Solana)
   - ❌ Centralization risk (Circle, Tether control)
   - ❌ Regulatory uncertainty
   - ❌ Not truly self-custodial

4. Traditional payments (credit cards, Stripe, PayPal)
   - ❌ High fees (2-3% + $0.30 fixed)
   - ❌ Chargeback risk
   - ❌ KYC/AML burden
   - ❌ Geographic restrictions

**Consequences**:
- ✅ **Production-ready**: Breez maintains SDK, node infrastructure, LSP (no DIY)
- ✅ **Nodeless**: No Lightning channel management (Spark handles liquidity automatically)
- ✅ **Better UX**: Offline receive, instant sends, automatic backups
- ✅ **Effort reduction**: ~35-40% reduction in Phase 2 effort (5.5-9.5w vs 10-13w manual)
- ✅ **Code sharing**: SparkWalletManager shared between iOS and macOS
- ✅ **Self-custodial**: Users control private keys (BIP39 mnemonic)
- ✅ **BOLT11 compatible**: Can transact with any Lightning wallet
- ✅ **Instant settlement**: Payments confirm in <1 second
- ✅ **Low fees**: Sub-penny for small payments
- ✅ **Permissionless**: No KYC required
- ✅ **Native Nostr integration**: Works with NIP-57 zaps
- ⚠️  **Breez API key required**: Free tier available, paid tier for production
- ⚠️  **Spark Operator dependency**: Relies on Breez's statechain operators (but pre-signed exits ensure safety)
- ⚠️  **SDK updates**: Must track Breez SDK releases for bug fixes/features

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
- **SPARK-SDK-INTEGRATION.md**: Breez Spark SDK integration plan and rationale
- **Issues #010, #012, #013, #015, #016**: Spark SDK wallet implementation

## Success Metrics

- [ ] 5 ADRs created (0007-0011)
- [ ] All major decisions documented
- [ ] ADRs reviewed and accepted
- [ ] Published in docs/adr/

## Reference

- **ADR Template**: `docs/adr/template.md`
- **ADR Guidelines**: `docs/adr/AGENTS.md`
- **Existing ADRs**: `docs/adr/0001-*.md` through `0006-*.md`
- **Spark SDK Integration**: `docs/compute/issues/SPARK-SDK-INTEGRATION.md` (full integration plan)
- **Breez Spark SDK Docs**: https://sdk-doc-spark.breez.technology/
- **Spark SDK Swift Bindings**: https://github.com/breez/breez-sdk-spark-swift
- **Related Issues**: #010 (iOS Wallet), #012 (API Key Config), #013 (macOS Wallet), #015 (Payment Coordinator), #016 (Seed Backup)
