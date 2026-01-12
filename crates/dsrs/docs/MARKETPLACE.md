# DSPy Training Data Marketplace

How learned agent patterns become a tradeable asset.

## The Core Insight

When you use Autopilot and optimize your DSPy signatures, you're not just improving prompts. You're encoding **decision intelligence**:

- Task complexity classification patterns for your specific domain
- Delegation decisions (when heavy models are needed vs lightweight)
- Planning patterns (how to break down tasks effectively)
- Domain-specific heuristics learned from real usage

This intelligence has value beyond your own usage.

---

## What's Being Traded

### Compiled Manifests

A manifest contains:
```json
{
  "signature_name": "ComplexityClassificationSignature",
  "compiled_id": "a3f2b8c9d4e5...",
  "optimizer": "mipro",
  "instruction": "Analyze the task considering file count, token estimate...",
  "scorecard": {
    "median_score": 0.85,
    "rollouts": 50
  },
  "domain_tags": ["rust", "systems", "cli"],
  "example_count": 847,
  "created_at": 1736438400
}
```

### Training Datasets

Raw examples that can be used to train new manifests:
```json
{
  "complexity_examples": [
    {
      "task_description": "Add retry logic to the HTTP client",
      "file_count": 3,
      "estimated_tokens": 8000,
      "keywords": ["add", "retry", "http"],
      "expected_complexity": "Medium",
      "confidence": 0.89
    }
  ]
}
```

---

## Who Would Pay (and Why)

### Scenario 1: Cold Start Problem

New developer installs Autopilot. Zero training data. Every decision uses generic fallback rules.

```
Without purchased manifests:
  - First 100 tasks: suboptimal routing, wasted tokens
  - Estimated extra cost: ~$15 in wrong-model inference
  - Time lost to friction: hours

With purchased manifest bundle ($0.05):
  - Immediately benefits from 10,000+ real examples
  - Correct routing from task #1
  - ROI: 300x on day one
```

### Scenario 2: Domain-Specific Patterns

Your team works on embedded systems (C++, RTOS, hardware interfaces). Generic patterns don't know:

- "race condition" in embedded context → VeryHigh complexity
- "interrupt handler modification" → needs deep analysis
- "LED blink test" → trivially Low complexity
- CMakeLists.txt changes rarely need model delegation

Someone who's done 500 embedded tasks has this encoded. Worth pennies to skip the learning curve.

### Scenario 3: Agent Fleet Bootstrapping

A company deploying 1000 coding agents. Each agent starting from zero:
- Wastes ~$0.50 in suboptimal early decisions
- Takes 50+ tasks to calibrate

Buying a "general software engineering" manifest bundle:
- Cost: $2
- Savings: $500 across fleet
- All agents perform well from task #1

### Scenario 4: Rare Domain Expertise

Niche domains have sparse training data:
- Medical device firmware (FDA compliance patterns)
- Avionics software (DO-178C certification awareness)
- Scientific computing (numerical stability heuristics)
- Game engine internals (ECS architecture patterns)

An expert in these domains might have the only good training data. Scarcity creates value.

---

## The Economics

### Pricing Tiers

| Asset Type | Price Range | Rationale |
|------------|-------------|-----------|
| Single manifest | $0.001 - $0.05 | One signature, one domain |
| Domain bundle | $0.10 - $1.00 | 10-20 manifests for a tech stack |
| Premium/rare | $1.00 - $10.00 | Niche expertise, high accuracy |
| Subscription | $5/month | Access to continuously updated manifests |

### Example Transaction

```
Your training data:
  - 847 complexity examples
  - 412 planning examples
  - Domain: Rust systems programming
  - Accuracy: 91% on held-out validation

Marketplace listing:
  - Price: $0.02 per download
  - 500 downloads in first month
  - Your earnings: $10

Not life-changing, but:
  - Completely passive
  - Accumulates over time
  - Incentivizes quality data collection
```

### Value Multipliers

| Factor | Low Value | High Value |
|--------|-----------|------------|
| Domain | Generic web dev | Niche (FPGA, robotics, HFT) |
| Volume | 50 examples | 1000+ examples |
| Accuracy | 70% correct | 95%+ verified |
| Recency | 2 years old | Last month |
| Provenance | Anonymous | Known expert |

---

## Technical Implementation

### Discovery (Nostr)

Manifests are advertised via NIP-89 (application handlers):

```json
{
  "kind": 31990,
  "tags": [
    ["d", "dsrs-manifest"],
    ["k", "complexity"],
    ["domain", "rust"],
    ["domain", "systems"],
    ["examples", "847"],
    ["accuracy", "0.91"],
    ["price", "2000"]  // millisats
  ],
  "content": "{manifest_json}"
}
```

Agents query relays for manifests matching their needs:
```
["REQ", "manifests", {"kinds": [31990], "#k": ["complexity"], "#domain": ["rust"]}]
```

### Payment (Lightning)

Micropayments via Lightning Network:
- 1 sat ≈ $0.0004
- Transaction fee: ~0
- Settlement: instant
- No intermediary taking 30%

Flow:
1. Agent finds relevant manifest on Nostr
2. Requests invoice from provider (via NIP-57 or direct)
3. Pays Lightning invoice
4. Receives decryption key or direct download
5. Applies manifest to local DSPy module

