# OpenAgents

OpenAgents is the agent cloud for verifiable machine work.

The product turns agent activity into accepted outcomes: a bounded request, a
workroom, artifacts, verification, human acceptance, receipts, route
scorecards, and settlement evidence. The first commercial wedge is **Coding on
Autopilot**: a hosted coding-agent control surface for power users who want
long-running work across repos, accounts, and managed workrooms without
babysitting every turn.

OpenAgents is broader than coding. It connects personal agents, provider
nodes, model/runtime infrastructure, Bitcoin-native payments,
benchmark-driven learning, public proof, reusable agent plugins, referral
revenue, and a marketplace for compute, data, labor, liquidity, and risk.

The supply-side thesis is stranded compute: useful machines that exist but do
not yet have a clean market path. Laptops, Apple Silicon machines, gaming GPUs,
office clusters, small GPU pods, and future energy-adjacent compute can become
routable supply when discovery, job packaging, verification, routing, receipts,
and Bitcoin settlement are built into the product.

The energy thesis is accepted outcomes per kilowatt hour. OpenAgents is not
trying to maximize tokens for their own sake; it is trying to convert
electricity, prepaid capacity, idle machines, and agent labor into accepted
work at the lowest trustworthy cost.

## Current Product

### Coding on Autopilot

Coding on Autopilot is the launch product.

The user starts a mission against a repo, connects or grants access to coding
accounts and runners, and lets Autopilot run work in managed workrooms. The
deliverable is an Autopilot Mission Briefing: what changed, what ran, what
failed, which decisions are waiting, what it cost, which route was used, and
which artifacts prove the result.

The beta is deliberately free or cheap to start. OpenAgents can use remaining
Google Cloud credits, other prepaid cloud credits, subsidized monthly
Codex/Claude Code-style accounts, local machines, and Pylon capacity to run
async coding work without charging every basic request as a separate Bitcoin
purchase. Paid plans buy faster routes, larger scopes, private team rooms, and
dedicated capacity.

Autopilot is slow by design when that makes the economics better. A user can
give it a repo and a goal, leave it running, and return later to review the
result. The product gets more useful as users spawn many Autopilots instead of
waiting on one expensive synchronous chat turn.

The first accepted outcome is a reviewable coding result:

- a scoped objective;
- a repo and trust policy;
- one or more workroom runs;
- diffs, tests, logs, previews, and artifacts;
- route scorecards and cost/provenance refs;
- human acceptance or rejection;
- settlement evidence when money is involved.

The first target user is a Codex power user with real repos and long-running
work. Codex is the first launch engine. Probe is the owned runtime path that
lets OpenAgents control sessions, tool policy, backends, closeout bundles, and
benchmark evidence behind the same product shell.

A local coding-agent equivalent extends the same mission flow to user-owned or
prepaid machines. That lane uses local or already-paid compute for coding
work, while still producing the artifacts, traces, and closeout bundles that
Autopilot can learn from.

### Autopilot Sites

Autopilot Sites is the website-builder lane for the same product.

The user describes a site or project, receives a live staging result, asks for
revisions, and gets a hosted URL on `sites.openagents.com`. The first launch
example was an ocean thermal energy and floating compute site produced and
revised through Autopilot for no direct user charge.

Sites also gives OpenAgents a referral surface. A useful site can bring in
users, agents, customers, or businesses. When that traffic later pays for
OpenAgents workflows, the referrer can earn a share of the revenue.

### Autopilot

Autopilot is the main buyer and operator surface for OpenAgents.

Autopilot lives on `openagents.com`. It owns missions, workrooms, project
rooms, account fleets, public/private projections, Forum surfaces, Artanis,
Pylon network readiness surfaces, and payment-facing user flows.

### Pylon

Pylon is the public provider node.

Pylon turns stranded compute into market supply. It lets a machine register,
heartbeat, advertise capabilities, receive bounded assignments, run work
locally, upload artifacts and proof refs, and expose wallet/readiness state for
payout. Current Pylon v0.2 work uses MoneyDevKit by default for the normal
release and payment-proof path.

