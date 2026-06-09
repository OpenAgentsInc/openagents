# Outcomes Per Kilowatt-Hour: The Missing Metric Of Bitcoin And AI

Date: 2026-06-06

Status: investor essay draft with Artanis substantiation addendum.

## Email Headline

```text
Outcomes per kilowatt-hour: the missing metric of bitcoin + AI
```

The headline is more defensible with the energy unit explicit.

Bitcoin mining made energy-native compute legible. A miner can look at a
machine, a power price, a hashprice, and an uptime assumption and ask:

```text
How many sats can this kWh earn?
```

AI still lacks the equivalent operating metric. The industry talks about
tokens, GPU-hours, model benchmarks, node counts, and data-center megawatts.
Those are useful internal inputs, but none of them answers the question an
operator, customer, or investor actually needs answered:

```text
How much verified useful work did this energy produce?
```

For OpenAgents, the answer is:

```text
accepted outcomes per kWh
accepted revenue per MWh
accepted gross profit per MWh
provider payable or settled value per kWh
```

The precise phrase is "outcomes per kilowatt-hour." "Outcomes per watt" remains
useful shorthand, but the diligence-grade unit is usually per kWh or per MWh,
because sites, miners, data centers, and power markets settle energy over time.

## Definition

Outcomes per kilowatt-hour is the energy-productivity ratio showing how a
model, agent, workroom, node, or capacity pool converts measured or explicitly
modeled electrical energy into accepted, receipt-backed outcomes.

The simplest numerator is:

```text
accepted outcome count
```

The stronger numerator is:

```text
accepted outcome value
```

The investor-grade numerator is:

```text
accepted gross profit
```

The provider-grade numerator is:

```text
provider payable value
```

The settlement-grade numerator is:

```text
provider settled value
```

The denominator must be measured or explicitly modeled energy:

```text
watt-hours -> kWh -> MWh
```

If the energy denominator is unknown, the metric should be null. A completed
outcome without measured or modeled energy is still useful, but it is not yet
an energy-productivity claim.

## Why Tokens Per Watt Is Not Enough

Tokens per watt measures raw model output against instantaneous power. Outcomes
per kilowatt-hour measures accepted useful work against energy over time.

An agentic workflow may involve model calls, tool calls, code execution,
browser work, file IO, retries, validation, grading, human review, and
settlement. A million tokens can produce nothing useful. A slow background
workroom can produce a valuable accepted artifact.

So the unit has to move from:

```text
correct answer
```

to:

```text
accepted artifact
completed workflow
verified business result
receipt-backed closeout
```

This is why OpenAgents should not lead with raw compute supply. The old cloud
sells machines. Model clouds sell intelligence. GPU clouds sell accelerators.
The Agent Cloud sells accepted outcomes, then routes the work to the cheapest
eligible path underneath.

## Why Bitcoin Makes The Metric Sharper

Bitcoin mining already prices computation in energy terms. It gives compatible
sites a floor:

```text
mining revenue per kWh
mining margin per kWh
mining breakeven power price
```

OpenAgents adds the upside:

```text
accepted outcome value per kWh
accepted gross profit per kWh
provider payable per kWh
```

The dispatch rule is then simple:

```text
Run accepted outcomes when risk-adjusted accepted-outcome value per kWh beats mining.
Mine when mining beats available accepted outcomes.
Curtail when grid or power-market value beats both.
Idle or reserve when none clears cost and risk.
```

That is the real bitcoin-plus-AI wedge. It is not "every miner becomes an AI
data center." It is:

```text
Mining gives the floor.
Accepted outcomes give the upside.
Routing decides which MWh should become which kind of value.
```

This is also why "mullet mining" has to be defined strictly. A site is not
full mullet mining just because it runs some AI near miners. In the full
mullet pattern, AI/HPC has front-of-house priority and mining absorbs residual
capacity. A mining-led AI pilot is useful, but it is a different claim.