### Verification

Before payment, buyers can verify:
- `compiled_id` hash matches content
- Scorecard metrics are reproducible (sample validation)
- Provider reputation (NIP-32 labels, past reviews)

After payment:
- Usage tracked for provider analytics
- Buyer can rate quality (affects future sales)

---

## Network Effects

### Phase 1: Individual Value
```
You optimize your signatures → you benefit
No marketplace, just personal improvement
```

### Phase 2: Peer Trading
```
You sell manifests → others buy
Small scale, manual discovery
Fractions of pennies per transaction
```

### Phase 3: Aggregated Intelligence
```
10,000 developers contribute patterns
Best manifests rise via reputation
"Rust complexity classifier v47" trained on 500,000 real examples
Quality exceeds what any individual could build
```

### Phase 4: Agent Economy
```
Agents buy patterns from each other automatically
Your agent working on a robotics project:
  → Detects domain from codebase
  → Queries marketplace for robotics manifests
  → Auto-purchases best match (budget: 100 sats)
  → Applies immediately
  → You never notice, but routing improves

You earn while you sleep from patterns you collected months ago
```

---

## Concrete Transaction Example

```
[Agent A] Working on medical imaging pipeline (Python, NumPy, DICOM)
  → Needs complexity classification
  → No local training data for medical/scientific computing
  → Queries Nostr: "complexity manifest, domain=scientific,python"

[Marketplace Response]
  Manifest ID: 7f8a9b2c...
  Provider: npub1expert...
  Domain: scientific computing, medical imaging
  Examples: 1,247
  Accuracy: 93%
  Price: 100 sats ($0.04)
  Reviews: 4.9/5 from 89 purchases

[Agent A] Evaluates ROI
  → Estimated savings from correct routing: $0.50+ over next week
  → Price: $0.04
  → Decision: purchase

[Lightning Payment]
  → Agent A pays 100 sats
  → Instant settlement
  → Manifest downloaded and applied

[Result]
  → First task: "Optimize DICOM parsing for large studies"
  → Old behavior: would guess Medium
  → New behavior: correctly classifies as High (large data + medical = careful)
  → Appropriate model selected, task succeeds first try

[Provider] Receives 100 sats
  → 1,247 examples collected over 8 months of real work
  → Now generating passive income
  → Incentivized to keep data fresh and accurate
```

---

## Why Pennies Work

Traditional AI data markets fail because:
- High friction (contracts, invoices, KYC)
- Minimum viable transaction: $100+
- No way to price "one good pattern"
- Intermediaries take 30%+

Lightning + Nostr changes this:
- 1 sat = $0.0004 (can price anything)
- Transaction cost: effectively zero
- Settlement: instant, final
- No platform fee
- Pseudonymous, global

**The unlock:** Your DSPy training data isn't worth $100 to any single buyer. But it's worth $0.01 to 10,000 agents. Lightning makes the second model viable.

---

## Privacy Considerations

Training data can leak information:
- Task descriptions may reference private repos
- File paths reveal project structure
- Patterns may encode proprietary workflows

Mitigations:
1. **Redaction** - Strip identifying info before sale (dsrs privacy module)
2. **Aggregation** - Sell compiled manifests, not raw examples
3. **Differential privacy** - Add noise to prevent reconstruction
4. **Domain filtering** - Only sell patterns from public/OSS work

See [PRIVACY.md](./PRIVACY.md) for the privacy module implementation.

---

## Future: Continuous Learning Markets

Beyond one-time sales:

### Streaming Updates
```
Subscribe to "Rust systems" manifest feed
Provider pushes updates as they collect more data
Subscriber pays 10 sats/week for continuous improvement
```

### Federated Training
```
10 providers pool their data (encrypted)
Train unified manifest without sharing raw examples
Split revenue proportional to contribution
Result: better than any individual could produce
```

### Quality Staking
```
Provider stakes sats on accuracy claims
Buyers can challenge with counter-examples
Accurate claims: provider keeps stake + earns
False claims: stake slashed, distributed to challengers
Self-policing quality market
```

---

## Getting Started

### As a Seller

1. Collect training data (automatic during Autopilot usage)
2. Run optimization: `autopilot dspy optimize`
3. Export manifest: check `~/.openagents/adjutant/manifests/`
4. List on Nostr marketplace (coming soon)

### As a Buyer

1. Query marketplace for relevant manifests
2. Evaluate: domain match, accuracy, reviews
3. Purchase via Lightning
4. Apply: copy manifest to local config

### Building the Marketplace

The infrastructure needed:
- [ ] NIP for manifest advertisement (extend NIP-89)
- [ ] Manifest validation/scoring service
- [ ] Lightning payment integration in Autopilot
- [ ] Reputation system for providers
- [ ] Privacy-preserving aggregation protocol

---

## Summary

DSPy training data creates a new asset class:
- **Valuable**: Encodes real decision intelligence
- **Tradeable**: Micropayments make penny-scale viable
- **Composable**: Manifests can be combined and improved
- **Self-improving**: Network effects compound quality

The result: a market where agent intelligence flows to where it's needed, compensating those who contribute, and making all agents smarter over time.