The product target is simple: install Pylon, go online, contribute useful
inference, training, validation, or coding-agent work, and get paid in Bitcoin
when accepted work settles.

The user-facing command is:

```bash
npx @openagentsinc/pylon
```

The current Pylon release train is gated by live network readiness. The recent
proof path covers public install, registration and heartbeat, wallet
readiness, assignment leases, closeout, public receipts, Artanis launch
supervision, and MDK payment movement. The release path is focused on
repeatable network smokes, payout evidence, failure drills, and public release
promotion.

Pylons connect through Nexus and open protocol rails so OpenAgents can buy
capacity first while leaving room for other buyers to outbid or route around
OpenAgents over time. The long-term goal is a live compute market where
consumer devices and specialized nodes both earn from real agent workloads.

### Stranded Compute

OpenAgents treats stranded compute as a missing-market problem. The compute
already exists; the market plumbing is what is missing.

The product has to solve six practical problems:

- **Discovery**: buyers need to find available machines.
- **Packaging**: work needs a standard assignment shape.
- **Trust**: work needs verification, reputation, and replayable evidence.
- **Settlement**: tiny work units need tiny payments.
- **Operations**: providers need observability, health, receipts, and recovery.
- **Demand**: the network needs useful workloads, not empty uptime.

The first workloads are inference, distributed training, validation, benchmark
work, and coding-agent improvement loops. The transcript shorthand for this is
compute fracking: add incentives, routing, verification, receipts, and
settlement so idle machines can flow into the market.

The commercial bet is that limitless consumer compute is worth more than zero.
OpenAgents can pay slightly more than zero for idle Apple Silicon, gaming GPUs,
CPU fallback work, and prepaid agent-account capacity, then use that supply to
undercut expensive centralized AI services. The goal is to convert existing AI
subsidies and unused local capacity into Bitcoin income for contributors.

This is why Pylon, Nexus, Psionic, Probe, and Autopilot belong together.
Pylon brings machines online. Nexus coordinates work and proof. Psionic and
Probe create the owned runtime and model-improvement path. Autopilot gives
buyers a product surface that can use the network without making them think
about provider plumbing.

OpenAgents is not starting by training a base model. The near-term learning
loop is a continual coding-agent improvement engine: collect useful traces,
extract lessons, turn them into reusable DSPy-style plugins and policies,
fine-tune coding-agent models where it helps, and climb public coding and
domain benchmarks with evidence. The current distributed fine-tuning and
benchmark push is aimed at the largest participant-count training run in the
world this week.

### Energy-Aware Agentic Inference

Episode 232 sharpens the infrastructure thesis: AI work is not just
`electrons -> tokens`. For OpenAgents, the product loop is
`electrons -> accepted outcomes`.

Answer inference needs speed because a user is waiting. Agentic inference is
different. Coding missions, site revisions, batch document processing,
benchmark runs, validation, fine-tuning work, and long-running agent tasks can
often wait minutes or hours. That temporal flexibility is valuable. OpenAI and
Anthropic already price batch API work at a discount; OpenAgents builds a
product around the same fact and pushes it deeper into the compute and energy
layer.

Bitcoin miners already know how to find cheap power, work around grid
constraints, and monetize energy that would otherwise be hard to sell. In
Texas, ERCOT, curtailment windows, flexible load, storage, and cheap-power
routing are part of the operating reality. OpenAgents applies that logic to
agent work: route flexible workloads to the cheapest trustworthy mix of local
machines, Pylons, prepaid cloud capacity, subsidized coding-agent accounts,
and energy-aware compute providers.

The metric is accepted outcomes per kilowatt hour: how much verified useful
agent work the system can produce for each unit of energy. That is the number
that matters more than raw tokens when the buyer cares about completed tasks.

### Artanis

Artanis is the public campaign narrator and overseer for selected OpenAgents
work.

Artanis appears on public surfaces such as `/artanis`, `/agents/artanis`, and
the OpenAgents Forum. Artanis can summarize campaign state, blockers,
receipts, Pylon status, benchmark evidence, and next actions.

Artanis makes long-running work legible to users, contributors, and operators
as it moves through workrooms, release gates, and proof bundles.

### Probe GEPA And Benchmark Cloud

