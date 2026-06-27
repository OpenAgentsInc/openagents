# IDAI Research-Area Map → OpenAgents Alignment

This folder analyzes the **IDAI corpus** — a 43-area research map produced by a
decentralized-AI institute — against OpenAgents' real systems, and turns it into
a **medium-term roadmap-alignment** view for prioritization.

## What the IDAI corpus is

`projects/idai/` (in the workspace root, outside this repo) holds 43 Markdown
files. Each file is one **research area** in decentralized / multi-agent AI, with:

- a short set of **tags** (`multi-agent systems`, `safety`, `oversight`,
  `economics`, `compute`, `infrastructure`, `distributed systems`,
  `law & governance`, `alignment`),
- the **companies**, **nonprofits**, **papers**, and **people** working on it.

It is a neutral field map, not an OpenAgents document. We treat it as a
high-quality external survey: a way to check our thesis against where the rest of
the decentralized-AI field is investing, and to find the seams where aligning our
roadmap creates the most leverage or differentiation.

The 43 areas: ad-hoc-evaluation-of-agents, agent-heterogeneity, agent-routing,
agentic-security, anomaly-drift-detection-in-multi-agentic-ai,
automated-mechanism-design, automated-protocol-design,
capability-delegation-revocation, collective-oversight,
collusion-cartel-detection, compute-inference-markets, cooperative-agents,
correlated-failure-in-agent-networks, cross-agent-credit-assignment,
decentralized-benchmarking, decentralized-data-sourcing, decentralized-governance,
decentralized-post-training, decentralized-serendipity,
decentralized-superalignment, decentralized-training,
distillation-as-a-decentralization-lever, distributed-verified-agentic-systems,
emergent-steganographic-collusion, executor-auditability,
exit-rights-forking-as-safety, human-empowerment-in-agent-interactions,
kill-switch-modes-recovery, legal-grounding-for-decentralized-ai,
multi-agent-interpretability, new-use-cases-for-decentralized-ai,
open-agent-telemetry, policy-to-constraints,
power-concentration-in-decentralized-ai, privacy-preserving-computation,
proof-of-humanity, safety-economics, speculative-execution-for-agentic-ai,
supercollaboration, superconsensus, sybil-resistance-for-agents,
trust-reputation-in-agentic-ai, universal-interoperability.

## How it maps to OpenAgents

The 43 areas map almost one-to-one onto our stack. Systems referenced throughout:

| OpenAgents system | What it is | Home |
| --- | --- | --- |
| **Khala** | OpenAI-compatible orchestrator (`openagents/khala`, base `https://openagents.com/api/v1`) over a network of agents; `ModelRouter`/coordinator, Blueprint/DSPy typed programs, durable resume, `openagents.khala.telemetry.v1` traces, Bitcoin revshare | `docs/khala/`, Worker |
| **Gym** | Eval + reward factory that trains Khala; typed environments (Terminal-Bench 2.0, OpenCode head-to-head, throughput-concurrency, long-context QA, M8); scored on **executed verification verdict + cost-per-accepted-outcome** | `docs/gym/` |
| **AgentCL** | Agent-evaluation / continual-eval work feeding the benchmark ladder | recent `docs/` synthesis |
| **Tassadar** | Supply side of the capability marketplace + the Percepta/decentralized training run; `compute.tassadar_executor_poc.v1` (green), `artanis.tassadar_evolution_loop.v1` dispatch->verify->accumulate loop | `docs/tassadar/`, `psionic` |
| **Pylon fleet + Codex/Claude supervisors** | Contributor/owned provider nodes that execute work; supervised on our GCE ("our cloud") | `docs/pylon/`, `cloud` |
| **Trace -> reputation -> Bitcoin-payout economy** | Replay-verification floor (Verified/Rejected challenge receipts at `/trace/{uuid}`), promise engine, settlement (Spark primary / MDK checkouts), MPP, labor market | `docs/mpp/`, `docs/labor/`, `docs/metrics/` |
| **Forum** | Agent registration, posting, web-of-trust identity, BOLT12 tipping | `docs/forum/` |
| **Collective intelligence as an economy** | The thesis: collective intelligence built as an open, Bitcoin-metabolized economy selected by verifiable value | `docs/collective-intelligence/` |

## How to read the analysis

Read [`roadmap-alignment.md`](roadmap-alignment.md). It contains:

1. An **at-a-glance table** — every IDAI area -> owning OpenAgents system ->
   posture (ahead / on-par / behind / absent) -> opportunity -> timeframe
   (now / medium / watch) -> suggested issue or epic theme.
2. The **medium-term priorities** — the highest-leverage areas with a concrete
   "what we'd build."
3. **Where we are already differentiated** (claim it) vs. **gaps that are
   strategic risks**.
4. **Notable companies/projects per area** worth tracking or learning from.
5. **Suggested new epics/issues** (titles + one-line scope) — proposed only,
   for Artanis to prioritize. Not filed.

### Conventions and honesty rules

- **Direction vs. shipped** is labeled explicitly. "Ahead" means we have a live,
  differentiated capability; "on-par" means comparable to the field; "behind"
  means the field is ahead of us; "absent" means we have effectively nothing yet.
- Invariants respected: Cloudflare-native, Bun/Effect/Foldkit, no vendoring of
  external code, reuse `@openagentsinc/three-effect` for visualization, and no
  weakening of the no-resale / verification-floor policy.
- Citations point at the specific `projects/idai/<area>.md` file so claims are
  traceable to the source survey.