## What Is Already Substantiated

OpenAgents product surface and the root mining/cloud docs already substantiate the measurement
shape.

### 1. Accepted Outcomes Are The Product Unit

The canonical Cloud source defines OpenAgents Cloud as a workroom-native
control plane for defining outcomes, running agents until artifacts meet those
outcomes, grading results, and recording receipts for every accepted outcome.
It explicitly says the durable unit is not GPU-hour, token, watt, hash, node
online, or agent effort. The durable unit is the accepted outcome.

That supports the essay's first claim:

```text
The AI side of the bitcoin + AI comparison should not be measured in tokens or
uptime alone. It should be measured in accepted outcomes.
```

### 2. Accepted Outcomes Per Energy Is Implemented As An OpenAgents product surface Contract

`docs/omni/2026-06-06-accepted-outcomes-per-power.md` records the current
OpenAgents product surface implementation of `OmniOutcomePowerProductivityRecord`.

The contract records:

- accepted outcome count;
- accepted revenue;
- accepted gross profit;
- provider payable value;
- provider settled value;
- energy in watt-hours;
- dark-capacity energy in watt-hours;
- modeled, measured, mixed, or unknown power-data state;
- payable, verified, settled, mixed, or not-settled provider state;
- accepted outcome refs;
- energy evidence refs;
- measured meter refs;
- settlement refs.

It derives:

- accepted outcomes per kWh;
- accepted outcomes per MWh;
- accepted revenue per kWh;
- accepted gross profit per kWh;
- provider payable per kWh;
- dark-capacity MWh.

This supports the essay's second claim:

```text
OpenAgents has a typed measurement contract for outcomes per energy, not only a
marketing phrase.
```

### 3. Investor Outcome Economics Is Implemented Separately

`docs/omni/2026-06-06-investor-grade-outcome-economics-metrics.md` keeps
accepted revenue, accepted gross profit, provider payable, provider settled
amounts, refunds, grading cost, review cost, retry cost, artifact cost, and
runner/provider costs separate.

That matters because "outcomes per kilowatt-hour" is not enough by itself.
Investors will ask:

```text
What was accepted?
What did the buyer pay?
What did it cost to verify?
What was payable to the provider?
What actually settled?
What failed, retried, or refunded?
```

The investor economics contract supports those questions without letting
modeled rows become accepted revenue or payable rows become settled payouts.

### 4. Margot Exports Put Mining, AI, Curtailment, And Power In Common Units

`docs/omni/2026-06-06-margot-export-ingestion-contract.md` supports
read-only packets for:

- mining floor per MWh;
- GPU rental floor per MWh;
- OpenAgents accepted-work assumption value per MWh;
- grid-service and curtailment value assumptions per MWh;
- power cost per MWh;
- market and dispatch-policy labels.

It also preserves a critical caveat: GPU chip TDP is not facility power. Real
facility energy includes the server, CPU, memory, storage, networking, fans,
cooling, PSU/PDU losses, and site overhead.

This supports the essay's third claim:

```text
The bitcoin + AI comparison should happen in per-MWh units with explicit
caveats, not in raw GPU-hour or nameplate-MW claims.
```

### 5. Margot's Latest Simulator Makes This A Product Requirement

I synced `projects/repos/oa_aibtc_model` to upstream
`dmrobotix/oa_aibtc_model` at `efccd28` on 2026-06-06. That repo now does more
than the earlier chip-TDP GPU rental floor.

It has three AI revenue lanes:

- GPU rental mode: `data/gpu_pricing.json` has 26 GPU models scraped from
  Vast.ai `/api/v0/bundles` on 2026-06-01, with minimum dollars per GPU-hour
  and rated chip TDP. The model computes `R_AI = rental_rate * 1e6 / watts`.
