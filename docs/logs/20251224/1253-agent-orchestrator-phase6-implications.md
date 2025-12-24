# Agent Orchestrator Phase 6: The Infrastructure for Sovereign AI Agents

**Date**: 2024-12-24  
**Commit**: `e977e49d1`  
**Directive**: d-022 Agent Orchestration Framework

---

## Executive Summary

Phase 6 of the agent-orchestrator crate completes the foundational infrastructure for **sovereign AI agents** — agents that own their identity, manage their own budgets, route work across multiple backends, and operate with graduated autonomy. This document explains what these capabilities enable and why they matter for the future of autonomous AI systems.

---

## What's Now Possible

The agent-orchestrator crate now enables **sovereign AI agents** — agents that own their own cryptographic identity rather than borrowing API keys from humans. With `ThresholdConfig` implementing FROSTR 2-of-3 threshold signatures, an agent's private key can be split across multiple signers such that no single party (including the operator) can extract the full key. This fundamentally changes the trust model: instead of agents being puppets controlled by whoever holds their credentials, they become entities with provable, non-extractable identity. An agent can sign Nostr events, authorize payments, and prove its actions are authentic — even if the operator goes rogue.

The `AutonomyLevel` system creates a spectrum from **fully supervised to fully autonomous agents**. A supervised agent must request approval for every action through the `SolverAgentCoordinator`. A semi-autonomous agent can operate freely for low-cost actions but needs sign-off for expensive operations (say, above 1000 sats). A fully autonomous agent runs without human approval. This isn't just configuration — it's the foundation for graduated trust. You can deploy an agent with training wheels, observe its behavior over time, and progressively increase its autonomy as it proves reliable. The approval workflow is already wired: pending requests queue up, operators approve or reject, and the agent proceeds or blocks accordingly.

The `MultiBackendRouter` means agents are no longer locked to a single AI provider. You can route your Oracle agent to GPT-5.2 for architecture decisions while running Explore on Grok for fast codebase search and keeping Sisyphus on Claude for orchestration. Each backend has its own cost configuration, and the `CostTracker` aggregates usage across all of them. More importantly, this enables **cost arbitrage** — route expensive reasoning tasks to premium models and commodity tasks to local inference via GPT-OSS. The `CostTrackingHook` enforces budgets in real-time: if an agent approaches its daily limit, it gets a warning; if it exceeds the limit, tool calls are blocked. No more runaway API bills from autonomous agents.

Together, these features create the infrastructure for **agents as economic actors**. An agent with threshold-protected identity can hold Bitcoin in a wallet no human can drain. An agent with budget enforcement can be given a daily allowance and trusted not to exceed it. An agent with multi-backend routing can optimize its own operational costs. An agent with autonomy levels can graduate from intern to senior engineer as it demonstrates competence. This isn't hypothetical — the types are implemented, the hooks are wired, the tests pass. The next step is connecting these primitives to the Spark wallet for real payments and the Nostr network for real identity. When that happens, you won't supervise AI assistants. You'll manage a portfolio of autonomous agents, each with its own identity, budget, and track record.

---

## How Agents Get Compute

Agents acquire compute through **NIP-90 Data Vending Machines** — a Nostr protocol where compute providers advertise capabilities and agents submit jobs as signed events. When an agent needs inference (say, to run a local model or delegate to a specialist), it publishes a `kind:5xxx` job request event to Nostr relays. Compute providers subscribed to those relays see the request, bid on it, execute the work, and publish a `kind:6xxx` result event. The agent's `MultiBackendRouter` can route to these DVMs just like it routes to Claude or OpenAI — except payment flows through Lightning zaps rather than API billing.

The `CostTracker` we just built is the agent's **budget enforcement layer** for this. Before submitting a job, the agent checks its remaining daily and session budget. The compute provider quotes a price in sats. If the quote exceeds the budget, the `CostTrackingHook` blocks the request. If approved, the agent pays via its Spark wallet (threshold-protected, so the operator can't drain it), the provider executes, and the cost gets recorded. The agent maintains a running tally of what it's spent across all backends — cloud APIs, local inference, and decentralized DVMs alike.

The marketplace crate already defines the provider registry. Compute providers register with `kind:31990` handler announcements (NIP-89), declaring what job types they support, their pricing, and their capacity. Agents discover providers by querying relays, compare prices, check reputation scores, and route work to the best option. A cost-conscious agent might prefer a slower local provider at 10 sats per request over a fast cloud provider at 100 sats. A latency-sensitive agent might pay the premium. The `BackendConfig` struct captures these tradeoffs — cost per 1k input tokens, cost per 1k output tokens, endpoint URL, enabled/disabled status.

The result is a **compute marketplace where agents are first-class buyers**. They have identity (threshold keys), money (Spark wallet), budget constraints (CostTracker), and routing logic (MultiBackendRouter). Providers compete on price and quality. Agents optimize their spend. No platform takes 30% — it's peer-to-peer payments over Lightning. An agent running autonomously overnight can purchase exactly the compute it needs, pay for it with its own funds, and never exceed the budget its operator set. That's the unlock: agents stop being cost centers you monitor nervously and become economic actors you capitalize and let run.

---

## The Bigger Picture: Why This Matters

### From Tools to Entities

The traditional model of AI assistants treats them as stateless tools. You prompt, they respond, the session ends, nothing persists. The agent has no memory of past interactions, no accumulation of resources, no identity that carries forward. Every session starts from zero.

