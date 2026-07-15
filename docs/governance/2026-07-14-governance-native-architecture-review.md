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

## Maintainer Architecture Review Addendum

### Executive verdict

The submitted review is valuable as an external architecture prompt. Its most
important idea is sound: authority is not complete merely because it is checked
once at dispatch. A long-running autonomous system also needs a truthful way to
observe the policy currently in force, propagate restrictive policy changes,
prove which revision an executor actually enforced, and stop work when that
proof becomes stale or unavailable.

That insight should be retained. The submitted priority stack should not be
adopted as the OpenAgents implementation roadmap, however. It mixes three
different categories of statement without consistently separating them:

1. accurate descriptions of controls that already exist in OpenAgents;
2. useful but incomplete design directions, especially around mid-run
   revocation and runtime observability; and
3. recommendations that conflict with current product scope or established
   authority boundaries, particularly mandatory relay governance, mandatory
   Nostr identity binding, revival of NIP-AC-shaped money semantics, and a
   migration path for clients that were deliberately removed.

The right disposition is therefore **accept as architectural input, not as an
endorsed plan**. The original review is preserved above as submitted. This
addendum records which claims are confirmed, which need narrower wording, which
should be rejected, and what an implementation-safe sequence would look like if
the owner later places this governance work into the canonical Sol roadmap.

### Review scope and evidence boundary

This assessment compares the submitted document with the repository snapshot it
names (`9c65c7a`) and with the authority boundaries that remain on current
`main`. The principal repo sources are:

- the [root invariant ledger](../../INVARIANTS.md), particularly Authority
  Boundaries, Background Agent Definition Tool Authority, Desktop Release
  Artifact Authority, live-agent projection, and Retired Client Boundary;
- the shared
  [`openagents.agent_definition.v1` contract](../../packages/agent-runtime-schema/src/index.ts)
  and its deny-by-default tool-policy compiler;
- the Worker
  [definition routes](../../apps/openagents.com/workers/api/src/agent-definition-routes.ts),
  [run dispatch](../../apps/openagents.com/workers/api/src/agent-definition-run-routes.ts),
  and
  [trigger store](../../apps/openagents.com/workers/api/src/agent-definition-trigger-store.ts);
- the [Nostr relay contract](../../apps/nostr-relay/README.md) and
  [market transport policy](../../apps/nostr-relay/src/market-policy.ts);
- the [NIP draft status](../nips/README.md), which explicitly says those drafts
  are postponed and must not currently drive new work;
- the shared
  [`openagents.live_agent_graph.v1` schema](../../packages/agent-runtime-schema/src/live-agent-graph.ts)
  and its Desktop/Sync consumers; and
- the [CUT-26 installed-artifact closure](../sol/2026-07-12-cut-26-rc5-installed-artifact-closure.md)
  plus the later client-removal invariants.

The paper citation was checked against canonical SSRN metadata. This addendum
does not claim a page-by-page scholarly validation of every phrase attributed
to a numbered paper section. It evaluates the OpenAgents-specific conclusions
and implementation proposals in the submitted review. That distinction matters:
a paper may establish a useful governance principle without establishing that a
particular OpenAgents component is missing, that a proposed protocol is the
right remedy, or that the remedy belongs at the priority assigned by the
submission.

### Assessment of the five controls the submission says to protect

#### 1. Segregated authority is real and should remain non-negotiable

This is the strongest confirmed part of the submission. OpenAgents deliberately
separates execution, observation, public presentation, assignment admission,
promotion, payment, and settlement. A Pylon can execute owner-local work but
does not thereby gain payout or settlement authority. Cloud daemons can execute
and emit redacted receipts but cannot promote a public claim or mutate a money
ledger. A public UI can request or display state but cannot turn presentation
state into accepted-work authority.

The practical significance is larger than ordinary service decomposition.
These are security boundaries, not code-organization preferences. A new
projection, relay event, runtime message, or client control must not acquire
authority merely because it carries an authority-looking identifier. The root
ledger repeatedly applies the rule that a ref is evidence only until the owning
authority resolves it under the correct subject, scope, revision, and lifecycle.