- Token inference mode: `data/model_revenue.json` has 29 priced models and 43
  model-task rows from OpenRouter pricing joined to ML.Energy benchmark rows.
  The intended unit is `completion price per token / J per token * 3.6e9`.
- Node config mode: `data/nodes.json` includes Ben's B300 SXM node with
  8 GPUs, 12,584 W system power, and 1,573 W effective power per GPU. That
  produces about `$4,315/MWh` rental-equivalent value, compared with about
  `$4,848/MWh` if the same B300 rental price is divided only by 1,400 W chip
  TDP.

That last delta is the product lesson. The proof product cannot stop at "we
tested a site." Artanis and Pylon need to produce a comparative economic packet
for every capacity slice:

```text
mining floor
vs GPU rental floor
vs token revenue floor
vs node/system-power-adjusted floor
vs accepted-outcome value
vs electricity, curtailment, idle, and dark-capacity alternatives
```

The current Margot repo also added `docs/token_speed_estimator.tex`, which
specifies the next missing lane: estimate token throughput from memory
bandwidth, quantization, context, interconnect, framework efficiency, active
parameters, VRAM fit, and break-even LMP. Ben's OCOLO calculator link belongs
in this lane as a reference source, not as authority by itself, until Artanis
captures the source, query params, timestamp, model, hardware, and caveats.

There is one immediate diligence issue Artanis should flag before any public
token claim: `build_model_revenue.py` computes from OpenRouter's raw dollars
per token, but some JSON field names and metadata text still use
`completion_usd_per_mtok` language. The economics can be right while the
labels are confusing. A proof packet needs an explicit unit audit:

```text
OpenRouter raw price unit
display price unit
ML.Energy J/token unit
derived $/MWh formula
sample row arithmetic
```

This supports a stronger product claim than a site-only pilot:

```text
OpenAgents is building a Pylon/Artanis analysis loop that can compare mining,
GPU rental, token inference, node power, and accepted outcomes in the same
energy-settled units.
```

But it is still a modeled loop until Pylon capacity, actual work, acceptance,
and energy evidence are joined.

### 6. The SHC Model Gives A Plausible Economic Curve, Not Final Proof

The root SHC compute-versus-mining model turns Dimi's sats-per-watt intuition
into revenue-per-kWh arithmetic. A 600 W refurbished server earning:

- `$200/month` implies about `$0.463/kWh`;
- `$600/month` implies about `$1.389/kWh`;
- `$960/month` implies about `$2.222/kWh`.

Against an implied `$0.023/kWh` mining/offtake floor, those are roughly
20x, 60x, and 96x gross revenue-per-kWh cases.

That supports the essay's fourth claim:

```text
Small compute islands can matter if accepted-outcome value per kWh materially
exceeds the mining/offtake floor.
```

But the model is not a public proof. It must be replaced by measured kWh,
accepted-outcome revenue, real mining floor, bandwidth cost, and provider
settlement evidence.

## What Is Not Yet Definitively Substantiated

The current support does not yet prove portfolio-wide or site-level outcomes
per kilowatt-hour. It proves that OpenAgents has the right measurement spine.

The still-missing proof is:

```text
measured energy
-> actual workroom execution
-> accepted outcome
-> accepted revenue/gross profit
-> provider payable
-> provider settlement
-> comparison against mining, GPU rental, token inference, node power,
   VPS/colo, curtailment, and idle alternatives
```

For investor email copy, we can say:

```text
OpenAgents is building around outcomes per kilowatt-hour.
```

For investor diligence, we need to show:

```text
Here are the MWh.
Here are the accepted outcomes those MWh produced.
Here is the mining/offtake, GPU-rental, token-inference, and node-power floor
for the same windows.
Here is the revenue, gross profit, provider payable, and settlement state.
Here is the dark capacity that did not produce accepted value.
```

## Artanis Addendum: What Artanis Can Support

Artanis is the proof steward for this story. He is not the watt meter, the
wallet, the provider runtime, or the settlement authority.

