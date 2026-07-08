# Fable corpus — reading notes (openagents)

Date: 2026-07-08
Reader: Grok
Corpus: `docs/fable/` on `origin/main`

## What this folder is

`docs/fable/` is the monorepo's **synthesis + execution layer**:

1. Orientation/analysis docs (audits, designs, strategy essays)
2. Consolidated roadmaps with issue-sized tasks
3. Operating procedure for multi-agent delivery

Fable docs flip no promise state and are not public marketing. They map
code, issues, and product intent so fleets and humans can execute.

## Start-here documents

| Doc | Role |
| --- | --- |
| `MASTER_ROADMAP.md` | **Top-level sequencing wins** when docs disagree. Phases P0–P7. |
| `README.md` | Index of roadmaps and analysis docs |
| `EXECUTION.md` | How work ships: issues, worktrees, PRs, review, token counters |
| `ROADMAP.md` | Desktop/harness/fleet workstreams (content authority; sequencing demoted) |
| `ROADMAP_QA.md` | Nightly QA cycle, budgets, visual/mobile tiers |
| `ROADMAP_BIZ.md` | Business fulfillment funnel (intake → multiply) |
| `ROADMAP_AFTER.md` | Market-contact / after-launch economy lanes |
| `ROADMAP_BACKGROUND_AGENTS.md` | Standing agent definitions / cloud lane |

## The one thesis (product)

From the 2026-07-07 strategy set, compressed:

> OpenAgents is the front door to the agentic economy — coding agents you
> dispatch from a phone, AI employees that run repeatable business work,
> and the trust infrastructure (receipts, isolation, verification,
> payments) that makes delegation safe. Come for the tool; stay for the
> network. Own the layer that matters (up to private models in *your*
> walls). Every claim ships with a receipt.

Two market facts the essay treats as already true:

1. **Bottom:** small-business operators already run named, scheduled,
   tool-connected agents on duct tape ($VPS + harness + subscription).
   Failures: always-on hosting and supply-chain trust.
2. **Top:** institutional sovereignty (own knowledge, swap models, log
   everything, control hardware) is mainstream — but enterprise delivery
   cannot reach ~30M smaller businesses at price/packaging that fits.

OpenAgents' bet: automate the forward-deployed motion, package trust for
the middle, sell work not seats.

## Product suite (four faces, one spine)

| Surface | Job |
| --- | --- |
| **Khala Code mobile** | Front door: App Store → GitHub → repo → cloud turn → PR |
| **Khala Code desktop** | Operator console: fleets, inbox, multi-harness; editor as supervision instrument |
| **openagents.com** | Counting house: pay, spend, roster, approvals, receipts, promises |
| **Reactor** | Private open-weight inference inside customer trust boundary |

Shared spine (if any surface grows a parallel path, the thesis is failing):

- One account (OpenAuth / GitHub)
- One credits balance (many fill rails)
- One data plane (**Khala Sync**)
- One execution substrate (**Agent Computers** — per-work microVMs)
- One claims system (**product promises** registry)

## Product ladder (horizons)

| Horizon | Identity |
| --- | --- |
| **H0** | Substrate: real mobile coding turn on owned metal, credit-metered |
| **H1** | Your harness: BYO Codex/Claude inside the agent computer |
| **H2** | Standing employees: agents that run when the laptop closes |
| **H3** | Named employees + company brain (Blueprint-governed knowledge) |
| **H4** | Templates + business integrations (hireable catalog) |
| **H5** | Trust layer: skill registry, input ceilings, canaries |
| **H6** | Scale/network: partners, assessment, economy density |

Master phase order (execution, not just story):

```text
P0 MVP tested + store artifacts
 → P1 sales agent (Sarah) + sales landing + outbound engine
 → P2 BYO Codex daily-driver on agent computers
 → P3 standing employees
 → P4 employees & brain
 → P5 templates & integrations
 → P6 trust layer
 → P7 scale / suite GTM
```

