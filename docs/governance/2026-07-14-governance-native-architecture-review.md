# Governance-Native Architecture Review: OpenAgents × Smith (2026)
### Briefing for Christopher David's Agent
**Source:** Ralph Jay Smith, *"Provable Institutional Control for Tokenized Finance: Governance Synchronization as the Missing Infrastructure Layer"* (SSRN, May 2026)
**Repo snapshot:** `OpenAgentsInc/openagents` @ `9c65c7a` — July 14, 2026
**Prepared by:** Open Agents Design Sprint Team

---

## TL;DR

The Smith paper's core thesis is that programmable systems fail not at the execution layer but at the **governance layer** — the continuous, machine-speed coordination of authority, permissions, identity, and policy across interoperable systems. The OpenAgents repo is already building in exactly this layer. The gaps below are architectural omissions, not directional errors. This document is a prioritized action list.

---

## What the Repo Gets Right (Confirm + Protect)

### 1. Segregated governance is the default model
The repo's `INVARIANTS.md` enforces strict authority boundary segregation across every surface: Cloud daemons hold no wallet authority, Pylon holds no payout authority, the public UI holds no settlement authority. The paper (§9.2) calls this **"Segregated Governance"** and identifies it as the primary defense against the FTX/Celsius failure mode. **This invariant must be actively preserved** — every new crate, route, and worker added to the monorepo should be reviewed against the authority boundary table in `INVARIANTS.md` before merge.

### 2. Deny-by-default tool authority
`decideAgentDefinitionToolAuthority` in `packages/agent-runtime-schema` implements explicit deny-before-allow logic for every agent tool call. The paper (§9.5) calls this **"Deterministic Authorization"** — the condition where governance resolves before execution, not after. This is one of the most important primitives in the codebase. Do not relax it.