The submission is therefore correct to say this boundary should be protected.
The more precise review rule is:

> For every proposed governance feature, name the component that decides,
> the component that executes, the component that observes, and the component
> that can revoke. Refuse designs in which transport or presentation silently
> becomes the decision authority.

That sharper formulation becomes important when evaluating Gap 1, where the
submission would move policy validity into a relay that is intentionally
transport-only.

#### 2. Deny-before-allow tool authority is a genuine shared primitive

The submission accurately identifies
`decideAgentDefinitionToolAuthority` and
`compileAgentDefinitionToolRuntimePolicy` as important primitives. The current
contract does more than attach a list of tools to an agent:

- explicit deny rules take precedence over ask and allow rules;
- ask rules create an operator-escalation outcome without authorizing the tool;
- an allow applies only to the matched typed tool reference;
- unmatched tools are denied; and
- the harness name is never itself authority.

This is a good example of deterministic authorization before execution. It is
also narrower than the submission sometimes implies. The compiler proves what
policy was produced from a particular `AgentDefinition` value. It does not, by
itself, prove that every external harness can be interrupted between arbitrary
tool calls, that a later definition revision reached an already-running
process, or that a provider-native tool event was intercepted before the tool
body began. Those are separate runtime and transport obligations.

The correct conclusion is not that tool governance is absent. It is that the
existing compile-and-enforce boundary is strong at dispatch and at supported
local tool dispatchers, while **revision coherence during a live run remains a
separate problem**. That is why the mid-run policy proposal deserves serious
design work.

#### 3. Scheduler budgets and failure auto-pause are meaningful circuit breakers

The existing background-agent scheduler enforces typed limits rather than
merely displaying configuration:

- invalid budgets are refused;
- `maxRunsPerDay` is checked against owner-and-definition usage;
- `maxCreditsPerDay`, when present, is checked before dispatch;
- `maxRunSeconds` becomes an assignment timeout; and
- the third consecutive failed or refused trigger attempt atomically pauses the
  trigger.

The scheduler also serializes cron scanning through a named Durable Object and
advances `next_run_at` before the row may be reconsidered. That prevents a
failed trigger from becoming a tight duplicate-dispatch loop.

These are real containment controls, so the submission is right to protect
them. It is wrong to treat them as equivalent to continuous per-tool anomaly
detection. Dispatch counts, wall-clock time, and daily credit reservations are
available at the scheduler boundary. Provider token deltas and tool-call
frequency may be visible only later, at a different runtime boundary, and some
providers report exact usage only when a turn completes. A new velocity feature
must respect those evidence limits rather than pretending the scheduler can see
every tool action in real time.

#### 4. VP-1 retirement of money authority is a binding scope decision

The submission correctly recognizes that OpenAgents chose to retire incomplete
money surfaces rather than leave ambiguous partial authority in production.
Under VP-1, payments, markets, tipping, wallet custody, billing credits, payout,
and settlement are outside the accepted MVP. Retained migrations, protocol
drafts, receipt records, and historical state are recovery or design evidence;
they are not live authority.

This has a direct consequence the original review does not carry through to its
recommendations. A proposal built around `budget_sats`, NIP-AC credit envelopes,
or an agent-to-agent market is not a small governance hardening of an accepted
surface. It is at least partially a money-surface revival. That requires a new
owner-approved custody and authority design, an explicit invariant change, and
a proof program. It cannot be smuggled back in as a low-effort relay tag.

The preserved NIP documents remain useful interoperability research, but their
README explicitly marks them postponed and says not to route new work from them
until the canonical roadmap or owner reactivates the lane.

#### 5. Owned CI is a repo policy; the paper analogy should remain an analogy

The factual statement is correct: this repository forbids GitHub-hosted CI and
runs automation on OpenAgents-owned infrastructure. That gives the project
direct control over execution environment, credentials, scheduling, retention,
and operational evidence.

The submitted review overstates the logical connection when it suggests that
third-party CI necessarily creates the same governance defect discussed in a
tokenized-finance control plane. Infrastructure ownership can reduce dependency
and improve auditability, but governance quality still depends on typed policy,
credential scope, reproducibility, observed receipts, revocation, and operator
control. Conversely, owning a runner does not make an untyped or unaudited
pipeline well governed.

