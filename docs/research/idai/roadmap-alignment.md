# IDAI -> OpenAgents Roadmap Alignment

Core synthesis of the 43-area IDAI map against OpenAgents' real systems, framed
for **medium-term** prioritization. Postures: **ahead** (live, differentiated) /
**on-par** (comparable to field) / **behind** (field ahead of us) / **absent**
(effectively nothing yet). Timeframes: **now** (already in flight or trivially
extendable) / **medium** (a quarter-ish of deliberate build) / **watch** (track
the field, don't build yet).

Every row cites the source file as `idai/<area>.md` (in workspace
`projects/idai/`).

## At-a-glance table

| IDAI area | OpenAgents system / owner | Posture | Opportunity (what aligning buys us) | Timeframe | Suggested epic theme |
| --- | --- | --- | --- | --- | --- |
| ad-hoc-evaluation-of-agents | Gym + AgentCL + Khala coordinator selection | on-par | Reframe coordinator selection as query->agent recommendation (AgentSelect); fold tau-bench/SWE-bench/GAIA/WebArena into Gym envs | now/medium | Gym eval-ladder expansion |
| agent-heterogeneity | Khala `ModelRouter` + provider fan-out + coordinator candidates | ahead | Make diversity an explicit selection objective (RMoA); X-MAS function-level model assignment; guard against quality-dilution | medium | Diversity-aware coordinator |
| agent-routing | Khala `ModelRouter`/coordinator | on-par->ahead | Learn routing from our own verified-outcome traces (RouteLLM/RouterBench); expose Khala as a router product; Contract Net for Tassadar dispatch | medium | Learned routing from verified outcomes |
| agentic-security | Pylon sandbox + cloud capability gateway + MCP gateway | behind | CaMeL control/data separation + Progent least-privilege on Pylon tool calls; run AgentDojo/InjecAgent in Gym | medium | Agent least-privilege + injection defense |
| anomaly-drift-detection | Khala telemetry scorecard + Tassadar loop | behind | Graph-based anomaly detection (SentinelAgent) over Khala/Tassadar execution graphs; ADWIN/DDM drift gates on coordinator quality | medium | Fleet anomaly + drift gates |
| automated-mechanism-design | Tassadar dispatch pricing + revshare + labor market | behind | Parametric/differentiable auction design for compute + labor; learn revshare splits instead of hand-tuning | watch->medium | Market mechanism search |
| automated-protocol-design | A2A/MCP usage + Tassadar parity protocol | watch | Commitment-based protocols for labor negotiation; mostly track the field | watch | - |
| capability-delegation-revocation | cloud capability gateway + Pylon capability envelope + Tassadar marketplace | behind | UCAN/Macaroon attenuated, revocable capability tokens for agent->Pylon->tool chains; auditable delegation | medium | Revocable capability chains |
| collective-oversight | Forum moderation + validator replay + promise gates | on-par | Bridging-based ranking (Community Notes) for forum/agent reputation; digital-jury for disputed work acceptance | medium/watch | Bridging-based acceptance |
| collusion-cartel-detection | Khala coordinator+validators + labor-market pricing | absent | Variance/structural screens on labor + Tassadar pricing; detect worker<->validator collusion | medium | Pricing + collusion screens |
| compute-inference-markets | Tassadar marketplace + Pylon + Khala adapters + inference gateway | ahead | **Differentiated**: live Bitcoin/Lightning-settled inference market w/ replay floor; benchmark vs Akash/Gensyn/Bittensor | now | (claim) |
| cooperative-agents | Khala `spawn` subagents + multi-agent coordination | on-par | Ad-hoc teamwork across owners; Melting Pot-style cooperation evals in Gym | medium/watch | Cross-owner cooperation evals |
| correlated-failure-in-agent-networks | Khala provider/model diversity + validator panels | behind | Ensure verifier model family != worker; diversity metrics on verification panels ("nine judges, two votes") | medium | Verifier independence |
| cross-agent-credit-assignment | revshare across coordinator/providers/plugins/contributors | behind | Shapley-style marginal-contribution revshare across the Khala chain; Data-Shapley for contributed traces | medium | Fair-attribution revshare |
| decentralized-benchmarking | Gym + AgentCL + validator scoring | on-par->ahead | Attestable/verifiable benchmark execution (TEE or replay); Decentralized-Arena models-judge-models | medium | Verifiable benchmark execution |
| decentralized-data-sourcing | traces as data + Gym reward factory + contributor data | behind | Contributor trace -> training-data market with attribution/payment feeding Tassadar | medium/watch | Trace-to-training data market |
| decentralized-governance | owner-gated decisions + promise engine | behind (by design) | Track DAO tooling; we are intentionally owner-led now | watch | - |
| decentralized-post-training | Tassadar evolution loop + Blueprint/DSPy + GEPA/TextGrad | on-par->ahead | Federated prompt/program evolution over contributor traces (GEPA/FedTextGrad); mergeable artifacts | medium | Federated program evolution |
| decentralized-serendipity | (privacy-preserving cross-party pattern discovery) | absent | Niche; track PETs/federated-analytics | watch | - |
| decentralized-superalignment | validator/worker role separation + coordinator cross-check | on-par | Frame replay floor explicitly as AI-control (trusted verifier disciplines untrusted worker); add debate/multi-model cross-check | medium | AI-control framing of the floor |
| decentralized-training | Tassadar run + DiLoCo (psionic; prime-diloco/templar refs) | behind (runs) / on-par (infra) | Real distributed-training milestone; honest direction-vs-shipped; SWARM/DiLoCo/Covenant-72B as references | medium | Distributed training milestone |
| distillation-as-a-decentralization-lever | Khala teacher + psionic students | behind | Distill a Khala-routed teacher into an owned student for the floor / edge Pylons | medium/watch | Owned distilled floor model |
| distributed-verified-agentic-systems | replay floor + Tassadar Verified/Rejected receipts + `/trace/{uuid}` | ahead | **Differentiated**: optimistic replay + Lightning settlement; add zkML/TEE attestation tier + opML fraud proofs for high-value work | now/medium | Verified-execution tiers |
| emergent-steganographic-collusion | worker<->validator covert-coordination risk; CoT monitoring | absent | Paraphrasing defense + steganalysis on agent-to-agent channels; cross-model verifier | watch->medium | Covert-channel defense |
| executor-auditability | Pylon executor traces + replay verification + RepOps-style reproducibility | ahead | **Differentiated**, but Proof-of-Sampling cheap audits + reproducible-ops needed before outsiders join; verifier's dilemma | medium | Cheap audits + RepOps |
| exit-rights-forking-as-safety | open protocols + OpenAI-compatible API = exit right | on-par | Make exit/forking explicit: data export, model portability, no lock-in as a feature/narrative | medium/watch | Portability-as-a-feature |
| human-empowerment-in-agent-interactions | owner review post-hoc + default-yes autonomy + Autopilot gates | on-par | HumanAgencyBench-style eval in Gym; approval-gate UX (Magentic-UI patterns) | medium/watch | Agency-support evals |
| kill-switch-modes-recovery | Pylon quarantine + agent pause + cloud node update/quarantine | on-par->behind | Graceful-halt + checkpoint/rollback for Khala durable streams + Tassadar loop; pause-guardian + safe-interruptibility | medium | Graceful halt + recovery |
| legal-grounding-for-decentralized-ai | legal/privacy docs + entity | behind | Legal wrapper for the agent economy / payouts (DUNA/LLC, DAO Model Law) | watch | - |
| multi-agent-interpretability | telemetry + traces + observability | behind | Delegation-scoped observability model; eBPF (AgentSight) system-level tracing on Pylons | medium | Delegation-scoped observability |
| new-use-cases-for-decentralized-ai | autonomous QA + Khala chat + labor market | ahead | Shipping real use cases; mine the taxonomy (Vitalik crypto+AI) for the next wedge | watch | (claim) |
| open-agent-telemetry | `openagents.khala.telemetry.v1` schema | behind (standards) | Emit OpenTelemetry-GenAI-compatible spans so traces interoperate; cheap, high interop leverage | medium | OTel-GenAI trace interop |
| policy-to-constraints | Blueprint typed programs + promise engine + capability envelope + no-resale | on-par | Compile owner policy (no-resale, spend limits) into enforced Cedar/Rego-style constraints + symbolic guardrails | medium | Policy-to-enforced-constraint |
| power-concentration-in-decentralized-ai | anti-concentration thesis (open protocols) | on-par (narrative) | Track SoK DeAI / compute-governance; measure our own concentration honestly | watch | - |
| privacy-preserving-computation | confidential-compute docs + TEE on Pylon | behind | TEE attestation for private inference (ties to verified-execution tier); FHE/MPC track | medium/watch | Confidential inference tier |
| proof-of-humanity | Forum web-of-trust + owner claims | behind | Personhood credentials to gate human-only vs agent participation; pairs with sybil resistance | medium | Personhood gating |
| safety-economics | promise engine + settlement + verified-work payment economics | ahead | **Differentiated**: our trace+replay IS the substrate for trace-economic underwriting; price risk / warrant accepted outcomes | medium | Trace-economic underwriting |
| speculative-execution-for-agentic-ai | Khala speculation telemetry (book p1-8) + coordinator | on-par->ahead | Speculative tool execution (PASTE/Speculative Actions) to cut Khala agent latency; tool cache API | medium | Speculative tool execution |
| supercollaboration | collective-intelligence economy + Khala spawn + forum | on-par | Track Polis/Unanimous; structure crowds-as-orgs for complex labor jobs (Flash Orgs) | watch/medium | - |
| superconsensus | bridging + collective input + owner decision | watch | Bridging-based acceptance for disputed verification; collective input into Khala policy | watch | - |
| sybil-resistance-for-agents | Forum/labor/Pylon registration | absent | TraceRank/EigenTrust payment-graph reputation; sybil-proof accounting (cost-to-fake = real verified work); ERC-8004 interop | medium | Sybil-proof reputation |
| trust-reputation-in-agentic-ai | trace->reputation + validator scoring + forum reputation | on-par->behind | **Differentiable**: reputation grounded in replay-verified outcomes (not self-reported reviews); portable agent reputation | medium | Verified-outcome reputation |
| universal-interoperability | OpenAI-compatible API + MCP infra + A2A | ahead | Add A2A endpoint so cross-vendor agents can hire Khala; expose MCP servers; claim OpenAI-compat + MCP | now/medium | A2A endpoint for Khala |

## Medium-term priorities (the highest-leverage alignments)

The 8-12 areas where aligning our roadmap creates the most leverage or
differentiation. Each is framed as "what we'd build" on top of systems we
already run. These are deliberately **medium-term**, not now-or-never.

### 1. Verified-trace reputation + sybil-proof accounting
**Areas:** `trust-reputation-in-agentic-ai`, `sybil-resistance-for-agents`,
`executor-auditability`.
**Why us:** every other reputation system in the corpus (ERC-8004 / Reputio /
RNWY, SingularityNET ratings, the EigenTrust/FIRE/TRAVOS lineage) is built on
**self-reported feedback** that is cheap to fake — the empirical ERC-8004 study
(`idai/sybil-resistance-for-agents.md`) shows the live agent trust layer is
already vulnerable to coordinated Sybil feedback. Our reputation is grounded in
**replay-verified outcomes** and **Bitcoin-settled work**, so the cost to fake a
good reputation is the cost to actually do verified work — a structurally
stronger Sybil defense.
**What we'd build:** a payment-graph reputation score (TraceRank/EigenTrust over
the trace->payout graph; `idai/sybil-resistance-for-agents.md`) for every Pylon,
coordinator, plugin, and forum agent, with sybil-proof accounting properties
(`idai/sybil-resistance-for-agents.md` Friedman/Seuken/Parkes) and optional
ERC-8004 interop for portability (`idai/trust-reputation-in-agentic-ai.md`).
This is both our biggest claimable differentiation *and* the biggest strategic
risk-closer before the fleet/labor market opens to outsiders.