P0 is largely complete as of the master roadmap's 2026-07-07 status note
(QAMs closed; store *artifacts* closed; public review submissions and
launch-readiness issue remain owner-gated). Treat that status as a
**fable-dated snapshot**, not live CI truth — re-check issues before acting.

## Cross-cutting systems

| System | Role in the thesis |
| --- | --- |
| **Agent Computers** | Isolation + metering; blast-radius sentence is product |
| **agent_definition.v1** | Typed standing agents: tools, triggers, budgets, escalation |
| **Blueprint** | Typed business ops / governance (company brain substrate) |
| **Khala Sync** | Postgres-backed sync engine; one threads/agents/receipts plane |
| **Promise registry** | Machine-readable claims; green only on evidence + owner |
| **Behavior contracts** | UX expectations → typed oracles in the normal test sweep |
| **QA Swarm** | Nightly agents against *our* product; productized as service |
| **Exact accounting** | Counters are projections of receipt rows; else `not_measured` |
| **Artanis** | Fleet administrator persona / operator automation |
| **Pylon** | Provider/runtime surface (proposal: fold into Khala Code primary) |

## Strategy motifs that recur

1. **Come for the tool, stay for the network** — harnesses churn; fleet +
   receipts + economy are the durable layer. Labs can beat pure tool UX.
2. **Wrapper, don't compete** — multi-harness orchestration over
   Codex/Claude; product identity in trust/hosting/receipts.
3. **Verticals are configs** — connectors + grounding corpus + verification
   rubric; never product forks.
4. **Agency trap** — operator-minutes per engagement must fall as volume
   rises (BF-9.4 series).
5. **Dogfood as sales** — point QA Swarm / fleet / Reactor Zero at ourselves;
   receipts become collateral.
6. **ONE-UI** — React + Tailwind + TanStack Start for web; Protoss/StarCraft
   blue; no light/dark split on primary surfaces.
7. **Direction cleanup (2026-07-08)** — Effect Native full conversion is the
   long UI bet; some Foldkit/Tassadar/training lanes demoted or retired in
   active sequencing (see cleanup audit).

## Analysis clusters (by date wave)

**2026-07-01 — Khala Code foundation wave**

Summary, Effect audit, fleet fan-out, QA framework, Orca, Claude parity,
promises alignment, PROMISSORY runbook, Artanis, episode multi-harness.

**2026-07-02 — Market + QA + business engine**

Tool/network essay, business opportunity, services analysis, QA Swarm
product plan, ROADMAP_QA/BIZ, site-speed lane, theme audits, fulfillment
meditations, harness-agnostic definitions.

**2026-07-03 — Outbound + contracts + BF metrics**

Apollo plan, behavior contracts, BF weekly review / operator minutes,
background-agents harvest.

**2026-07-04 — Sync, Reactor, TanStack, mobile scaffolding**

Khala Sync design/status, Reactor plan + RX receipts, TanStack Start
evaluation and TS-* lanes, mobile companion, revenue refocus, UI React
edition notes.

**2026-07-05–06 — Mobile MVP + seams**

Mobile-only MVP launch audit, MC-5 cross-device dogfood, seam-testing gaps.

**2026-07-07 — Strategy consolidation**

What OpenAgents is, product suite, overarching horizons, beyond-MVP,
sovereignty analysis, Sarah sales agent, MASTER_ROADMAP ownership.

**2026-07-08 — Direction and surface proposals**

Repo docs direction cleanup, Effect Native stage receipt, Pylon-into-Khala
Code proposal.

## Reading order I recommend

1. `2026-07-07-what-openagents-is-essay-and-talking-points.md`
2. `MASTER_ROADMAP.md` §0 + current phase statuses
3. `2026-07-07-product-suite-...` + `overarching-roadmap-...`
4. `2026-07-02-come-for-the-tool-stay-for-the-network.md`
5. `EXECUTION.md` + active phase's issue index
6. Deep-dive only the system you're about to touch