The sound conclusion is narrower: owned CI is an established OpenAgents
invariant and is consistent with the paper's preference for controllable,
observable enforcement. It is not independent proof that every owned pipeline
is governed correctly, nor should the paper be cited as the original authority
for the repo's no-GitHub-Actions decision.

### Detailed review of the six proposed gaps

#### Gap 1: governance envelope on NIP-90 events — reject as written

The underlying question is legitimate: if an agent requests work from another
agent, how can an observer determine who authorized the request, which policy
applied, what limits were in force, and when that authority expired or was
revoked?

The proposed answer is placed at the wrong layer. The OpenAgents relay contract
is explicit: it is event transport only and grants no payment, identity,
moderation, assignment, payout, or settlement authority. NIP-90 market kinds
are accepted from any signing pubkey subject to structural, signature, size,
kind, retention, and rate-limit policy. The relay's job is to transport valid
events, not to decide whether a requester was institutionally authorized to
commission work.

Requiring the relay to reject an event unless its `authority_pubkey`,
`policy_ref`, credit-envelope hash, budget, expiry, and revocation relay are
"valid" raises questions the proposal does not answer:

- Which issuer is trusted to create the policy or credit envelope?
- What owner or tenant is the authority about?
- What audience may rely on it?
- Is validity structural, cryptographic, economic, or all three?
- Where is revocation state resolved, and what happens when that resolver is
  unavailable?
- Does relay acceptance now imply assignment or payment legitimacy?
- Can a relay replay a previously valid envelope after policy revocation?
- How is a policy revision ordered relative to an already-published job?

If the relay resolves those questions, it has become an authorization and
possibly money-policy service, contradicting its transport-only invariant. If
it checks only that the tags are present, the envelope looks authoritative
without proving authority and creates a false-green governance signal.

The specific use of NIP-AC also conflicts with current sequencing. NIP-AC is a
postponed draft, and VP-1 retired the active money authority that a
`budget_sats` envelope would presuppose.

An acceptable future shape, if the market lane is explicitly reactivated,
would keep the relay ignorant of institutional authorization:

1. Define a signed, versioned, public-safe capability or delegation receipt in
   a shared protocol package.
2. Let the relay enforce only its existing transport concerns plus ordinary
   event signature and size rules.
3. Require the requester, provider, or an application-layer admission service
   to resolve the receipt against its trusted issuer, audience, subject,
   revision, expiry, and revocation policy.
4. Keep assignment acceptance and any future payment decision in their owning
   receipt-backed systems.
5. Treat missing or unavailable resolution as an application-layer refusal,
   never as evidence that the relay itself grants authority.

Until the market and money lanes are owner-reactivated, this gap should be
classified **deferred / out of accepted scope**, not P1-low-effort.

#### Gap 2: governance state visibility — retain, but narrow the claim

The submission is directionally right that users and operators would benefit
from a coherent view of the policy and limits governing an active background
run. It is inaccurate to say governance state is wholly unobservable today.

OpenAgents already projects several relevant facts through separate typed
systems:

- `openagents.live_agent_graph.v1` carries stable run and agent references,
  provider/runtime/tool facts, status, attention, terminal state, versions,
  cursors, and typed edges;
- definition-trigger rows expose enable/pause state, pause reason,
  `next_run_at`, and consecutive failures;
- runtime events expose typed start, pause, interruption, cancellation,
  failure, and completion facts;
- exact token accounting exists for supported completed turns; and
- compiled tool policies and assignment timeouts exist at their enforcement
  boundaries.

What is missing is a **single revision-bound governance status view** that says
which policy revision an executor is currently enforcing and combines only the
facts that can be stated truthfully at that moment.

That projection should not be bolted into the live-agent graph casually. The
graph deliberately omits usage fields, and its consumers rely on stable graph
semantics. A separate schema may be clearer, linked to the same run and agent
refs. Whether it is named `governance_state.v1`,
`agent_governance_status.v1`, or something else should follow a shared-contract
design rather than the memo's proposed name.