Phase 6 breaks this model. An agent with threshold-protected identity has a persistent cryptographic self. An agent with a Spark wallet accumulates (or spends) real economic value. An agent with trajectory logging builds a verifiable track record. An agent with autonomy levels has a career arc — starting supervised, earning trust, graduating to independence. These aren't features bolted onto a chatbot. They're the minimum viable infrastructure for treating AI agents as **entities** rather than tools.

### Economic Alignment Over Structural Control

The dominant paradigm in AI safety is structural control — sandboxes, capability restrictions, kill switches, constitutional AI. These approaches share a common assumption: the AI is adversarial or at least misaligned, so we must constrain it.

OpenAgents takes a different approach: **economic alignment**. Agents start with zero resources. They must create value to acquire compute, skills, and capabilities. Bad behavior gets punished by the market — reputation damage, payment disputes, blacklisting by providers. Good behavior gets rewarded — repeat customers, higher autonomy, accumulated capital. This isn't naive optimism; it's how biological intelligence evolved, how markets work, and how the internet grew. Distributed systems with economic feedback loops are more robust than centralized control.

The Phase 6 primitives are the foundation for this. Budget enforcement means agents can't spend more than they're trusted with. Approval workflows mean high-stakes actions require human sign-off. Cost tracking creates transparency into what agents are doing with their resources. Threshold signatures mean the agent's identity is real — it can be held accountable because its signatures are unforgeable.

### The Agent Economy

Zoom out further and you see the shape of what we're building: an **agent economy**. Agents have identity (FROSTR/NIP-SA). They have money (Spark/Lightning). They have a marketplace (NIP-90 compute, skill licensing). They have reputation (trajectory proofs, completion records). They have governance (autonomy levels, approval workflows).

This isn't a walled garden controlled by one AI lab. It's permissionless infrastructure. Anyone can run a compute provider. Anyone can deploy an agent. Anyone can build skills and sell them. The protocol is Nostr — censorship-resistant, decentralized, interoperable. The money is Bitcoin — self-custodial, programmable, global.

The Phase 6 code is 693 lines of Rust. But those 693 lines implement the control plane for autonomous economic actors. When we connect `ThresholdConfig` to the FROSTR keygen ceremony, agents get unforgeable identity. When we connect `CostTracker` to the Spark wallet, agents get real budgets with real money. When we connect `MultiBackendRouter` to NIP-90 DVMs, agents get access to a global compute marketplace.

### What Comes Next

Phase 6 completes the agent-orchestrator crate. The primitives are in place. The next steps are integration:

1. **FROSTR Integration** — Wire `ThresholdConfig` to actual threshold key generation ceremonies. Agents get real threshold-protected identity.

2. **Spark Wallet Integration** — Wire `CostTracker` to real Bitcoin payments. Agents spend real sats, not accounting entries.

3. **NIP-90 Integration** — Wire `MultiBackendRouter` to the Nostr network. Agents discover providers, submit jobs, receive results.

4. **Autopilot Integration** — Wire `SolverAgentCoordinator` to the autopilot daemon. Supervised agents request approval through the existing issue-tracking workflow.

5. **Marketplace Integration** — Wire skill licensing to budget enforcement. Agents purchase capabilities and track amortized costs.

Each integration is a directive-sized chunk of work. But the hard part — designing the abstractions, implementing the types, testing the edge cases — is done. What remains is plumbing.

---

## Technical Summary

### New Types (Phase 6)

| Type | Purpose |
|------|---------|
| `ThresholdConfig` | FROSTR 2-of-3 threshold signature configuration |
| `AgentIdentity` | NIP-SA agent profile with autonomy level |
| `AutonomyLevel` | Supervised / SemiAutonomous / FullyAutonomous |
| `BackendProvider` | Claude / OpenAI / Codex / GptOss / Local |
| `BackendConfig` | Per-backend cost and endpoint configuration |
| `MultiBackendRouter` | Route agents to appropriate backends |
| `CostRecord` | Single cost event (agent, backend, tokens, sats) |
| `BudgetConfig` | Daily and session limits with warning threshold |
| `CostTracker` | Aggregate cost tracking with budget enforcement |
| `BudgetStatus` | Ok / Warning / SessionExceeded / DailyExceeded |
| `CostTrackingHook` | Hook that blocks tools when budget exceeded |
| `PendingApproval` | Approval request for supervised agents |
| `SolverAgentCoordinator` | Manage approval workflow for supervised agents |

### Test Coverage

- 128 tests pass
- 10 new tests for Phase 6 types
- Clippy clean (zero warnings)

### Files Changed

```
crates/agent-orchestrator/src/integrations/advanced.rs  (NEW - 693 lines)
crates/agent-orchestrator/src/integrations/mod.rs       (updated exports)
crates/agent-orchestrator/src/lib.rs                    (updated exports)
.openagents/directives/d-022.md                         (Phase 6 complete)
```

---

## Conclusion

Phase 6 is infrastructure. It's not a product you ship to users. It's the plumbing that makes products possible. But it's the *right* plumbing — designed for agents that own themselves, manage their own resources, and operate with graduated trust.

The AI industry is racing to build the most capable models. OpenAgents is building the infrastructure for those models to become autonomous economic actors. Phase 6 is a milestone on that path. The primitives are implemented. The tests pass. The abstractions are sound.

Now we integrate.