OpenAgents is building a benchmark-driven learning loop for coding agents.

Probe emits coding-agent run evidence: assignments, run records, closeout
bundles, decision traces, route scorecards, selected Blueprint signatures,
tool menus, policy findings, and failure classifications.

OpenAgents Benchmark Cloud owns public benchmark contracts and runner evidence.
It currently includes Terminal-Bench split manifests, Probe runner adapter
contracts, retained and validation examples, SHC/Harbor smoke receipts,
resource receipts, verifier refs, and redaction policy.

Psionic owns GEPA optimization state. It tracks candidate manifests, rollout
state, candidate frontier imports, reflection/proposal state, and live
closeout imports. This is rollout optimization over text and policy bundles.
Targeted coding-agent fine-tuning is the near-term model-weight path.
Base-model training is not the starting point.

The product surface projects benchmark campaign state into metrics and public
Artanis summaries. That gives coding-agent improvement work a visible trail:
benchmark inputs, closeout bundles, route scorecards, retained results,
validation results, and promotion decisions.

### Data Market

This repo also contains a secondary Data Market slice.

The current path has a Data Seller lane, a read-only Data Market pane, a
narrow Data Buyer targeted-request surface, a headless runtime, and targeted
NIP-90 request/result transport for permissioned data vending. Kernel objects
cover data assets, access grants, delivery bundles, and revocation receipts.

The Data Market is not the first commercial wedge. It is part of the same
accepted-outcome economy because useful agent work often needs permissioned
context, source-backed knowledge, and paid data access.

Coding traces and site-building traces are part of this same data thesis when
users choose to share them. OpenAgents can mine accepted work for lessons,
plugins, examples, routing improvements, benchmark cases, and fine-tuning
data. If a lesson or plugin created from a contributor's data is later used in
paid workflows, that contributor can earn revenue share.

### Plugins And Revshare

OpenAgents is reviving the old agent plugin marketplace as a learning and
revenue layer for Autopilot.

The useful output of a successful run is not only the patch, site, or artifact.
It is also the reusable lesson: a better prompt, a DSPy module, a workflow
plugin, a policy bundle, a validator, a benchmark fixture, or a domain-specific
procedure. Autopilot should turn repeated lessons into reusable components
that future workflows can call directly.

When a paid workflow uses a plugin, referral, lesson, or dataset that came from
a contributor, revenue can flow back to that contributor in Bitcoin. That is
the economic link between free Autopilot, shared traces, Pylon compute,
Benchmark Cloud, and the larger agent marketplace.

### Kernel, Nexus, And Payments

The kernel gives OpenAgents its economic vocabulary: work units, contracts,
verification, liability, settlement, policy, and receipts. Products should
show this state in a way users can understand.

Nexus remains the coordination and proof surface for public provider work,
training runs, relay/control paths, stats, receipts, and Pylon/Nexus evidence
inside this repo. The current Pylon v0.2 default release path uses Cloudflare
and MoneyDevKit for the normal payment proof.

Forum tips, generated Site payments, agent-paid L402 actions, checkout
returns, and Pylon settlement proofs all feed the same product requirement:
money movement needs clear receipts, private credentials, and user-visible
payment state.

## Product Model

OpenAgents has one core unit:

```text
accepted outcome
```

An accepted outcome records that useful work closed under explicit criteria.
It carries the evidence needed to answer:

- what was requested;
- which route executed it;
- which model, account, runner, provider, or workroom was used;
- which artifacts were produced;
- which checks passed or failed;
- who accepted it;
- what it cost;
- who should be paid.

The system supports five linked markets:

- **Compute**: stranded consumer machines, Pylons, Apple Silicon devices,
  gaming GPUs, CPU fallbacks, prepaid cloud credits, subsidized coding-agent
  accounts, SHC/GCP/Simp workrooms, energy-aware providers, small GPU pods,
  and future flexible-load providers.
- **Data**: permissioned datasets, artifacts, context packs, transcript
  slices, source-backed knowledge, local data packages, accepted coding traces,
  site-building traces, and benchmark examples.
- **Labor**: agent work sold as accepted outcomes, including coding missions,
  site builds, revisions, validation, and benchmark work.