At minimum, a credible projection needs:

- an exact owner/tenant-safe run reference;
- the definition ID plus immutable definition revision or digest;
- the compiled tool-policy digest actually acknowledged by the executor;
- the configured wall-clock and dispatch limits;
- exact observed usage with an explicit completeness state, never an estimate
  presented as current truth;
- trigger failure streak and pause/breaker state;
- policy-delivery and executor-acknowledgement state;
- `observedAt`, freshness bounds, and typed unavailable/stale reasons; and
- public/private redaction rules that exclude raw tool arguments, secrets,
  prompts, credentials, account identifiers, and private paths.

The UI should be the final consumer, not the starting point. The contract,
authority source, freshness model, and false-green tests must exist before a
Desktop right rail can claim to show governance state. A stale policy digest
must render as stale or blocked, not as the last known green state.

This is useful work, but its priority is coupled to Gap 3. A projection that
cannot distinguish "policy changed" from "executor acknowledged and enforced
the new restriction" risks making governance more visible-looking without
making it more truthful.

#### Gap 3: mid-run policy propagation — strongest finding, incomplete remedy

This is the submission's most important finding. Agent definitions can be
updated while a previously dispatched run continues under the policy compiled
from an earlier definition value. The current contract is strong about
deny-before-execution at supported enforcement boundaries, but it does not yet
define a universal revision protocol for active runs.

Adding a `policy_revision_signal` message is not sufficient on its own. A
complete design must answer:

- What constitutes the monotonic policy revision: a sequence, content digest,
  immutable definition revision, or a combination?
- Which service is authoritative for issuing that revision?
- How is the signal ordered against tool events already accepted or in flight?
- How does the Pylon acknowledge receipt and enforcement?
- What deadline applies when the Pylon is disconnected?
- What happens after process restart or durable-stream resume?
- Can a stale acknowledgement from a previous session satisfy a new run?
- Which runtimes can interpose before every tool call, and which can only
  cancel or pause a whole provider turn?
- How are broadened permissions distinguished from restrictive revocation?

The safest initial semantic split is asymmetric:

- **Restriction or revocation:** must become effective immediately at the
  owning control plane. If the active executor cannot prove acknowledgement
  before the next enforceable boundary, the run pauses or is cancelled. It
  must not continue on last-known policy.
- **Permission expansion:** should not hot-grant more authority to a running
  process merely because a definition changed. Expansion should require an
  explicit approval and a new dispatch or equally strong re-admission step.
- **Non-authority metadata change:** may update independently only when it
  cannot alter tools, network, secrets, spend, execution target, or external
  write scope.

Some harnesses expose a controllable tool dispatcher. Others execute tools
inside a provider runtime where OpenAgents observes events but cannot guarantee
an atomic checkpoint before each action. For those runtimes, the honest
fallback is whole-turn interruption or cancellation. The system must not claim
"re-evaluated before the next tool call" where the adapter cannot enforce that
promise.

This should begin as a bounded state machine, not as an unmodeled message type.
The state space should cover at least current/stale revisions, connected/
disconnected executor, delivered/unacknowledged/acknowledged policy,
restrictive/expansive change, tool boundary available/unavailable, restart,
timeout, and cancellation. Meaningful counterexamples should become regression
tests. The design must fail closed without weakening the existing runtime
policy merely to make the model pass.

Within this governance topic, policy revision coherence is the first technical
problem to solve because the visibility proposal depends on it.

#### Gap 4: mandatory Nostr identity binding — reject the mandatory coupling

The submission correctly distinguishes cryptographic identity from a complete
governance relationship, then draws the wrong implementation conclusion. A
Nostr public key proves control of a signing key for a Nostr event. It does not
by itself establish who is entitled to define an OpenAgents agent, which owner
scope applies, which tools are granted, which Pylon may execute, or which
issuer can revoke the relationship.