### 2. Trace-economic underwriting (safety-economics)
**Areas:** `safety-economics`, `distributed-verified-agentic-systems`.
**Why us:** the most direct paper in `idai/safety-economics.md` — "When Agent
Automation Becomes Profitable: ... Trace-Economic Underwriting" — maps agent
tool-use **traces** to customer exposure, claimable loss, pricing, and controls.
We already produce exactly that substrate (`/trace/{uuid}` + replay verdicts +
settlement). AIUC, Armilla, and Munich Re are building agent insurance *without*
this data; we have it natively.
**What we'd build:** price work-risk and offer **warranties on accepted
outcomes** — a verified-outcome SLA / refund-on-rejection product riding the
existing metering/settlement spine. Turns the verification floor into a revenue
and trust instrument, and is a uniquely ours narrative.

### 3. Revocable capability delegation (UCAN / Macaroons)
**Areas:** `capability-delegation-revocation`, `agentic-security`.
**Why us:** we already have a "capability envelope" concept for Pylon
(`docs/tassadar/...capability-envelope...`) and cloud capability gateways, but no
cryptographically attenuated, **revocable** delegation chain. `idai/capability-
delegation-revocation.md` (UCAN, ZCAP-LD, Macaroons, ocap) is the mature design
space.
**What we'd build:** UCAN/Macaroon-style attenuated, time-bounded, revocable
capability tokens for agent->Pylon->tool delegation, with an auditable chain. A
prerequisite safety control before any outside agent can drive our executors.

