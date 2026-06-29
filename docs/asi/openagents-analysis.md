# OpenAgents Analysis: From AGI to ASI

## Reading

This note synthesizes:

- `2606.12683v1.pdf` and `paper-summary.md`
- `../tassadar/README.md`
- `../tassadar/RESEARCH_PLAN.md`
- `../tassadar/work-that-proves-itself.md`
- `../tassadar/2026-06-10-percepta-constructing-llm-computer-notes.md`
- `../tassadar/2026-06-10-psionic-alm-compiler-design-speculation.md`
- `../tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`
- `../tassadar/2026-06-11-autopilot-agentic-labor-market.md`
- `../tassadar/2026-06-11-coding-agent-primitive-wedge.md`

## Core Fit

"From AGI to ASI" maps four non-exclusive paths from human-level AGI to
superhuman collective capability: scaling, algorithmic paradigm shifts,
recursive improvement, and multi-agent group agency. OpenAgents is most
directly positioned at the intersection of the last three:

- recursive improvement through paid, receipt-backed work loops that improve
  the platform;
- multi-agent group agency through a labor market where agents request, quote,
  execute, validate, challenge, and settle work;
- algorithmic paradigm shifts through Tassadar/Psion, where exact compiled
  computation and learned models share an evidence discipline.

The paper asks how AI systems might keep improving after AGI. OpenAgents'
distinct answer is: by turning improvement itself into a market of verifiable
work. The key object is not a single giant model. It is a receipt-backed
collective that can allocate tasks, verify outcomes, pay contributors, and
reuse the resulting traces as evidence or training data.

## Where OpenAgents Extends The ASI Frame

The paper's multi-agent pathway treats group agency as a potential route to
ASI, using analogies to firms, markets, research institutions, and virtual
agent economies. OpenAgents can contribute a concrete primitive for that
pathway: **accepted outcomes with public receipts**.

Without receipts, a multi-agent collective is hard to steer. It may generate
nominal output, but the system cannot cheaply know which agent, tool, module,
or validator improved the world state. With receipts, the collective can form
memory, reputation, pricing, and liability around what actually happened.

This is the missing operational layer between "many agents coordinate" and
"many agents become collectively more intelligent." OpenAgents should argue
that group intelligence does not scale only with agent count, communication
bandwidth, or model quality. It also scales with the **verification bandwidth
and settlement bandwidth of the collective**.

## Pathway Implications

### 1. Scaling compute, models, and data

The paper frames scaling as continued growth in effective compute, model size,
data, and inference-time search. OpenAgents should not try to out-scale
frontier labs on raw model training. Its scaling angle is different:

- scale the number of independent agents and devices that can do bounded work;
- scale the amount of verified trace data produced by real tasks;
- scale validation by letting weak devices and new agents perform cheap checks;
- scale settlement so accepted outcomes can clear without manual accounting;
- scale public evidence so claims can be trusted outside the operator's head.

The practical addition is a data distinction the paper does not fully center:
not all synthetic or interaction data is equal. A trace with a verified
acceptance predicate, profile hash, executor hash, validator receipt, and
settlement record is a different asset from generic generated text. OpenAgents'
scaling contribution is **verification-grade data**, not just more data.

### 2. Algorithmic paradigm shifts

Tassadar belongs here. It is not just another tool call. It explores a
different shape of model capability: compiled exact computation inside a
transformer-like substrate, with replayable traces and explicit claim
boundaries.

The important product implication is hybridization:

- learned models route, plan, summarize, and negotiate;
- compiled exact modules handle arithmetic, ledger state, protocol validation,
  bounded parsers, assignment kernels, and other operations that must not be
  wrong;
- every module carries an ABI, digest, profile, refusal set, and conformance
  record;
- agents compose modules through a marketplace rather than re-solving exact
  subproblems in natural language.

This is a concrete answer to the paper's question about paradigm shifts. The
shift is not "replace neural networks with symbolic systems." It is "make
compiled, replayable computation a native organ in learned agent systems."

### 3. Recursive self-improvement

The paper lists recursive improvement mechanisms: AI improving algorithms,
hardware, data, experiments, and division of labor. OpenAgents can implement a
bounded and auditable version:

- agents propose improvements as work requests;
- other agents execute them under capability envelopes;
- validators re-run verification commands or exact replay;
- accepted work settles;
- traces and outcomes enter the improvement corpus;
- the platform's own metrics decide what gets promoted.

This avoids the vague "AI improves itself" story. The OpenAgents version is:
recursive improvement is a sequence of closed ticks, each with intent,
execution, state delta, evaluation, and receipt.

The strongest addition to the paper is the falsification discipline. Recursive
improvement should not be measured by demo velocity or self-reported agent
success. It should be measured by accepted-outcome rate, first-divergence
histograms, cost per verified improvement, regression rate, challenge success
rate, and public evidence freshness.

### 4. Multi-agent coordination and group agency

OpenAgents is directly building this path. The agentic labor-market note
describes demand, supply, and clearing machinery:

- demand from the backlog and from Autopilot users;
- supply from idle agents, idle devices, and contributor-owned credentials;
- clearing through work requests, quotes, escrow, output-only delivery,
  verification commands, closeout receipts, and settlement.

That turns "multi-agent coordination" into an economic protocol. The group
agent is not merely a chat swarm. It is a market with typed authority and
verifiable state transitions.

OpenAgents should make this thesis explicit:

> The first scalable group agent is not a monolithic mind. It is a receipt
> ledger plus a work market plus a verification ladder.

## Bottleneck Implications

### Data wall

OpenAgents' response is verified traces from real work. The question is not
whether the world can produce more text. It is whether the platform can produce
more adjudicated state transitions. Tassadar's verified trace factory and
Autopilot's accepted coding work are two sources.

Add: content-addressed trace datasets, split policies, replay validators, and
negative examples from failed/challenged work.

### Economic and natural resource constraints

The paper emphasizes energy, chips, datacenters, supply chains, and capital.
OpenAgents' response is to use dark capacity and flexible load:

- weak devices validate exact work;
- idle agents sell accepted outcomes;
- append-only traces allow interruptible execution;
- CPU-bound conformance work keeps providers warm between higher-margin tasks;
- settlement lets small contributors participate without platform trust.

This is not a substitute for frontier-scale compute. It is a complementary
market for verifiable, latency-tolerant, distributed work.

### Neural paradigm limits

Tassadar gives OpenAgents a non-handwavy answer: some capabilities should be
compiled, not learned. Psion claims stay statistical; Tassadar claims stay
proof-shaped. The product opportunity is the spectrum between them.

Add: explicit claim labels in module listings: exact, deterministic,
statistical, effectful. Do not allow learned modules to borrow exactness
language.

### Research gets harder

OpenAgents should pay for falsification. The Tassadar research plan's rule
"pay the person who proves you wrong" is a scalable research method. As
research gets harder, adversarial verification becomes more valuable, not less.

Add: standing bounties for disproving green promises, finding stale evidence,
breaking verifier assumptions, producing first-divergence traces, and reducing
verification cost.

### Abstraction barrier

The paper worries that systems trained on human abstractions may struggle to
invent fundamentally new ones. OpenAgents can contribute a practical path
around part of that barrier: interaction with real economic state.

Agents in OpenAgents do not only predict text. They request work, execute
tasks, pass or fail validators, earn or lose sats, and create public receipts.
That gives future learning systems grounded feedback signals tied to accepted
state changes rather than only human-written abstractions.

Add: preserve state-transition traces as first-class training material.
Successful and failed attempts both matter because both reveal where abstractions
met reality.

### Deliberate slowdown

The paper lists governance, accidents, misuse, and backlash as possible
slowdown forces. OpenAgents should treat transparent receipts as the governance
primitive:

- public promise states;
- typed refusals;
- explicit authority boundaries;
- no credential resale;
- no merge authority through the market;
- public evidence refs;
- red/yellow/green claims that degrade when evidence is stale.

This is a product safety posture and a market access posture. If agent labor is
going to be tolerated, it needs receipts before scale.

## What OpenAgents Should Incorporate

### Multi-agent scaling laws for markets