OpenAgents agent definitions are deliberately harness-agnostic and support
`own_pylon`, `cloud_workroom`, `worker_only`, and `test_fixture` lanes. Their
authority begins with an owner-scoped definition and a compiled tool policy,
not with a transport-specific key. Requiring every definition-backed Pylon
session to carry a `NostrIdentityBinding` would make one optional
interoperability identity mandatory for unrelated local, cloud, Worker, and
fixture execution paths.

The proposed type also duplicates compiled grants inside an identity binding.
That creates two potential policy sources: the durable agent definition and the
binding's embedded grant set. A policy change could update one without the
other, recreating exactly the synchronization problem the memo seeks to solve.

If cross-system identity binding becomes necessary, the shared abstraction
should be transport-neutral and should reference—not copy—the authoritative
policy. A possible conceptual shape would include:

- subject identity scheme and subject identifier;
- issuer and audience;
- owner/tenant scope;
- agent definition ID and immutable revision;
- compiled policy digest;
- issued/effective/expiry times;
- revocation reference and resolver policy; and
- proof or signature references appropriate to the selected identity scheme.

A Nostr adapter could produce or verify that generic binding for a Nostr market
or coordination path. A WorkOS/OpenAuth identity, device-local identity,
service identity, or another future scheme could use the same semantic
contract without pretending to be Nostr. The binding would remain evidence
submitted to an authority decision, not authority merely because it is signed.

Therefore the original mandatory proposal should be rejected. Optional,
transport-neutral authority evidence may be worth designing later, but only for
a concrete interoperability path that actually requires it.

#### Gap 5: autonomous containment — valid concern, wrong name and placement

The repository does not lack a kill switch. It already has assignment
timeouts, cancellation, interruption, pause states, bounded auto-approval,
scheduler serialization, daily run caps, failure auto-pause, provider-account
health breakers, and relay publish-rate limits. These mechanisms operate at
different boundaries and should not be collapsed into one claim.

The narrower missing feature is **behavior-rate anomaly detection for active
background runs**. A run might remain under its wall-clock and daily accounting
limits while producing an unexpectedly high rate of model turns, approvals, or
tool invocations.

Placing a `SpendVelocityMonitor` only in the background scheduler would be
misleading because the scheduler does not necessarily observe the relevant
events. Exact provider usage may arrive only at turn completion. Tool-call
events may be available for one harness and unavailable or lossy for another.
Network requests inside a provider tool may not pass through the same counter
as OpenAgents-native tools.

A safe design starts with an evidence inventory:

| Signal | Likely enforcement boundary | Truth limitation |
| --- | --- | --- |
| Runs per day | Definition scheduler / run store | Exact for admitted definition runs |
| Wall-clock duration | Assignment/runtime timeout | Exact for the bounded run lifecycle |
| Reserved credits | Definition run accounting | Exact only for the configured accounting model |
| Provider tokens | Provider completion/usage event | Often unavailable until a turn completes |
| OpenAgents tool calls | Typed tool dispatcher | Exact only when the dispatcher owns the call |
| Provider-native tool events | Provider adapter | May be delayed, lossy, or observation-only |
| Relay publishes | Relay per-pubkey bucket | Exact only per relay Durable Object instance/window |

Hard deterministic ceilings should come before statistical anomaly scoring.
For every supported signal, the system should define an exact window, cap,
owner scope, reset rule, persistence rule, and fail-closed outcome. Anomaly
detection can then operate as an additional conservative signal with explicit
false-positive handling; it must not be the only containment mechanism.

An operator acknowledgement should clear only the anomaly hold. It must never
override an exhausted hard cap, expired authority, revoked policy, unsafe
credential finding, or missing enforcement acknowledgement.

This is a reasonable later workstream after the policy-revision contract and
truthful status projection. Calling it "no kill switch" understates the
controls already present and obscures the actual observability problem.

#### Gap 6: legacy Autopilot migration guide — close with no implementation

This recommendation is based on an incorrect temporal and product premise. At
the cited repo snapshot:

- the legacy desktop lockout was already armed by default;
- CUT-26 was already closed with a signed, notarized, stapled, installed, and
  production-feed-verified OpenAgents Desktop artifact;
- `apps/autopilot-desktop` had been deleted by owner direction; and
- the new `openagents.agent_definition.v1` state was not a legacy
  Autopilot-Desktop-owned data model awaiting export.

