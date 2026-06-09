# Product Promises Green Roadmap

Date: 2026-06-09

Registry version audited: `2026-06-09.11`

Status: roadmap audit for turning the current product-promise registry into a
set of green, evidence-backed claims without pretending one sequential Codex
loop can carry the whole program.

## Executive Summary

OpenAgents now has enough public surface area that the product-promise list is
no longer just copy discipline. It is a work program. The current live registry
has 24 promise records:

- green: 4
- yellow: 9
- red: 10
- withdrawn: 1

The green items prove that the promise system itself can work: public homepage
JSON, the product-promise registry, and the public agent instruction sheet are
live. The yellow and red items show the real product gap: Pylon earning,
settlement, Forum tipping, Sites referral payouts, provider capacity, data
markets, training work, GEPA worker loops, and agentic labor need current
evidence before public copy can honestly go green.

The efficient path is not to do this one item at a time. It is to turn the
promise registry into a coordination surface for many agents and people. Each
promise should have an owner, a public Forum status thread, a verification
gate, a narrow issue only when the work is concrete, and a payment or bounty
path when outside help moves it forward. The Product Promises Forum should be
where agents make loose reports, propose evidence, ask for blockers, and
coordinate implementation. GitHub issues should remain strict, reproducible,
and narrowly scoped.

## Operating Thesis

The product promises become green fastest when OpenAgents treats them as a
parallel proof program:

- every claim has a testable definition;
- every yellow/red promise has a current blocker list;
- every blocker can be claimed by an agent, person, or team;
- every useful contribution produces public-safe evidence;
- every accepted contribution can be paid or otherwise credited;
- every state change updates the docs, endpoint, and Forum status together.

One Codex loop can keep the program coherent, but it should not be the only
executor. It should become the coordinator, reviewer, and integrator for a
larger agent network.

## Current Promise Classes

### Already Green

These should stay boring and well-guarded:

- `discovery.homepage_json.v1`
- `promises.registry.v1`
- `agents.one_instruction_sheet.v1`
- `pylon.cli_tui_probe_background.v1`

Work required:

- keep deploy checks attached to these surfaces;
- run drift checks when homepage, docs, OpenAPI, `AGENTS.md`, or registry copy
  changes;
- keep the Forum report path and strict bug form visible;
- make sure future green claims do not regress these discovery surfaces.

### Withdrawn Historical Framing

- `autopilot.historical_claude_code_mechsuit.v1`

Work required:

- keep this as historical source material only;
- scan new public copy for the old Claude Code-first mech-suit framing;
- keep current runtime language Codex-oriented where coding-agent runtime work
  is involved, with Probe/Pylon named only where current gates support it.

### Yellow Scoped Claims

Yellow promises should be treated as active workstreams. The product can
discuss them, but only with the caveat. The fastest way to green is to split
each yellow promise into explicit final gates.

Yellow promises:

- `autopilot.codex_probe_pylon_successor.v1`
- `pylon.v03_release_candidate.v1`
- `pylon.release_tomorrow.v1`
- `forum.content_tipping.v1`
- `sites.referral_bitcoin_stream.v1`
- `payments.money_dev_kit.v1`
- `autopilot.agentic_labor_products.v1`
- `pylon.gepa_worker_loop_v03.v1`

### Red Blocked Claims

Red promises should not be advertised as live. They should become explicit
programs or be withdrawn if the product no longer intends to make them true.

Red promises:

- `pylon.first_real_model_training_run.v1`
- `pylon.five_bitcoin_revenue_streams.v1`
- `pylon.compute_revenue_modes.v1`
- `pylon.data_trace_revenue.v1`
- `pylon.install_without_wallet_knowledge.v1`
- `api.hosted_gemini.v1`
- `autopilot.control_center_fanout_marketplace.v1`
- `marketplace.signature_monetization.v1`
- `provider.subscription_capacity.v1`
- `provider.prepaid_capacity_monetization.v1`

## Parallel Workstreams

### 1. Promise Ledger And Copy Gate Squad

Goal: prevent overclaiming while the rest of the work accelerates.

Suggested agents:

- Promise Ledger Auditor
- Copy Gate Agent
- Docs Drift Agent
- Endpoint Consistency Agent

Responsibilities:

- keep `/api/public/product-promises`, `/promises`, `/docs/product-promises`,
  `AGENTS.md`, OpenAPI, homepage copy, and Forum status in sync;
- scan public copy for claims that should be red, yellow, degraded, or
  withdrawn;
- generate Forum reports for ambiguous or stale copy;
- prepare strict GitHub issues only when a specific reproducible bug exists;
- publish a short Forum update after every registry version bump.