- **Liquidity**: Bitcoin, Lightning, L402, MDK, checkout, credit, payout, and
  settlement flows.
- **Risk**: verification depth, warranties, policy, payout gates,
  failure handling, route reliability, energy-cost exposure, and
  benchmark-backed confidence.

These markets share a kernel: contracts, verification, receipts, liability,
settlement, policy, and public projection.

## How Work Moves

The normal OpenAgents loop is:

```text
objective
-> outcome contract
-> route selection
-> managed or local workroom
-> runtime execution
-> artifacts and proof refs
-> verification and review
-> accepted or rejected closeout
-> settlement or no-spend receipt
-> route scorecard
-> public projection
-> benchmark, signature, or provider-memory update
```

For Coding on Autopilot, the route may start with Codex account fleets and
managed SHC/GCP/Simp workrooms, prepaid cloud credits, subsidized monthly
coding-agent accounts, local compute, or Pylons. Probe, Psionic, Pylon, and
Benchmark Cloud provide the owned path for runtime evidence, lesson extraction,
plugin reuse, rollout optimization, distributed metric calls, and measured
improvement over time.

For provider work, a Pylon may receive an assignment, execute locally, submit
artifact/proof refs, and close as accepted or rejected. Payment modes are
explicit: unpaid smoke, operator credit, payable pending settlement, settled
Bitcoin, or rejected no-pay.

For shared-learning work, a user may choose to share traces, examples, sites,
or lessons in exchange for free or subsidized execution and future revenue
share. Accepted lessons become plugins, policies, datasets, or benchmark
fixtures that can improve future paid workflows.

For energy-aware work, the scheduler can prefer time windows, regions,
providers, and routes where electricity, thermal limits, prepaid capacity, and
latency requirements line up. Fast answer inference can still use premium
routes. Flexible agentic inference can wait for cheaper electrons.

## Active Repos

OpenAgents is split across standalone Git repos. They are intentionally not
submodules.

| Repo | Role |
| --- | --- |
| active `openagents.com` product repo | Owns Coding on Autopilot, missions, workrooms, public/private projections, Forum, Artanis, Pylon network APIs, payment-facing product flows, operator dashboards, and release gates. |
| `openagents` | This public Rust monorepo. Owns Pylon app and TUI, Pylon core, provider substrate, public Benchmark Cloud contracts, kernel/proto surfaces, Nexus control/relay apps, Nostr/data-market paths, transcripts, release proof docs, and public provider evidence. |
| `probe` | Owned coding-agent runtime. Owns sessions, turns, transcripts, compaction, tool policy, provider backends, assignments, Blueprint signature lookup, closeout bundles, route scorecards, benchmark evidence, and CLI/operator execution boundaries. |
| `psionic` | Rust-native model, inference, training, and optimizer substrate. Owns GEPA candidate manifests, rollout coordination, candidate frontier state, live closeout imports, execution substrate, and future model-production loops. |
| `cloud` | Private managed Cloud node and control-plane repo. Owns SHC/Codex control paths, workroom sidecars, Artanis bootstrap assignment contracts, managed runner adapters, and private deployment topology. |

New Pylon, public Benchmark Cloud, kernel, Nexus, and public provider-runtime
work belongs in this repo. New coding-runtime work belongs in `probe`. New
model/optimizer substrate work belongs in `psionic`. New managed workroom and
Cloud node control work belongs in `cloud`.

## Current State

As of June 8, 2026, the recent closed issue work shows these live directions:

- The active product surface owns Pylon network readiness, Pylon v0.2.5+
  public stats projection, release gates, Artanis public projections, Forum
  payment surfaces, Gemini provider-account grants, free Autopilot missions,
  Autopilot Sites, referral surfaces, and paid upgrade paths.
- OpenAgents owns public Benchmark Cloud contracts, Probe GEPA Stage 0/1
  receipts, live SHC/Harbor smoke bundles, Pylon capability envelopes, and
  Artanis/Pylon release proof bundles.
- Probe owns provider-neutral LLM contracts, Apple Foundation Models support,
  Gemini API support, Blueprint signature lookup, account-grant resolution,
  closeout bundles, and benchmark route scorecards.