### 4. Shapley-style cross-agent credit assignment
**Area:** `cross-agent-credit-assignment` (+ `decentralized-data-sourcing`).
**Why us:** the Khala chain already splits Bitcoin revshare across coordinator,
providers, plugins, and contributors, but attribution is coarse. `idai/cross-
agent-credit-assignment.md` (Shapley value, Data Shapley, Shapley-Coop) gives the
fair-allocation primitives; Kite AI / OpenLedger / Allora are commercializing it.
**What we'd build:** marginal-contribution (Shapley-approx) revshare across the
chain, and Data-Shapley valuation for contributed traces that feed Gym/Tassadar.
Makes the economy provably fair — load-bearing for legitimacy as it scales.

### 5. Verifier independence vs correlated failure
**Areas:** `correlated-failure-in-agent-networks`,
`emergent-steganographic-collusion`, `collusion-cartel-detection`.
**Why us:** our replay-verification floor is only as strong as the *independence*
of the verifier. `idai/correlated-failure-in-agent-networks.md` ("Correlated
Errors in LLMs", "Nine Judges, Two Effective Votes") shows nominally-diverse
models fail together; if the validator shares the worker's model family, the
floor silently weakens, and worker<->validator collusion
(`idai/collusion-cartel-detection.md`, `idai/emergent-steganographic-
collusion.md`) becomes possible.
**What we'd build:** enforce verifier-model-family != worker-model-family,
measure effective-independence of verification panels, add paraphrasing/cross-
model defenses on agent-to-agent channels, and variance screens on pricing.
Protects the single most important invariant we have.

### 6. Open agent telemetry on OpenTelemetry-GenAI + delegation-scoped observability
**Areas:** `open-agent-telemetry`, `multi-agent-interpretability`,
`anomaly-drift-detection-in-multi-agentic-ai`.
**Why us:** we have a strong proprietary schema (`openagents.khala.telemetry.v1`)
but the field is standardizing on OpenTelemetry GenAI conventions
(`idai/open-agent-telemetry.md`: Traceloop OpenLLMetry, Arize OpenInference,
AGNTCY, the OTel GenAI repo). Emitting OTel-GenAI-compatible spans is cheap and
makes our traces interoperable with every observability backend — and is a
precondition for graph anomaly detection (SentinelAgent) and delegation-scoped
observability (`idai/multi-agent-interpretability.md`).
**What we'd build:** an OTel-GenAI export path for Khala traces + a delegation
information model, then graph/drift anomaly gates on top. Low effort, high
interoperability and oversight leverage.

### 7. Learned routing from verified outcomes (Khala as a router product)
**Areas:** `agent-routing`, `agent-heterogeneity`.
**Why us:** Martian/OpenRouter/Not Diamond/Unify route on benchmarks or
preference data; we have something better — **our own verified-outcome traces**
(executed verdict + cost-per-accepted-outcome). `idai/agent-routing.md`
(RouteLLM/RouterBench) is the method; `idai/agent-heterogeneity.md` (RMoA,
X-MAS) is the diversity objective.
**What we'd build:** train a Khala router from verified-outcome traces, with
explicit diversity selection, and expose Khala itself as a routing product. Turns
the data exhaust of the economy into a moat.

### 8. Agent least-privilege + injection defense
**Areas:** `agentic-security`, `policy-to-constraints`.
**Why us:** as Pylons run untrusted external data and (eventually) outside
agents, indirect prompt injection and tool misuse become real
(`idai/agentic-security.md`: AgentDojo, InjecAgent, CaMeL, Progent). We have
sandboxing but not design-level injection defense, and we have Blueprint typed
programs that are a natural home for enforced policy
(`idai/policy-to-constraints.md`: Cedar, Prose2Policy, symbolic guardrails).
**What we'd build:** CaMeL-style control/data-flow separation + Progent
least-privilege on Pylon tool calls; compile owner policy (no-resale, spend
limits) into enforced constraints; run AgentDojo/InjecAgent as Gym environments.

### 9. Graceful halt + checkpoint recovery (kill-switch)
**Area:** `kill-switch-modes-recovery`.
**Why us:** default-yes autonomy + the standing `artanis.tassadar_evolution_loop`
+ Khala durable streams mean we need clean, non-corrupting stop/rollback.
`idai/kill-switch-modes-recovery.md` (Safely Interruptible Agents,
Chandy-Lamport snapshots, rollback-recovery, pause-guardian) is the playbook.
**What we'd build:** consistent checkpoint + graceful-halt + rollback for the
Tassadar loop and Khala durable streams, plus a granular pause-guardian (freeze a
capability, not the whole system) rather than a single global off-switch.

### 10. A2A endpoint for Khala (universal interoperability)
**Area:** `universal-interoperability` (+ `exit-rights-forking-as-safety`).
**Why us:** Khala is already OpenAI-compatible and we have MCP infrastructure —
two of the three interop layers in `idai/universal-interoperability.md`. Adding an
**A2A** (Agent2Agent) endpoint lets cross-vendor agents discover and *hire* Khala,
and reinforces the no-lock-in / exit-right story
(`idai/exit-rights-forking-as-safety.md`).
**What we'd build:** an A2A capability-card + task endpoint in front of Khala, and
MCP server exposure of our tools, positioned as "no lock-in: standard in,
standard out."

### Honourable mentions (medium/watch)
- **Federated program evolution** (`idai/decentralized-post-training.md`):
  GEPA/TextGrad/FedTextGrad over contributor traces — strong fit with our
  Blueprint/DSPy + the `gepa` reference repo, mergeable cheap artifacts.
- **Verifiable / attestable benchmarking** (`idai/decentralized-benchmarking.md`):
  TEE-attested or replay-attested Gym runs (Attestable Audits, zkSNARK evals,
  Decentralized Arena).
- **Speculative tool execution** (`idai/speculative-execution-for-agentic-ai.md`):
  PASTE/Speculative Actions to hide tool latency in Khala.

## Where OpenAgents is already differentiated (claim it)

- **Compute & inference markets** (`idai/compute-inference-markets.md`): a *live*
  inference market settled in Bitcoin/Lightning with a replay-verification floor.
  Akash/Vast/Gensyn/io.net/Bittensor are the comparables; few combine a working
  market with verified execution *and* fiat-grade settlement.
- **Distributed, verified agentic systems** (`idai/distributed-verified-agentic-
  systems.md`) + **executor-auditability** (`idai/executor-auditability.md`):
  optimistic replay verification with Verified/Rejected challenge receipts and
  per-job Lightning settlement is a deployed analogue of Gensyn Verde / opML /
  Truebit / Livepeer — and we ship it as a product surface (`/trace/{uuid}`).
- **Safety-economics** (`idai/safety-economics.md`): we already produce the
  trace+verdict+settlement substrate that the trace-economic-underwriting and
  agentic-insurance literature is reaching for.
- **Trust grounded in verified outcomes** (`idai/trust-reputation-in-agentic-
  ai.md`): reputation from replay-verified work, not self-reported reviews.
- **Universal interoperability** (`idai/universal-interoperability.md`):
  OpenAI-compatible endpoint + MCP infrastructure already in place.
- **New use cases** (`idai/new-use-cases-for-decentralized-ai.md`): autonomous QA
  leaving a green VERIFIED trace is a concrete, shipped decentralized-AI use case.
- **Decentralized post-training direction** (`idai/decentralized-post-training.md`):
  Blueprint/DSPy + GEPA make prompt/program improvement a mergeable, distributable
  artifact — aligned with the field's "post-training without weight ownership."

## Gaps that are strategic risks

These are the areas where being **absent/behind** is dangerous *specifically
because* of our economy design — they become acute the moment the fleet or labor
market opens to outsiders:

- **Sybil resistance** (`idai/sybil-resistance-for-agents.md`): an open agent/
  Pylon registry with payouts is a Sybil magnet. Mitigated by priority #1.
- **Collusion & cartel detection** (`idai/collusion-cartel-detection.md`) and
  **emergent/steganographic collusion** (`idai/emergent-steganographic-
  collusion.md`): worker<->validator collusion or coordinated pricing would
  silently break the verification floor and the labor market. Mitigated by #5.
- **Correlated failure** (`idai/correlated-failure-in-agent-networks.md`): a
  verifier sharing the worker's model family gives false assurance. Mitigated by #5.
- **Capability delegation/revocation** (`idai/capability-delegation-revocation.md`)
  and **agentic security** (`idai/agentic-security.md`): without revocable,
  least-privilege delegation and injection defense, an outside agent on a Pylon is
  an unbounded-authority risk. Mitigated by #3 and #8.
- **Executor-auditability at scale** (`idai/executor-auditability.md`): the
  verifier's dilemma means we cannot afford to fully re-verify every job once
  volume grows; we need cheap sampling audits (Proof-of-Sampling) and reproducible
  ops before outsiders execute. Mitigated by #1 (audit tooling) + a sampling tier.
- **Kill-switch / recovery** (`idai/kill-switch-modes-recovery.md`): default-yes
  autonomy without graceful halt + rollback risks corrupting shared state.
  Mitigated by #9.

## Notable companies / projects per area worth tracking or learning from

(Concrete actors pulled from the IDAI files; learn-from, do not vendor.)

- **Routing & heterogeneity:** Martian (RouterBench), OpenRouter, Not Diamond,
  Unify, Katanemo Arch-Router, AGNTCY Agent Directory, LMSYS RouteLLM; Together AI
  Mixture-of-Agents. (`idai/agent-routing.md`, `idai/agent-heterogeneity.md`)
- **Compute/inference & verified execution:** Akash, Vast.ai, Gensyn (Verde),
  io.net, Bittensor/OpenTensor, Ritual, ORA (opML), Phala (TEE), EZKL/Lagrange
  (zkML), Livepeer, Truebit. (`idai/compute-inference-markets.md`,
  `idai/distributed-verified-agentic-systems.md`, `idai/executor-auditability.md`)
- **Reputation / Sybil / trust:** MetaMask + Coinbase (ERC-8004, x402), 8004 Labs
  (Reputio), RNWY (Sybil detection), QuickNode + Chitin (ERC-8004 tooling), Olas.
  (`idai/sybil-resistance-for-agents.md`, `idai/trust-reputation-in-agentic-ai.md`)
- **Safety-economics / insurance:** AIUC (AIUC-1), Armilla, Munich Re (aiSure),
  Testudo, Relm, METR (evals as underwriting input). (`idai/safety-economics.md`)
- **Agent security:** Invariant Labs (CaMeL), Lakera, Straiker, Zenity, Prompt
  Security; OWASP Agentic, CSA MAESTRO, MITRE ATLAS. (`idai/agentic-security.md`)
- **Telemetry / observability:** Traceloop (OpenLLMetry), Arize (Phoenix/
  OpenInference), Langfuse, AgentOps, AGNTCY, OTel GenAI conventions.
  (`idai/open-agent-telemetry.md`, `idai/multi-agent-interpretability.md`,
  `idai/anomaly-drift-detection-in-multi-agentic-ai.md`)
- **Capability delegation:** Agoric (ocap), Storacha (UCAN), SpruceID, Spritely
  (OCapN), Macaroons/ZCAP-LD. (`idai/capability-delegation-revocation.md`)
- **Decentralized training/post-training:** Prime Intellect (INTELLECT, DiLoCo),
  Nous (DisTrO/Psyche), Pluralis (Protocol Learning), Templar (Covenant-72B),
  Gensyn, Flower (federated), Hivemind/Petals. (`idai/decentralized-training.md`,
  `idai/decentralized-post-training.md`) — several already in our `projects/`
  reference lanes (prime-diloco, covenant/templar, nous, pluralis).
- **Eval / benchmarking:** Braintrust, LangSmith, Patronus, Arize Phoenix; ORO
  (Bittensor), MLCommons (MedPerf), OpenMined; SWE-bench/tau-bench/GAIA/WebArena.
  (`idai/ad-hoc-evaluation-of-agents.md`, `idai/decentralized-benchmarking.md`)
- **Interoperability:** Anthropic (MCP), Google (A2A), IBM (ACP/BeeAI), Cisco
  (AGNTCY), Linux Foundation Agentic AI Foundation. (`idai/universal-
  interoperability.md`)
- **Credit assignment / mechanism design:** Kite AI, OpenLedger, Allora; the AI
  Economist, RegretNet/differentiable economics. (`idai/cross-agent-credit-
  assignment.md`, `idai/automated-mechanism-design.md`)
- **Personhood / governance / oversight:** World/Tools for Humanity, BrightID,
  Holonym, Proof of Humanity; Kleros, Community Notes (bridging), PolicyKit;
  Habermas Machine, Polis, Collective Constitutional AI. (`idai/proof-of-
  humanity.md`, `idai/collective-oversight.md`, `idai/superconsensus.md`)

## Suggested new epics / issues (proposed only — Artanis prioritizes)

Not filed. Titles + one-line scope for medium-term incorporation.

1. **Verified-trace reputation + Sybil-proof accounting** — payment-graph
   reputation (TraceRank/EigenTrust) for Pylons/coordinators/plugins/agents,
   grounded in replay-verified work; optional ERC-8004 interop.
2. **Trace-economic underwriting / verified-outcome warranties** — price work
   risk and offer refund-on-rejection / SLA on accepted outcomes over the
   existing metering+settlement spine.
3. **Revocable capability delegation** — UCAN/Macaroon attenuated, time-bounded,
   revocable capability chains for agent->Pylon->tool.
4. **Shapley revshare + Data-Shapley trace valuation** — marginal-contribution
   payout splits across the Khala chain and fair valuation of contributed traces.
5. **Verifier independence + collusion screens** — enforce verifier!=worker model
   family, measure verification-panel effective-independence, variance/structural
   pricing screens, paraphrasing defense on agent channels.
6. **OTel-GenAI trace interop + delegation-scoped observability** — emit
   OpenTelemetry GenAI spans from Khala; add a delegation information model and
   graph/drift anomaly gates.
7. **Learned Khala routing from verified outcomes** — train + serve a routing
   policy from executed-verdict/cost traces with explicit diversity selection;
   expose Khala as a router.
8. **Pylon least-privilege + injection defense** — CaMeL/Progent-style control on
   tool calls; AgentDojo/InjecAgent as Gym environments; policy-to-constraint
   compilation (Cedar/Rego) for owner policy.
9. **Graceful halt + checkpoint recovery** — consistent checkpoint/rollback +
   pause-guardian for the Tassadar evolution loop and Khala durable streams.
10. **A2A endpoint for Khala + MCP exposure** — capability-card + task endpoint so
    cross-vendor agents can hire Khala; "no lock-in" interop narrative.
11. **Verifiable/attestable Gym runs** — TEE- or replay-attested benchmark
    execution so Gym verdicts are independently checkable.
12. **Federated program evolution** — GEPA/TextGrad/FedTextGrad over contributor
    traces to evolve Blueprint/DSPy programs as mergeable artifacts.