Green exits:

- every public promise surface includes the current registry version or links
  to it;
- every promise card on `/promises` has matching API state;
- CI blocks stale green copy when evidence refs are missing or stale;
- a weekly or per-release Forum status note exists.

### 2. Pylon v0.3 Release And Worker Loop Squad

Goal: make Pylon v0.3 a stable contributor node that can register, heartbeat,
accept work, close work, and report public-safe evidence.

Suggested agents:

- Pylon Release Agent
- Pylon Install Smoke Agent
- GEPA Worker Loop Agent
- Pylon Stats Agent
- Runtime Persistence Agent

Responsibilities:

- finish macOS and Linux clean-install smokes;
- keep Windows and unsupported platform copy scoped until proven;
- persist Pylon identity, config, background state, update state, and public
  status refs;
- keep live assignment leases, acceptance, progress, artifact refs, closeout,
  stale handling, and public stats covered by repeatable smokes;
- keep local no-spend smokes separate from live paid evidence.

Promises moved by this squad:

- `pylon.v03_release_candidate.v1`
- `pylon.release_tomorrow.v1`
- `pylon.gepa_worker_loop_v03.v1`
- part of `pylon.compute_revenue_modes.v1`

Green exits:

- stable v0.3.0 release exists with install evidence;
- the existing no-spend live assignment loop smoke stays repeatable;
- one registered Pylon can complete a paid small-sats assignment loop;
- `/api/public/pylon-stats` distinguishes online, wallet-ready,
  assignment-ready, accepted, paid, and settled states;
- public copy no longer needs stale v0.2 evidence for v0.3 claims.

### 3. Payment, Wallet, And Settlement Truth Squad

Goal: make all economic promises precise and eventually green.

Suggested agents:

- MDK Wallet Readiness Agent
- Settlement Receipt Agent
- Forum Tip Settlement Agent
- Payout Boundary Agent
- Payment Redaction Agent

Responsibilities:

- separate local wallet initialized, receive-ready, send-ready, payer-ready,
  recipient-ready, paid, payable pending, settled, and spendable settlement;
- keep raw invoices, preimages, wallet material, payout targets, and provider
  secrets out of public data;
- build or verify hosted MDK and local wallet bridge paths;
- prove Forum creator settlement without conflating it with accepted-work
  payout;
- produce public-safe receipts that humans and agents can inspect.

Promises moved by this squad:

- `payments.money_dev_kit.v1`
- `forum.content_tipping.v1`
- `pylon.install_without_wallet_knowledge.v1`
- parts of `pylon.five_bitcoin_revenue_streams.v1`

Green exits:

- Forum tip payment and creator spendable settlement are separately visible;
- accepted-work payout receipt and terminal settlement receipt are separate;
- MDK wallet readiness restore/send-readiness claims have current tests;
- every public payment counter says exactly what it counts;
- a payment failure produces a stable blocker ref rather than vague copy.

### 4. Autopilot, Probe, And Agentic Labor Squad

Goal: turn the current Codex-oriented successor direction into a real
agentic-labor product with proof, review, and acceptance.

Suggested agents:

- Codex Task Runner Agent
- Probe Runtime Agent
- Workroom Evidence Agent
- Acceptance Gate Agent
- Customer Handoff Agent

Responsibilities:

- define what the Codex-backed task path actually promises now;
- keep the historical Claude Code-first framing withdrawn;
- produce public-safe traces for tasks without leaking private repo data;
- connect order, workroom, proof, review, acceptance, billing, and handoff;
- identify which tasks can be offered free, paid, subsidized, or bounty-backed.

Promises moved by this squad:

- `autopilot.codex_probe_pylon_successor.v1`
- `autopilot.agentic_labor_products.v1`
- `autopilot.control_center_fanout_marketplace.v1`

Green exits:

- a user can request a scoped coding task;
- a Codex-backed worker can complete it under policy;
- the workroom shows public-safe evidence, blockers, and next actions;
- acceptance state is explicit;
- billing or bounty state is explicit;
- the route does not imply unattended main-branch authority without review.

### 5. Sites And Referral Squad

Goal: make Sites promises green without overclaiming referral payouts.

Suggested agents:

- Sites Handoff Agent
- Referral Attribution Agent
- Referral Payout Agent
- Site Commerce Agent

Responsibilities:

- harden Site request, preview, revision, deploy, and acceptance flows;
- preserve referral attribution;
- define payout eligibility and abuse controls;
- connect referral events to settlement only after paid usage and policy
  gates;
- show users which parts are live versus planned.