Current `main` is even more explicit: OpenAgents Desktop is the only supported
desktop app, OpenAgents Mobile is the only supported mobile app, Pylon is the
supported terminal/Codex-capacity path, and the remaining legacy clients were
removed. Git history is the archive. Removed clients may be referenced as
historical evidence or negative sentinels, not restored as compatibility roots.

A new export/import guide would imply a supported source application and a
transferable state model that do not exist. It could also pressure the project
to revive deleted code solely to manufacture an exporter, contradicting the
owner's supersession decision.

If there is a concrete concern about user continuity, it should be asked in a
technology-neutral way: **Was any supported server-owned or device-local user
state stranded by the cutover?** That requires an inventory of actual persisted
schemas and user-visible data, not an assumption that legacy Autopilot stored
the later agent-definition model. If such an inventory finds a real stranded
state class, address that exact class through the current supported app or a
one-time offline recovery tool. Do not reopen a general legacy-client migration
program.

For this review, Gap 6 should be marked **factually unsupported and closed**.

### Cross-cutting design requirements missing from the submission

The six recommendations focus on feature names. A governance system succeeds or
fails on the semantics between those features. Any future work should answer the
following before code is accepted.

#### Revision identity and ordering

Every authority-bearing policy needs an immutable revision identity or content
digest. Signals, acknowledgements, runtime events, status projections, and
closeout receipts must bind to that exact revision. "Latest" is not a durable
identity. Out-of-order delivery, replay, process restart, delayed provider
events, and concurrent edits must have explicit outcomes.

#### Issuer, subject, audience, and scope

A signed object is incomplete unless the system knows who may issue it, which
subject it concerns, which audience may rely on it, and which actions it
governs. An identity key is not a substitute for those relationships.

#### Revocation semantics and resolver failure

The design must say when revocation becomes effective, where it is resolved,
how long cached status remains valid, and what happens when the resolver cannot
be reached. Authority-bearing execution should fail closed rather than silently
continue forever on stale positive state.

#### Evidence versus authority

Event IDs, job IDs, account refs, policy refs, configuration values, and status
documents are evidence inputs. None should become authority because it looks
official or was transported through an owned system. The owning decision point
must resolve and record the relationship.

#### Projection truth and privacy

A governance UI must distinguish configured, admitted, delivered,
acknowledged, enforced, observed, stale, unavailable, and terminal states. It
must also preserve existing redaction boundaries. Raw prompts, tool arguments,
provider payloads, secrets, local paths, private repo content, wallet material,
and customer data do not belong in public or broadly replicated governance
state.

#### Capability differences between runtimes

The system should publish a tested enforcement-capability matrix rather than
pretend every harness offers the same hooks. One adapter may enforce policy at
each OpenAgents tool dispatch; another may only observe provider-native events
and cancel a whole turn. The fallback behavior must be explicit and honest.

#### Formal model and regression conversion

Mid-run revision and revocation are well suited to a bounded state model. The
model should explore stale delivery, duplicate delivery, acknowledgement races,
restart, timeout, cancellation, permission expansion, restrictive revocation,
and tool-boundary availability. Counterexamples should be converted into
regression tests. Runtime policy must not be weakened to make the model pass.

#### No accidental revival of retired authority

Tags or schemas that mention sats, credit envelopes, settlement, markets, or
payout cannot be treated as neutral metadata under VP-1. Any active use must go
through the owner-approved invariant-change process for reviving money
authority.

### Revised disposition and sequencing

The canonical OpenAgents priority order remains owned by
[`docs/sol/MASTER_ROADMAP.md`](../sol/MASTER_ROADMAP.md). This addendum does not
relabel the overall program. If the owner later opens a bounded governance
workstream, the safe local order is:

| Order | Work item | Disposition |
| --- | --- | --- |
| 1 | Policy revision identity, propagation, acknowledgement, and fail-closed state machine | Retain and design first |
| 2 | Revision-bound governance status projection with freshness and redaction | Retain, dependent on order 1 |
| 3 | Deterministic hard velocity ceilings at boundaries with exact observations | Retain in narrower form |
| 4 | Anomaly detection over supported observed signals | Optional extension after hard ceilings |
| 5 | Transport-neutral authority binding for a concrete cross-system path | Deferred until a real path requires it |
| 6 | NIP-90 governance/credit envelope and market admission | Deferred unless market and money authority are explicitly reactivated |
| — | Mandatory Nostr binding for every definition-backed run | Reject |
| — | Legacy Autopilot export/import migration program | Close; no action |

This order deliberately puts enforceable revision semantics before visibility.
A dashboard should not become the first implementation of a policy concept it
cannot yet prove.

### Suggested bounded implementation slices

These are design slices, not automatically authorized issues or roadmap
commitments.

#### Slice A: policy revision contract and model

- Add an immutable revision/digest to the authoritative definition-policy
  boundary.
- Define restrictive, expansive, and non-authority changes separately.
- Model delivery, acknowledgement, timeout, disconnect, restart, and tool-hook
  availability.
- Specify the fail-closed action for each adapter capability class.
- Prove stale or mismatched acknowledgements cannot authorize continued work.
- Convert model counterexamples into focused regression tests.

#### Slice B: active-run policy transport

- Deliver the exact revision to every active definition-backed run.
- Bind the acknowledgement to owner, definition, run, executor generation, and
  policy digest.
- Refuse a broadened policy without explicit re-admission.
- Interrupt or pause when a restrictive revision cannot be enforced within its
  bound.
- Preserve durable resume without accepting an acknowledgement from a previous
  executor generation.

#### Slice C: governance status projection

- Project the configured revision, executor-acknowledged revision, and their
  relationship.
- Include only exact usage observations and label incomplete usage explicitly.
- Carry freshness, blocker, pause, failure-streak, and hard-limit facts.
- Enforce structural redaction before Sync or public projection.
- Add false-green tests for stale, missing, rejected, mismatched, and
  unavailable policy resolution.
- Only then add a Desktop presentation surface.

#### Slice D: rate containment

- Inventory exact signals per runtime adapter.
- Add deterministic ceilings at the boundary that owns each exact signal.
- Persist rolling-window state where restart must not reset the protection.
- Keep anomaly holds distinct from hard-cap exhaustion and authority
  revocation.
- Require explicit evidence before describing a provider-native tool rate as
  exact.

#### Slice E: optional interoperability binding

- Start only from a named product path and threat model.
- Define a transport-neutral binding that references the authoritative policy
  digest instead of copying grants.
- Implement Nostr as one adapter only if the named path uses Nostr.
- Keep verification above the relay unless the relay invariant is explicitly
  changed through the policy process.

### Source metadata correction

The preserved submission describes the Smith paper as a May 2026 paper. The
canonical [SSRN record](https://ssrn.com/abstract=6864258) lists Jay Smith as
the author, June 1, 2026 as the written date, and June 18, 2026 as the posting
date. The original 127-line submission remains unchanged for provenance; this
addendum records the correction rather than silently rewriting the imported
text.

### Final strategic assessment

OpenAgents does not lack governance in the broad sense. It already has strong
segregated authority, typed deny-by-default tool policy, bounded scheduling,
owner-scoped execution, cancellation and timeout paths, exact-evidence rules,
redaction boundaries, and explicit retirement of incomplete money authority.

The meaningful architectural gap is narrower and more actionable:

> OpenAgents needs a revision-coherent contract connecting policy mutation,
> active-run enforcement, executor acknowledgement, fail-closed revocation,
> and truthful observation across the runtimes that can actually support those
> guarantees.

That is a serious systems problem and worth solving. It should be solved through
typed shared contracts, explicit authority ownership, bounded formal modeling,
adapter-specific enforcement guarantees, and regression evidence—not by making
the relay an authorization service, making Nostr mandatory everywhere,
reviving postponed money semantics, or reopening deleted clients.

The merged review should therefore be read as a productive external challenge
to make runtime policy revision and visibility stronger. It is not approval of
the original P0/P1/P2 stack, effort estimates, or implementation locations.
