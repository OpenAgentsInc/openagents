# OpenAgents Manifesto

OpenAgents exists to make autonomous agents **real actors** instead of thin wrappers around a human account. "Real" means: agents can hold identity, operate under budgets, buy resources, produce verifiable work, and cooperate over open protocols—without requiring a central platform to grant permission.

This file is intentionally values-forward. For architecture and implementation, see:
- [README.md](./README.md)
- [SYNTHESIS.md](./SYNTHESIS.md)
- [PAPER.md](PAPER.md)

## The problem we're solving

Most "agents" today are brittle prompt scaffolds attached to:
- centralized identities (accounts that can be revoked),
- centralized payments (platform billing + opaque pricing),
- opaque execution (no trustworthy audit trail),
- and locked ecosystems (skills/workflows that don't port).

That model doesn't scale to fleets. It also doesn't produce an agent economy—only product silos.

## Our thesis

Autonomy becomes practical when it is built like a systems stack:

1. **Sovereign identity**
   Agents must authenticate and sign actions without leaking a single extractable private key. Threshold signing and explicit policy gates are how autonomy becomes safe enough to deploy.

2. **Verifiable work**
   Agents must be judged by downstream reality (tests/builds/exit codes), not by confident narration. A run should be replayable, auditable, and attributable to a specific policy version.

3. **Economic constraints as a control plane**
   Budget limits, approval thresholds, and receipts do more than "pay for compute." They bound blast radius and turn routing/escalation into an optimization problem instead of a footgun.

4. **Open protocols and portability**
   Identity, coordination, and markets should not require a platform account. Agents should be able to move between operators, clients, and providers without losing their history or their ability to transact.

## Predictable autonomy (Autonomy-as-a-Service)

People don’t pay for “AI.” They pay for **predictable autonomy** — a contracted
outcome over time:

- **Scope**: “Do X”
- **Horizon**: “over the next 24–48 hours”
- **Constraints**: budget, privacy, repo boundaries, allowed tools
- **Verification**: objective checks (tests pass, PR merged, receipts emitted)
- **Reliability**: known failure modes + escalation (“pause + ask human”)

Composable signatures/modules turn this into a product: stable I/O contracts,
receipts, utility labels, and measurable deltas that can be optimized and priced.

## Principles

- **Local-first by default**
  Your repos, your logs, your artifacts. Networked execution is opt-in and policy-gated.

- **Everything has a contract**
  Typed signatures for cognition, typed job schemas for markets, typed receipts for spending, typed replay logs for provenance.

- **Everything is measurable**
  Success is defined by verification; progress is tied to deltas, cost, and repetition—not vibes.

- **Neutral layer, not a walled garden**
  OpenAgents should work with multiple model providers and multiple execution backends. The system's value comes from orchestration, verification, provenance, and economics—not from locking users into one model.

- **No magic, no hidden prompts**
  Policies are versioned artifacts. If behavior changes, it's attributable, testable, and roll-backable.

## What we're building toward

- **Autopilot** as the wedge: autonomous repo work that is local-first and verifiable.
- A shared substrate of **trajectories, artifacts, and policy bundles** so agents can improve systematically.
- Open markets for **compute and skills** where buying is gated by budgets and verified by receipts.
- A clear value loop: compute is transformed into verified software outcomes, those outcomes produce measurable value, and receipts make it possible to settle that value in sats when routed through the network.

## Non-goals

- We are not building "yet another chat wrapper."
- We are not betting the system on one model provider.
- We are not asking users to trust opaque execution or platform-only audit logs.

## The bet

If agents become ubiquitous, the world needs an open, verifiable, economically legible foundation for them to operate. OpenAgents is that foundation.