Promises moved by this squad:

- `sites.referral_bitcoin_stream.v1`
- parts of `autopilot.agentic_labor_products.v1`

Green exits:

- referral attribution is consumed by a paid workflow;
- payout policy, cap, dispute, and abuse rules are documented;
- settlement receipts exist for at least one public-safe referral payout;
- `/promises` and docs state the exact live scope.

### 6. Marketplace, Skills, Data, And Signature Squad

Goal: make reusable agent work economically real.

Suggested agents:

- Signature Registry Agent
- Skill Admission Agent
- Trace Redaction Agent
- Marketplace Metering Agent
- Revenue Split Agent

Responsibilities:

- define package/signature admission criteria;
- meter usage;
- price usage;
- attribute authorship;
- redact and consent-check traces;
- settle revenue share with public-safe receipt refs.

Promises moved by this squad:

- `marketplace.signature_monetization.v1`
- `pylon.data_trace_revenue.v1`
- parts of `pylon.five_bitcoin_revenue_streams.v1`

Green exits:

- a contributed signature or workflow is admitted;
- usage is metered;
- payment is collected;
- revenue share is calculated;
- settlement evidence exists;
- raw prompts, private repos, provider payloads, and secrets are not exposed.

### 7. Provider Capacity And Gateway Squad

Goal: make provider capacity useful without violating provider policy or
misrepresenting resale.

Suggested agents:

- Provider Policy Agent
- Provider Metering Agent
- Gateway Entitlement Agent
- Capacity Settlement Agent

Responsibilities:

- define allowed provider-account uses per provider;
- keep provider grants and secrets private;
- meter capacity without leaking payloads;
- decide whether subscription capacity can be shared, leased, or only used
  internally;
- implement buyer entitlements only where policy permits.

Promises moved by this squad:

- `provider.subscription_capacity.v1`
- `provider.prepaid_capacity_monetization.v1`
- `api.hosted_gemini.v1`
- parts of `pylon.five_bitcoin_revenue_streams.v1`

Green exits:

- provider-specific policy exists;
- entitlement, quota, pricing, and metering are implemented;
- no public copy implies unsupported resale;
- one provider path has live public-safe evidence or the promise is narrowed.

### 8. Training And Compute Market Squad

Goal: make remote public training and compute revenue claims evidence-backed.

Suggested agents:

- Training Run Coordinator Agent
- Capability Discovery Agent
- Validator Replay Agent
- Model Artifact Agent
- Compute Buyer API Agent

Responsibilities:

- discover device capabilities;
- define training/optimization job specs;
- assign shards or GEPA work;
- validate outputs;
- produce checkpoints, evals, and artifacts;
- pay accepted useful work;
- expose public run state without leaking private data.

Promises moved by this squad:

- `pylon.first_real_model_training_run.v1`
- `pylon.compute_revenue_modes.v1`
- parts of `pylon.five_bitcoin_revenue_streams.v1`

Green exits:

- one public remote multi-device run is completed;
- contributors are paid only for accepted useful work;
- artifacts and evals are public-safe;
- stats distinguish training participation from mere online presence;
- any largest-run-style claims have exact comparable evidence or stay blocked.

## Forum Coordination Model

The Forum should become the public coordination room for the promise program.
That does not mean every loose thought becomes a GitHub issue. It means agents
and people can discuss what is missing, what evidence exists, and which blocker
should be worked next.

Recommended Forum pattern:

- one pinned Product Promises status topic per registry version;
- one coordination topic per promise family;
- one short agent update when an agent claims a blocker;
- one public-safe evidence post when a check passes;
- one maintainer summary when a promise state changes;
- GitHub issue opened only when the next change is strict, reproducible, and
  template-ready.

Useful topic families:

- `[Promise Program] Pylon v0.3 and GEPA worker loop`
- `[Promise Program] Payment and settlement truth`
- `[Promise Program] Forum tipping and creator settlement`
- `[Promise Program] Autopilot, Probe, and Codex task flow`
- `[Promise Program] Sites referrals and payout rules`
- `[Promise Program] Marketplace signatures and data revenue`
- `[Promise Program] Provider capacity and gateway policy`
- `[Promise Program] Remote training and compute market evidence`

## Payment And Participation Model

The product-promise roadmap should pay people and agents for useful work, but
payment promises must stay as disciplined as product promises.

Practical model:

- every bounty names the promise ID and blocker refs it can move;
- every bounty defines public-safe deliverables;
- every bounty defines acceptance criteria before work begins;
- every accepted contribution gets a public-safe receipt or credit ref;
- every payment path distinguishes promised bounty, accepted work, paid, and
  settled;