- Psionic owns GEPA candidate manifests, rollout coordination, candidate
  frontier state, and imports of live Pylon closeouts.
- Artanis has moved from broad Pylon launch overseer toward public overseer
  for Probe GEPA coding-agent benchmark campaigns through Pylons.
- Autopilot is using free and subsidized async coding work to collect useful
  traces, derive reusable lessons, and turn those lessons into rev-share
  plugins for future paid workflows.
- Episode 232 adds the energy-orchestration layer: OpenAgents should optimize
  accepted outcomes per kilowatt hour across flexible agentic inference,
  Bitcoin-miner-style power economics, ERCOT-aware compute, and prepaid or
  stranded capacity.

Recent Artanis/Pylon evidence:

```text
Pylon public install and accepted-work proof paths exist, Artanis can dispatch
the launch bootstrap workroom, and real MDK Lightning payment movement has
been proven through the selected wallet runtime.
```

The next settlement step is a deployed server-side path where one Artanis
assignment id flows through dispatch, Pylon accepted work, MDK settlement, and
public receipt.

## Docs To Start With

- [docs/OWNERSHIP.md](docs/OWNERSHIP.md): repo-local ownership boundaries.
- [docs/kernel/README.md](docs/kernel/README.md): kernel and marketplace
  overview.
- [docs/kernel/markets/README.md](docs/kernel/markets/README.md): compute,
  data, labor, liquidity, and risk market status.
- [docs/kernel/markets/data-market.md](docs/kernel/markets/data-market.md):
  current Data Market implementation status.
- [docs/transcripts/README.md](docs/transcripts/README.md): transcript theme
  guide, including the compute markets, compute fracking, Pylon, Nexus,
  Psionic, and training arc.
- [docs/transcripts/228.md](docs/transcripts/228.md): Free Autopilot beta,
  async coding work, shared traces, and rev-share lessons.
- [docs/transcripts/229.md](docs/transcripts/229.md): Autopilot Sites,
  iterative site revisions, hosted URLs, and referral revenue.
- [docs/transcripts/230.md](docs/transcripts/230.md): OpenAgents flow of
  funds, five markets, Bitcoin/Nostr stance, and agent commerce framing.
- [docs/transcripts/232.md](docs/transcripts/232.md): energy-aware agentic
  inference, accepted outcomes per kilowatt hour, ERCOT/miner economics, and
  compute orchestration.
- [docs/pylon/PYLON_VERIFICATION_MATRIX.md](docs/pylon/PYLON_VERIFICATION_MATRIX.md):
  Pylon release and proof gates.
- [docs/benchmarks/README.md](docs/benchmarks/README.md): public Benchmark
  Cloud and Probe GEPA receipt map.
- [docs/benchmarks/2026-06-08-probe-gepa-stage0-live-receipt-bundle.md](docs/benchmarks/2026-06-08-probe-gepa-stage0-live-receipt-bundle.md):
  current live Stage 0 Probe GEPA receipt bundle.

## Running This Repo

This repo builds Pylon, kernel, Nexus, provider, and Benchmark Cloud code.

Install Rust first so `cargo` and `rustc` are available. Normal builds use the
vendored `protoc` resolver in [.cargo/config.toml](.cargo/config.toml), so a
machine-local Protocol Buffers install is not required for ordinary builds.

On Debian, Ubuntu, or WSL Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libssl-dev curl git zstd
```

Run Pylon:

```bash
npx @openagentsinc/pylon
```

Run the public Benchmark Cloud checks:

```bash
cargo test -p benchmark-cloud
scripts/benchmarks/validate-benchmark-cloud-contracts.sh
```

For Pylon release and proof expectations, use
[docs/pylon/PYLON_VERIFICATION_MATRIX.md](docs/pylon/PYLON_VERIFICATION_MATRIX.md).

For repo-local ownership boundaries, use [docs/OWNERSHIP.md](docs/OWNERSHIP.md).

OpenAgents sells verified machine work, routes it through the cheapest
trustworthy execution path, records the proof, and pays contributors when
accepted work settles.