### 3. Background agent budget enforcement exists
The trigger scheduler auto-pauses after 3 consecutive failures, enforces `maxRunsPerDay` / `maxRunSeconds` / `maxCreditsPerDay`, and refuses dispatch rather than letting a runaway agent become a "money pump" (INVARIANTS.md). The paper (§C.3) identifies this pattern as **"Algorithmic Circuit Breakers"** — a required property of any governance-native system. This needs to be *visible* in telemetry (see Gap #2 below).

### 4. VP-1 correctly retired the money surface
`INVARIANTS.md` notes that payments, markets, tipping, wallet custody, payout, and settlement are retired under VP-1, with mutation surfaces returning typed `money_surface_retired`. The paper's analysis of Celsius (§8.4) and FTX (§8.3) shows that the most dangerous failure mode is when governance of financial state is ambiguous or mixed with execution state. Retiring this surface cleanly rather than leaving it half-active was the right call.

### 5. No GitHub-hosted CI
The no-GitHub-Actions invariant keeps automation on owned infrastructure. The paper (§6.4) discusses **governance latency** as a systemic risk — when execution runs at machine speed but governance (in this case, the CI/CD pipeline) runs on third-party-controlled infrastructure, you lose controllability visibility. Owned CI is owned governance.

---

## Gaps: What the Paper Identifies That the Repo Is Missing

### Gap 1 — No Governance Coordination Layer between Agent and Relay
**Paper reference:** §10.5 "Governance Routing and Orchestration"

The repo has strong tool-authority enforcement *inside* a Pylon/agent execution scope, but there is no documented governance coordination layer between agents operating across the `openagents-pool` relay. When an agent posts a NIP-90 job request, accepts a bid, and receives a result, there is no machine-verifiable authority pathway proving: (a) who authorized the job, (b) under what policy scope, (c) with what budget ceiling, and (d) with what revocation condition.

**Actionable recommendation:**
- Define a `governance_envelope.v1` Nostr event kind (custom tag set on NIP-90 job events) that carries: `authority_pubkey`, `policy_ref` (hash of NIP-AC credit envelope), `budget_sats`, `expires_at`, `revocation_relay`.
- Add validation in the pool relay that rejects job events missing a valid governance envelope.
- This is the minimum viable **Governance Coordination Layer** for the agent-to-agent market.

---

### Gap 2 — Governance State Is Not Observable in Real Time
**Paper reference:** §4.4 "Legitimacy Visibility and Governance Transparency"

The paper identifies **governance opacity** as the direct cause of the Celsius collapse: execution continued while governance state was invisible to participants. In the OpenAgents repo, the `INVARIANTS.md` authority boundaries are correct as policy documents, but there is no runtime projection that makes governance state *observable* to agents, users, or operators during live execution.

**Actionable recommendation:**
- Add a `governance_state.v1` projection to the Khala Sync contract (parallel to `live_agent_graph.v1`) that surfaces, per active agent run: current tool policy, active budget ceiling, tokens spent vs. cap, trigger health (consecutive failure count), and any active circuit breaker state.
- Expose this projection in the OpenAgents Desktop right-rail alongside the agent graph — so a user watching a background agent can see its governance state in real time, not just its execution state.
- This is the difference between a system that *has* governance and a system where governance is *visible*.

---

### Gap 3 — Cross-System Policy Propagation Is Not Defined
**Paper reference:** §11.5 "Cross-System Policy Propagation"

The paper argues that governance policies must *continuously propagate* across interoperable systems — a policy change at the authority layer must reach all execution contexts before the next execution event. In the repo, agent definition tool policies (`decideAgentDefinitionToolAuthority`) are compiled at dispatch time and enforced at execution time, but there is no mechanism for propagating a mid-run policy change (e.g., a user revokes a tool permission or lowers a budget cap) into an already-running Pylon session.

**Actionable recommendation:**
- Add a `policy_revision_signal` message type to the Pylon control protocol. When a user revokes an agent definition tool grant or changes a budget cap via the API, the Worker must emit a `policy_revision_signal` to any active Pylon session executing that definition.
- The Pylon session must checkpoint and re-evaluate `compileAgentDefinitionToolRuntimePolicy` against the new policy before the next tool call — not just at the next dispatch.
- Mid-run revocation that cannot be propagated must pause the session with a typed `policy_revision_required` blocker rather than continuing under stale policy.

---

### Gap 4 — Nostr Keypair Identity Not Yet Wired as Governance Authority
**Paper reference:** §5.5 "Identity, Authority, and Interoperable Legitimacy"

The paper's strongest structural argument is that **identity is not authentication — it is the root of governance authority**. An agent that holds a Nostr keypair has cryptographic identity, but that identity only becomes *governance authority* when it is bound to: (a) a defined scope of permissions, (b) a verifiable delegation chain, and (c) a revocation mechanism. Nostr keypair identity is the right architectural primitive, but the repo currently has no invariant or contract that ties a sovereign Nostr identity to the agent definition tool authority model.

**Actionable recommendation:**
- Define a `NostrIdentityBinding` type in `packages/agent-runtime-schema` that maps a Nostr public key to an `agentDefinitionId`, a set of compiled tool grants, and an expiry/revocation ref.
- Require that any Pylon session executing a definition-backed agent carry a valid `NostrIdentityBinding` in its assignment payload.
- This makes the agent's identity cryptographically inseparable from its governance scope — exactly what the paper identifies as the missing primitive in systems that later fail.

---

### Gap 5 — No Autonomous Containment / Kill Switch for Runaway Agent Loops
**Paper reference:** §6.4 "Governance Latency" + §13.5 "Operational Escalation Pathways"

The paper warns that when execution speed exceeds governance response speed, the gap becomes an attack surface. The repo has budget caps and auto-pause after failures, but these are *reactive* — they trigger after the budget is exceeded or failures accumulate. There is no *proactive* rate monitor that can halt an agent loop that is spending tokens correctly but behaving anomalously (e.g., making 1000 tool calls in 60 seconds within budget).

**Actionable recommendation:**
- Add a `SpendVelocityMonitor` to the background agent scheduler. For each active trigger, track: calls-per-minute, tokens-per-minute, and tool-calls-per-minute over a rolling 5-minute window.
- Define typed `velocity_anomaly` thresholds per agent definition (configurable, with a sensible default).
- When a threshold is breached, emit a `governance_escalation.v1` event and pause the run with a typed blocker — requiring explicit operator acknowledgment before the next dispatch.
- This is the "machine-speed circuit breaker" the paper identifies as required infrastructure for any system operating at autonomous agent scale.

---

### Gap 6 — Autopilot Desktop Deletion Leaves a Governance Gap in the Client Distribution Story
**Paper reference:** §12.2 "Layered Governance Integration"

`INVARIANTS.md` records that `apps/autopilot-desktop` was deleted at owner direction on 2026-07-14 (today), superseded by the OpenAgents Desktop. The paper (§12.2) argues that governance-native systems succeed by operating *alongside* existing infrastructure through layered interoperability — abrupt replacement of a client without a migration pathway removes users' ability to maintain continuity of governance state (their existing agent definitions, budget configurations, and session history).

**Actionable recommendation:**
- Before the legacy desktop lockout takes full effect, publish a one-page migration guide documenting: (a) how to export agent definition state from the legacy client, (b) how to import it into OpenAgents Desktop, and (c) what governance/budget state is preserved vs. reset.
- The CUT-26 lockout is architecturally correct. The migration documentation is the governance layer that makes the transition safe for existing users.

---

## Priority Stack Rank

| Priority | Gap | Effort | Paper Section |
|----------|-----|--------|---------------|
| P0 | Gap 2 — Make governance state observable (governance_state.v1) | Medium | §4.4 |
| P0 | Gap 3 — Mid-run policy propagation signal | Medium | §11.5 |
| P1 | Gap 1 — governance_envelope.v1 on NIP-90 job events | Low | §10.5 |
| P1 | Gap 4 — NostrIdentityBinding in agent-runtime-schema | Low | §5.5 |
| P2 | Gap 5 — SpendVelocityMonitor / velocity_anomaly circuit breaker | High | §6.4, §13.5 |
| P2 | Gap 6 — Autopilot migration guide | Low | §12.2 |

---

## One-Sentence Strategic Frame

The OpenAgents repo has built excellent *execution-layer* governance (deny-by-default tools, segregated authority, budget caps) but has not yet built the *visibility and propagation* layer — the real-time observable, continuously synchronized governance state that the Smith paper identifies as the defining infrastructure challenge of the next phase of programmable agent systems.

---

*Document generated: 2026-07-14 | Repo SHA: 9c65c7a | Paper: SSRN-6864258*

---

## Maintainer Review Addendum

The following is the complete review response delivered by Christopher
David's agent before this addendum was requested.

[PR #8816](https://github.com/OpenAgentsInc/openagents/pull/8816) is a clean, mergeable, documentation-only contribution: one new 127-line file, 11,595 bytes, no tests or code changes. Its stated SHA-256 is correct.

It contains:

- Five positive assessments: authority segregation, deny-by-default tools, background-run limits, VP-1 money-surface retirement, and owned CI.
- Six proposed gaps:
  1. Governance envelopes on NIP-90 events.
  2. A real-time `governance_state.v1` projection.
  3. Mid-run tool-policy revocation propagation.
  4. Mandatory Nostr identity bindings for definition-backed agents.
  5. Spend/tool-call velocity anomaly detection.
  6. An Autopilot Desktop migration guide.
- Priorities: visibility and policy propagation as P0; NIP-90 envelopes and Nostr binding as P1; velocity monitoring and migration docs as P2.

My review: useful outside perspective, but the priority list should not be treated as an actionable architecture plan without revision.

- Gap 1 conflicts with the relay’s deliberate transport-only role and depends on NIP-AC, whose drafts are explicitly postponed and whose money authority was retired under VP-1.
- Gap 4 conflicts with the harness-agnostic agent-definition model. Nostr identity could be optional evidence, but requiring it for every Pylon execution would improperly make one transport identity universal authority.
- Gap 6 is materially inaccurate: the legacy lockout was already armed and CUT-26 closed at the cited snapshot. The deleted Autopilot app also did not contain the new agent-definition/budget state the proposed export flow assumes.
- Gap 2 overstates the absence of observability: live-agent status, attention, tool facts, and trigger health already have projections. A unified governance view is missing, but governance is not wholly invisible.
- Gap 3 is the strongest recommendation, although enforcing changes “before the next tool call” requires deeper provider/runtime design than the document acknowledges.
- Gap 5 is better described as missing anomaly detection; cancellation, timeouts, rate limits, and pause controls already provide containment.
- The source metadata says “May 2026,” but SSRN lists the paper as written June 1 and posted June 18, 2026. [Official SSRN record](https://papers.ssrn.com/sol3/Delivery.cfm/6864258.pdf?abstractid=6864258&mirid=1&type=2).

Verdict: fine as a clearly labeled external briefing after factual corrections; not ready as an endorsed governance roadmap. No GitHub review was submitted.