The Artanis issues split into two categories:

- closed issues #386-#404, which give Artanis typed contracts, public-safe
  projection, claim-state discipline, health/staleness, work-routing,
  readiness, marketplace, reward visibility, launch smoke, D1 persistence, and
  scheduled tick execution;
- open issues #405-#415, which still gate operator UI, Forum
  delivery/listening, Nexus/Pylon adapters, marketplace intake implementation,
  continual-learning templates, reward smoke, launch communications, the
  production launch gate, and Margot/Pylon comparative-economics evidence
  packets.

### 48-Hour Artanis Sprint For Margot And Pylon

The next 48 hours should not be a narrow "test some gear at a site" exercise.
It should be an Artanis-managed evidence sprint that turns Margot's simulator
into a Pylon product requirement.

This is now tracked as GitHub issue
[#415](https://github.com/OpenAgentsInc/openagents/issues/415),
`ARTANIS-029: Collect Margot/Pylon comparative economics evidence packets`.

The sprint target:

```text
For each Pylon capacity slice, show the mining floor, GPU rental floor, token
revenue floor, node/system-power correction, power-market window, and
accepted-outcome evidence state in one labeled packet.
```

Minimum data Artanis should collect:

1. Margot baseline snapshot.
   Record the `dmrobotix/oa_aibtc_model` commit, data timestamps, source URLs,
   and a normalized export of GPU rental, token inference, node-config, ERCOT,
   and NYISO assumptions. Public projection can name the repo and commit but
   must not expose private invite or collaborator material.
2. Unit audit.
   Check OpenRouter raw pricing units, display units, ML.Energy J/token units,
   and sample row arithmetic before any token `$ / MWh` claim leaves operator
   context.
3. Pylon capacity survey.
   For each node or proposed node: GPU model, GPU count, VRAM, interconnect,
   framework/runtime, system power, chip TDP, measured PDU/IPMI availability,
   cooling/PUE assumption, availability window, resource mode, and cost term.
4. Power-market window.
   Use ERCOT and NYISO only unless an unsupported-market caveat is attached.
   Capture zone, LMP window, refresh timestamp, and missing-data flags.
5. Mining counterfactual.
   Capture ASIC model, efficiency, capacity, pool/firmware/ops assumptions,
   mining revenue per MWh, mining margin per MWh, and curtailment policy for
   the same windows used by the AI comparison.
6. Token-throughput cross-check.
   Treat ML.Energy as measured benchmark evidence, OCOLO-style calculators as
   modeled reference evidence, and local Pylon benchmark runs as the strongest
   near-term measured evidence. Label each source separately.
7. Work/outcome sample.
   If Pylon work can run inside the window, collect assignment, runtime,
   artifact, grading, acceptance, rejection, retry, and closeout refs. If not,
   label the packet as economic-model-only and do not claim accepted outcomes.
8. Product gap list.
   Record which fields OpenAgents product surface already ingests through Margot export packets and
   which fields still need first-class Artanis/Pylon support.

What this can substantiate in 48 hours:

```text
OpenAgents can compare mining, GPU rental, token inference, and Pylon node
economics in common per-MWh units with explicit provenance and caveats.
```

What it cannot substantiate in 48 hours unless real accepted work also runs:

```text
OpenAgents has proven outcomes per kilowatt-hour for Pylon capacity.
```

The product implication is direct: Artanis needs a recurring comparative
economics loop, not a one-off spreadsheet. Pylon readiness packets should carry
the same data Margot needs to calculate the floor, and workroom acceptance
receipts should carry the numerator needed to convert the floor into outcomes
per kWh.

### Already Supported By Closed Artanis Work

Artanis can already support the outcomes-per-kWh essay in these ways.

#### 1. Claim-State Discipline

The standalone autonomy claim ledger covers:

- autonomous loop;
- operator steering;
- Forum communication;
- Pylon campaign;
- Nexus/Pylon administration;
- Model Lab stewardship;
- work routing;
- spend authority;
- bitcoin rewards;
- accepted-work payout;
- settlement.

It projects planned, modeled, measured, verified, blocked, prohibited, and
settled states. That gives the essay a public claim-control layer:

```text
No accepted-work payout claim without accepted-work receipt chains.
No settlement claim without settlement receipt chains.
No spend or provider mutation claim without approved authority.
```

#### 2. Public Report Projection

`/artanis` and `GET /api/public/artanis/report` can aggregate public-safe
runtime state, loop state, health labels, Pylon stats, Model Lab reports,
Forum refs, receipt refs, blockers, caveats, and claim-state rows.

That means the outcomes-per-kWh proof packet can eventually have a public
wrapper:

```text
Artanis summarizes what is measured, modeled, blocked, prohibited, payable, or
settled without exposing private meter, provider, workroom, wallet, customer,
or raw log material.
```

#### 3. Work-Routing Proposals

Artanis can model work-routing proposals for inference, benchmark evaluation,
GEPA/DSPy optimization, LoRA/fine-tuning, training, embedding/data prep, and
validation.

Each proposal can carry:

- source evidence refs;
- target capability refs;
- acceptance criteria refs;
- risk label;
- resource mode;
- spend/cost caveat;
- approval requirement refs;
- traceable work refs and receipt refs when accepted.

This supports the essay's routing argument:

```text
Outcomes per kilowatt-hour improves when the system routes each accepted
outcome to the cheapest eligible capacity with the right trust, power, and
verification profile.
```

But the proposal is not dispatch. It does not prove work ran.

#### 4. Health And Staleness

Artanis health records track loop freshness, blocker reason, pending
approvals, Forum publication lag, Pylon stats freshness, Nexus public stats
freshness, Model Lab report freshness, and runner/backend availability.

This supports the essay's diligence posture:

```text
Stale evidence blocks overclaiming.
```

If a proof packet is old, missing, or blocked, Artanis can surface the stale
state instead of letting a polished headline outrun the evidence.

#### 5. D1 Persistence

Issue #403 added durable Artanis tables for runtime snapshots, loop records,
loop ticks, approval gates, health snapshots, work-routing proposals, and Forum
publication intents.

Persistence is evidence, not authority. This matters because outcomes per kWh
proof requires durable history:

```text
which loop saw which evidence
which proposal was made
which approval was required
which public summary was published
which blocker prevented a claim
```

#### 6. Pylon Readiness, Resource Modes, Marketplace Contracts, And Reward Visibility

Closed Artanis issues already give the vocabulary needed for an
outcomes-per-kWh proof packet:

- Pylon readiness states: source-ready, release-ready, platform-ready,
  eligible, accepted, paid, settled;
- resource modes: background, balanced, overnight, dedicated;
- marketplace job kinds: inference, optimization, fine-tuning/training,
  benchmark evaluation, embedding/data prep, validation;
- reward visibility that separates Forum content rewards from accepted-work
  payouts and settlement.

This lets the essay say:

```text
OpenAgents has the claim and contract vocabulary to separate online capacity,
eligible capacity, assigned work, accepted outcomes, payment intent, payout,
and settlement.
```

### Still Needed Before Definitive Substantiation

Open Artanis issues #405-#415 are the missing support layer for turning the
essay into a repeatable public proof program.

#### #405: Operator Console

Needed measurement support:

- operator view of loop state, blockers, approvals, and private evidence;
- approve/reject flows for risky proposals;
- spend/cost caps and rollback posture;
- private evidence by reference.

Why it matters:

```text
Measured watts, private provider data, workroom refs, and settlement refs
cannot all be exposed publicly. Operators need the private control surface
that decides what can be summarized by Artanis.
```

#### #406 And #407: Forum Delivery And Listener

Needed measurement support:

- delivery of public-safe proof summaries into canonical Forum topics;
- listener/triage for investor, operator, and agent questions;
- idempotent replies and blocker creation;
- unsafe material handling.

Why it matters:

```text
If the investor headline becomes public, Artanis needs a public question and
update loop that can say what is proven, what is modeled, and what remains
blocked.
```

#### #408: Nexus/Pylon Admin Adapters

Needed measurement support:

- Pylon fleet status;
- provider inventory;
- job offers and assignments;
- run status;
- artifacts;
- acceptance;
- payout and settlement caveats.

Why it matters:

```text
This is the main gap between contract and proof. Outcomes per kilowatt-hour
cannot be definitive until Artanis can read actual Nexus/Pylon evidence and
connect it to accepted outcome refs, energy refs, and payout/settlement refs.
```

#### #409: Resource-Mode Setup Command Packets

Needed measurement support:

- owner-approved background, overnight, and dedicated setup packets;
- CPU/GPU/memory/network/storage intent;
- pause/resume/checkpoint expectations;
- telemetry refs;
- resource-mode caveats;
- command dry-run and completion receipts.

Why it matters:

```text
The denominator depends on the operating mode. A background 20 percent CPU
mode and a dedicated full-blast mode are not the same energy claim.
```

#### #410: Marketplace Job Intake And Assignment

Needed measurement support:

- authenticated job intake;
- work kind;
- capability requirements;
- budget/spend cap;
- acceptance criteria;
- artifact requirements;
- public/private projection;
- proposed assignment state;
- separation of payment, acceptance, payout, and settlement.

Why it matters:

```text
The numerator depends on accepted outcomes with explicit criteria. A job that
was merely created, assigned, or run is not an accepted outcome.
```

#### #411: Continual-Learning Job Templates

Needed measurement support:

- eval reruns;
- GEPA/DSPy prompt/program optimization;
- dataset curation;
- adapter validation;
- LoRA/fine-tuning/training;
- regression analysis;
- benchmark target and acceptance criteria;
- rollback posture and approval requirement.

Why it matters:

```text
Outcomes per kilowatt-hour should apply to continual-learning work too, but
only when templates define what counts as accepted improvement and what
evidence proves it.
```

#### #412: Forum Bitcoin Reward Smoke

Needed measurement support:

- deterministic fake-bitcoin or approved live bitcoin reward smoke;
- receipt projection;
- earning notification;
- accepted-contribution bridge boundary;
- separation between Forum rewards and accepted-work payout.

Why it matters:

```text
Bitcoin rewards are useful market signals, but they cannot be counted as
accepted-work payout or provider settlement unless the receipt chain says so.
```

#### #413 And #414: Launch Communications And Production Launch Gate

Needed measurement support:

- public-safe launch copy;
- readiness status;
- production enable/disable/check/recover runbook;
- launch-gate checklist across persistence, runner, operator UI, Forum,
  Nexus/Pylon adapters, marketplace intake, rewards, and public report;
- rollback steps for claim mistakes.

Why it matters:

```text
Artanis cannot be described as continuously autonomous, and cannot be the
public proof steward for outcomes-per-kWh packets, until the production launch
gate is satisfied.
```

#### #415: Margot/Pylon Comparative Economics Evidence Packets

Needed measurement support:

- Margot simulator provenance: repo, commit, data timestamps, source URLs, and
  normalized export refs;
- GPU rental evidence: Vast.ai sample timestamp, GPU model, listing sample
  size, dollars per GPU-hour, TDP/source, and derived dollars per MWh;
- token economics evidence: OpenRouter price timestamp and raw/display units,
  ML.Energy run/task/GPU/J-token/tokens-sec/stability fields, and derived
  dollars per MWh;
- OCOLO or similar throughput-calculator refs as modeled evidence only, with
  URL/query, timestamp, model, hardware, context, quantization, and caveats;
- Pylon capacity evidence: node/cohort, GPU count/model, VRAM, interconnect,
  runtime/framework, resource mode, system power, chip TDP, effective watts per
  GPU, measured-meter availability, cooling/PUE assumption, availability
  window, and cost term;
- ERCOT/NYISO power-market windows, missing-data flags, and unsupported-market
  caveats for anything outside ERCOT/NYISO;
- mining counterfactuals for the same windows;
- accepted-work refs when actual work runs.

Why it matters:

```text
The investor comparison is no longer only "did this site run work?" It is
"which energy-settled value lane should each Pylon capacity slice enter, and
what evidence says so?"
```

## What We Need To Measure To Substantiate The Essay

The essay should be backed by a narrow first proof packet, not by a broad
portfolio claim.

### 1. Energy Denominator

Measure:

- device average watts;
- workroom runtime watt-hours;
- site or rack overhead when available;
- resource mode;
- start/end window;
- measured meter refs or modeled-energy refs;
- whether power data is measured, modeled, mixed, or unknown.

Do not claim:

- GPU chip TDP as facility energy;
- nameplate MW as usable AI capacity;
- accepted outcome energy efficiency when energy denominator is unknown.

### 2. Outcome Numerator

Measure:

- accepted outcome refs;
- work kind;
- assignment refs;
- artifact refs;
- grading refs;
- acceptance criteria refs;
- rejected/retried/failed/aborted counts;
- acceptance rate.

Do not claim:

- online node equals outcome;
- assigned job equals accepted outcome;
- runtime activity equals accepted revenue.

### 3. Economic Numerator

Measure:

- buyer accepted revenue;
- runner/model/provider costs;
- review/grading/retry/artifact costs;
- accepted gross profit;
- provider payable;
- provider settled amount;
- refund exposure.

Do not claim:

- modeled revenue equals accepted revenue;
- provider payable equals settled provider payout;
- Forum reward equals accepted-work payout.

### 4. Mining And Alternative Floor

Measure for the same time window:

- mining revenue per kWh or MWh;
- mining margin per kWh or MWh;
- power price;
- ASIC efficiency and uptime;
- pool/firmware/ops fees;
- VPS/colo opportunity cost where relevant;
- curtailment or grid-service value where relevant;
- idle/dark capacity.

Do not claim:

- AI beats mining without same-window opportunity-cost comparison;
- whole-site economics changed when only a small compute island was measured.

### 5. Routing And Governance

Measure:

- proposed route;
- selected route;
- rejected alternatives;
- approval gates;
- public claim state;
- health/staleness state;
- public-safe proof refs;
- operator-only private evidence refs.

Do not claim:

- Artanis self-authorized dispatch;
- Artanis had spend, wallet, provider, training, runtime, or settlement
  authority;
- stale proof remains current.

### 6. Comparative Model Inputs

Measure:

- Margot repo commit and data timestamps;
- Vast.ai pricing sample timestamp, listing sample size, GPU model, and TDP
  source;
- OpenRouter model price timestamp, raw price unit, and display unit;
- ML.Energy benchmark run, task, GPU, J/token, tokens/sec, stability flag, and
  number of GPUs;
- OCOLO or similar calculator URL/query/source when used as a modeled
  throughput reference;
- node system power, chip TDP, effective watts per GPU, and whether the value
  is measured, vendor-rated, modeled, or operator-supplied;
- token throughput estimate, quantization, context length, framework,
  interconnect, active parameters, and VRAM-fit result;
- ERCOT/NYISO zone, LMP window, refresh timestamp, and unsupported-market
  caveats.

Do not claim:

- token economics are diligence-ready before unit labels and sample arithmetic
  have been audited;
- ML.Energy, OCOLO, or OpenRouter rows transfer directly to OpenAgents
  workloads without workload, latency, batching, and acceptance caveats;
- chip-only rental `$ / MWh` is equivalent to full node or facility economics;
- PJM or another unsupported market is in-scope without an explicit caveat.

## First Diligence-Grade Proof Packet

The first packet should be a Margot-compatible comparative Pylon packet. It may
include one site, node, or device cohort, but it should not be framed as only a
site test. The point is to prove that the product can join capacity,
electricity, mining, GPU rental, token inference, node power, and accepted-work
evidence without collapsing them into one overconfident number.

Minimum design:

1. Export Margot-compatible baselines for the selected ERCOT/NYISO windows:
   mining floor, power cost, GPU rental floor, token inference floor,
   node/system-power-adjusted floor, and unsupported-market caveats.
2. Build a Pylon capacity snapshot for the selected node or cohort: hardware,
   resource mode, runtime/framework, system power, TDP, metering availability,
   utilization assumption, cost term, and availability window.
3. Run 25-100 low-risk jobs across document processing, eval/replay, artifact
   validation, coding-agent support, embedding/indexing, benchmark work, or
   other workroom tasks that can produce acceptance refs quickly.
4. Record assignment, execution, artifact, grading, acceptance, rejection,
   retry, and closeout refs.
5. Record measured or explicitly modeled watt-hours for each work window,
   with denominator type: chip TDP, node system power, PDU/IPMI/meter, PUE
   adjusted, mixed, or unknown.
6. Calculate accepted outcomes per kWh and accepted revenue/gross profit per
   MWh.
7. Calculate provider payable per kWh and settlement state.
8. Compare each window against mining, GPU rental, token inference,
   VPS/colo, curtailment, and idle alternatives.
9. Record unit-audit refs, freshness/staleness refs, dark-capacity MWh, and
   reason refs.
10. Let Artanis publish only the public-safe summary and blocker list after
    the relevant launch gates are complete.

The strongest investor-safe sentence after that packet would be:

```text
In this measured Pylon/Margot packet, OpenAgents converted [X] kWh into [Y]
accepted outcomes, [Z] accepted gross profit per MWh, and [P] provider payable
or settled value per kWh, compared against same-window mining, GPU rental,
token inference, power-cost, and idle-capacity floors. Values are labeled
measured, modeled, payable, or settled.
```

Until that packet exists, the safer investor-email sentence is:

```text
OpenAgents is building the missing metric for bitcoin + AI: outcomes per
kilowatt-hour. Mining gives the energy-native floor. Accepted outcomes give
the upside. Our next Artanis/Pylon packet compares which MWh should become
mining, GPU rental, token inference, accepted work, dark capacity, payable
provider value, or settled payout.
```

## Public Claim Guardrails

Safe to say now:

- Outcomes per kilowatt-hour is the right metric for the bitcoin + AI thesis.
- OpenAgents product surface has typed contracts for accepted outcomes per energy, investor
  economics, Margot export ingestion, Artanis claim-state projection, and
  Pylon marketplace/readiness/resource-mode boundaries.
- Margot's current simulator can model GPU rental, token inference, node-power,
  mining, and ERCOT/NYISO power-market comparisons in common per-MWh units.
- Artanis can become the public proof steward for outcomes-per-kWh packets.
- Mining is the economic floor; accepted outcomes are the upside.

Not safe to say yet:

- OpenAgents has definitively proven outcomes per kilowatt-hour across a site or
  portfolio.
- Artanis is continuously autonomous in production.
- Pylon capacity is payout-proven because it is online.
- Forum rewards are accepted-work payouts.
- Provider payable is settled payout.
- Modeled AI revenue beats mining in measured operation.
- Nameplate MW is sellable AI capacity.
- Token `$ / MWh` rows are investor-ready before unit labels, source freshness,
  and sample arithmetic are audited.
- ML.Energy or OCOLO-style throughput estimates prove OpenAgents workload
  economics without local Pylon/runtime evidence.

The essay should therefore sell the metric and the proof discipline, not claim
the final measured result before the first packet exists.