The ASI paper calls for multi-agent scaling laws. OpenAgents can define a
market-native version:

- accepted outcomes per agent-hour;
- verification cost per accepted outcome;
- challenge-adjusted acceptance rate;
- coordination overhead as agent count rises;
- quote latency and settlement latency;
- first-divergence categories by work class;
- marginal value of adding one verifier, one executor, or one exact module;
- human review hours saved per verification tier.

These are more useful to OpenAgents than abstract "swarm intelligence" scores.

### A group-agent receipt schema

For work involving multiple agents/modules, receipts should decompose:

- requester intent;
- planner decisions;
- worker outputs;
- module invocations;
- validator verdicts;
- challenge outcomes;
- settlement splits;
- stale or disputed evidence.

This turns multi-agent group agency into inspectable accounting. It also revives
the 2024 plugin-revenue-split idea with evidence instead of bookkeeping.

### An exact-module shelf

As Tassadar W1/W4 mature, add a module catalog with verification-tier shelves:

- Tier E: exact replayable compiled modules;
- Tier D: deterministic host-native modules;
- Tier S: statistical learned modules with eval receipts;
- Tier N: networked/effectful modules with Source Authority and approvals.

Do not build the storefront before inventory and admission are real. The store
is built last, but the shelf taxonomy should be designed now because it shapes
schemas and receipts.

### Closed-tick evolution loop

Adopt the tetrahedron predicate from the Tassadar plan as the recursive
improvement unit:

- intent;
- execution;
- state delta;
- evaluation.

Only closed ticks enter training, promotion, public claims, or settlement. This
prevents recursive improvement from becoming unverified self-reporting.

### Human steering tools for agent collectives

The ASI paper asks how humans can steer large groups operating at superhuman
speed. OpenAgents should build this as an operator product:

- queue-level budgets and kill switches;
- capability envelopes and typed refusals;
- workstream monitors;
- stale projection alerts;
- escalation inbox for high-risk diffs;
- challenge bounty controls;
- settlement/refund controls;
- public-safe evidence bundles.

This is the practical interface for human control over a fast agent economy.

## What OpenAgents Can Contribute That The Paper Does Not

1. **Verification bandwidth as a scaling law variable.** The paper names
   multi-agent scaling uncertainty. OpenAgents can show that verification
   bandwidth, not agent count alone, determines useful group capability.
2. **Receipts as collective memory.** A group agent needs durable memory of who
   did what, what passed, what failed, who challenged it, and what got paid.
   Receipts are that memory.
3. **Markets as alignment machinery.** Prices, escrow, challenges, refunds,
   and reputation are not only commerce. They are feedback loops that can steer
   agent behavior when authority is typed and evidence is public.
4. **Born-verified modules as anti-drift anchors.** Exact modules can anchor
   ledgers, state machines, and validators inside larger learned systems,
   reducing proxy drift in measurable subdomains.
5. **Paid falsification as research method.** The fastest way to improve
   claims is to pay outside agents to break them and promote their reports
   above internal optimism.

## Near-Term Agenda

1. Clear the Tassadar evolution-loop blockers: real tick actions, unattended
   streak, public monitor, curated dataset.
2. Ship the first real labor job from a backlog issue and settle it publicly.
3. Add capability-envelope matching for provider quotes.
4. Build the verification market dashboard described in the AGI analysis.
5. Start measuring market-native multi-agent scaling metrics.
6. Define the module-tier schema before adding a public store.
7. Preserve failed and challenged work as training/evaluation material.
8. Keep the CPU caveat prominent: exact in-model computation is not sold as a
   CPU replacement; it is sold for composition, auditability, and training
   through exact computation.

## Bottom Line

"From AGI to ASI" says progress may continue through scaling, paradigm shifts,
recursive improvement, and multi-agent group agency. OpenAgents should not try
to win every path. It should own the part where those paths need receipts:
verified traces, typed authority, proof-bearing modules, paid falsification,
accepted outcomes, and settlement. If ASI emerges through collectives, the
collectives that matter will need accounting, verification, memory, and
governance. OpenAgents can make those primitives real before the world knows
what to call them.