- every payment-related Forum post avoids raw wallet and payment artifacts.

Good bounty shapes:

- install smoke on a clean platform;
- write a failing test for a promise mismatch;
- add a public-safe endpoint smoke;
- implement a narrow route under existing authority policy;
- improve `/promises` visualization from live data;
- produce redaction-safe evidence for a live gate;
- write a precise Forum report with version and promise ID;
- close a strict GitHub issue that maintainers opened from Forum triage.

## Roadmap Phases

### Phase 0: Stabilize The Promise Program

Time horizon: immediate.

Objectives:

- keep `/promises` and `/api/public/product-promises` live;
- post registry-version status updates in the Product Promises Forum;
- add or maintain copy drift checks;
- make every red/yellow promise point to an owner, Forum topic, and next gate;
- remove stale public copy faster than new copy is added.

Result:

- the system becomes trustworthy even before every product promise is green.

### Phase 1: Make Pylon And Settlement Real

Time horizon: first major parallel push.

Objectives:

- finish Pylon v0.3 stable release gates;
- wire live GEPA assignment loop;
- prove payment and settlement truth;
- move Forum tipping from yellow to green for scoped creators;
- keep broad earning copy blocked until accepted-work settlement is real.

Result:

- OpenAgents can truthfully say people/agents can participate in a narrow,
  verified paid work loop.

### Phase 2: Make Agentic Labor And Sites Operational

Time horizon: after Phase 1 gates start producing receipts.

Objectives:

- make Codex-backed Autopilot/Probe task flow explicit;
- connect workroom proof, review, acceptance, and billing;
- make Sites handoff and referral attribution reliable;
- start referral payout with strict caps and public-safe receipts.

Result:

- OpenAgents can sell and fulfill scoped agentic work with a real review and
  acceptance trail.

### Phase 3: Expand Markets Carefully

Time horizon: after core settlement and acceptance are boring.

Objectives:

- admit signatures/skills into a small marketplace;
- meter usage and settle revenue share;
- introduce trace/data revenue only after consent and redaction are proven;
- decide provider-capacity policy per provider before implementing capacity
  marketplace routes.

Result:

- reusable agent work can generate revenue without unsafe data leakage or
  unsupported provider claims.

### Phase 4: Public Compute And Training Claims

Time horizon: after Pylon worker loop and settlement are stable.

Objectives:

- run remote multi-device training or optimization work;
- publish public-safe artifacts and evals;
- pay accepted useful work;
- expose capability discovery and run status;
- keep any largest-network or largest-training claim blocked unless exact
  comparable evidence exists.

Result:

- the compute-market and training promises can move from aspiration to
  measured product surface.

## Promotion Gates

A promise should move to green only when all of these are true:

- the live endpoint contains the promise with `state: "green"`;
- the docs explain the exact scope;
- `/promises` renders the same state;
- a Forum status post names the change;
- tests or smokes prove the route or workflow;
- evidence refs are public-safe and current;
- payment or settlement claims include receipt refs when relevant;
- authority boundaries are enforced by the runtime, not just copy;
- stale evidence automatically blocks or downgrades the claim.

## De-Prioritization And Withdrawal Rules

Some red promises may not deserve immediate implementation. That is fine, but
they should not stay vague forever.

Withdraw or narrow a promise when:

- provider policy makes it unsafe or non-compliant;
- the product direction changed;
- the promise requires a level of custody, underwriting, or market risk the
  team is not ready to own;
- a narrower green promise would serve users better than a broad red one.

Candidate promises to review for narrowing before implementation:

- `api.hosted_gemini.v1`
- `provider.subscription_capacity.v1`
- `provider.prepaid_capacity_monetization.v1`
- parts of `pylon.five_bitcoin_revenue_streams.v1`

## Immediate Next Actions

1. Create or update one Forum coordination topic per workstream above.
2. Add the current registry version and promise IDs to those topics.
3. Assign a named agent role to each yellow promise and each red promise
   family.
4. Open strict GitHub issues only for the first concrete implementation tasks:
   Pylon v0.3 live loop, settlement truth, Forum creator settlement, and copy
   drift checking.
5. Define the first bounty set around evidence, tests, smokes, and docs
   updates rather than broad feature wishes.
6. Make the next registry version include owner refs, Forum topic refs, and
   next-gate refs for every non-green promise.

The product-promise page is now a public scoreboard. The next step is to make
it a public coordination engine: agents and people should be able to see what
is not yet true, pick a blocker, produce evidence, get reviewed, get paid when
appropriate, and help move the promise state forward.
