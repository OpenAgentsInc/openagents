# Mullet Mining Investor Measurement Audit

Date: 2026-06-06

Status: first OpenAgents product surface mining audit for investor-grade measurement, Margot export
ingestion, and Artanis-facing proof discipline.

## Bottom Line

Mullet mining is investable only when the priority rule is explicit:

```text
AI / HPC gets the front-of-house priority.
Bitcoin mining absorbs underfilled, residual, ramp, and flexible capacity.
Mining curtails when AI, power prices, grid events, or reserves need the power.
```

Mining-led opportunistic AI is still useful, especially for SHC-style sites,
but it is not the same claim. Until a site has AI-priority capacity, accepted
work receipts, measured power, and backfill evidence, OpenAgents product surface should classify it
as `mining_led_ai_pilot_not_mullet`, not as proven mullet mining.

The investor case should therefore be measured in this order:

1. capacity that exists;
2. capacity that is eligible;
3. capacity that ran work;
4. work that was accepted;
5. accepted work per kWh / MWh;
6. mining, VPS, colo, grid-service, and curtailment opportunity cost;
7. provider payable and settlement state;
8. proof that AI priority did not destroy mining backfill, flexibility, or SLA
   credibility.

The first public/investor-safe claim is not "AI beats mining." It is:

```text
OpenAgents can compare mining, AI work, curtailment, reserve, VPS/colo, and
idle choices in common per-MWh units, then attach receipts that show which MWh
became accepted outcomes, mining revenue, dark capacity, or settled payout.
```

## Source Scope

OpenAgents product surface issue scope checked with `gh issue list` / `gh issue view`:

- open issues #403-#414: the active Artanis implementation queue, covering D1
  persistence, scheduled ticks, operator console, Forum publication/listening,
  Nexus/Pylon admin adapters, Pylon resource modes, marketplace intake,
  continual-learning jobs, Forum reward smoke, launch communications, and the
  production launch gate;
- recently closed issues #361-#367: investor outcome economics, capacity
  funnel, accepted outcomes per power, flex profiles, flexible-load telemetry,
  forward-power scenarios, and Margot export ingestion;
- recently closed issues #395-#402: Artanis work routing, autonomy claim
  ledger, launch smoke, Pylon v0.2 readiness, resource modes, marketplace job
  contract, Forum bitcoin reward visibility, and payment simulation.

OpenAgents product surface docs read:

- `docs/artanis/2026-06-06-artanis-implementation-audit.md`
- `docs/pylon/2026-06-06-r10-artanis-pylon-campaign-ledger.md`
- `docs/omni/2026-06-06-investor-grade-outcome-economics-metrics.md`
- `docs/omni/2026-06-06-accepted-outcomes-per-power.md`
- `docs/omni/2026-06-06-margot-export-ingestion-contract.md`
- `docs/omni/2026-06-06-investor-demo-bundle-export.md`
- `docs/pylon/2026-06-06-capacity-funnel-and-dark-capacity.md`
- `docs/pylon/2026-06-06-flexible-load-profiles.md`
- `docs/pylon/2026-06-06-flexible-load-event-telemetry.md`
- `docs/pylon/2026-06-06-forward-power-interconnection-scenarios.md`

Workspace-root Margot/mining sources read outside OpenAgents product surface:

- `../docs/mining/README.md`
- `../docs/mining/DESIGN.md`
- `../docs/mining/MULLET_MINING_MVP_SPEC.md`
- `../docs/mining/2026-05-25-openagents-margot-paez-shc-call-transcript.md`
- `../docs/mining/chris-margot-conversation-june-3-2026.md`
- `../docs/mining/shc/2026-05-25-margot-paez-shc-dashboard-requests.md`
- `../docs/mining/margot-paez-dissertation.pdf`
- `../docs/omni/margot-paez-flexible-compute-synthesis.md`
- `../vortex/docs/operator-energy-margot-model.md`
- `../docs/mining/margot-mvp.jpeg`
- `../projects/repos/oa_aibtc_model`

The local Margot dissertation PDF was present, but local text extraction did
not yield useful text. This audit therefore relies on the workspace-extracted
dissertation notes, Margot conversation/call transcripts, and the
`oa_aibtc_model` implementation for dissertation-derived metrics.

External framing checked:

- Hashrate Index, "Mullet Mining: Running AI and Bitcoin at the Same Site":
  `https://hashrateindex.com/blog/mullet-mining-ai-bitcoin-mining-same-site/`
- Duke Nicholas Institute, "Rethinking Load Growth":
  `https://nicholasinstitute.duke.edu/index.php/publications/rethinking-load-growth`
- Duke report PDF:
  `https://nicholasinstitute.duke.edu/sites/default/files/publications/rethinking-load-growth.pdf`
- Paez et al. arXiv, "Aligning load flexibility with emissions reduction":
  `https://arxiv.org/abs/2509.04380`

## Artanis Relevance

Artanis is the right public steward for the proof story, but not yet the
authority for production autonomous claims.

Current OpenAgents product surface truth from the Artanis audit:

- `/artanis` and `/agents/artanis` are public-safe proof/status surfaces.
- Artanis has public Pylon stats, public goal projection, R10 Pylon campaign
  claims, standalone autonomy claim ledger, and Forum reward visibility.
- The fully continuous autonomous agent loop is still gated by open issues
  #403-#414.
- Issue #414 is the production launch gate. Public copy cannot say Artanis is
  continuously autonomous until persistence, scheduled runner, operator
  console, approval gates, Forum delivery/listener, Nexus/Pylon adapters,
  marketplace intake, reward boundaries, and public report projection all pass
  the launch gate.

For mining/investor work, Artanis should:

- summarize Margot export packets and missing diligence;
- publish public-safe Forum updates after operator approval;
- connect Pylon job proposals to measured resource modes and capacity funnel
  states;
- point investors to proof bundles, not raw simulator output;
- keep modeled, measured, accepted, paid, and settled states separate.

Artanis must not:

- self-authorize dispatch, provider mutation, wallet spend, payout, settlement,
  training promotion, or public claim upgrades;
- expose raw meter data, private provider material, wallet/payment material,
  customer data, raw logs, private repo refs, or raw timestamps;
- state that a pilot is "mullet mining" until AI priority and mining backfill
  are measured.

## Margot Data Findings

Margot's core contribution is the common-denominator measurement layer.

Her SHC call made the first data ask very specific:

- grid, off-grid, or behind-the-meter status;
- fixed price, real-time price exposure, gas-site deal, or blended tariff;
- ERCOT load zone, hub, settlement point, or bus;
- demand-response enrollment, curtailment trigger, and curtailment economics;
- ASIC models, machine efficiency, hashrate, pool fee, uptime, and payout
  history;
- installed and active kW by site;
- CPU, GPU, server, VPS, and colo inventory;
- GPU cooling and thermal constraints;
- commercial structure: who owns hardware, who earns which share, and what
  OpenAgents revenue means in dollars per MWh.

The important unit is:

```text
dollars per MWh
```

Margot can compare mining, AI work, curtailment, and demand response only after
OpenAgents converts accepted work, GPU rental, token inference, or agentic
runtime into a per-MWh value with caveats.

### `oa_aibtc_model`

The local `projects/repos/oa_aibtc_model` repo is a standalone Dash Facility
Revenue Advisor. It combines:

- block-by-block mining economics;
- ASIC machine efficiency data;
- ERCOT and NYISO hourly LMP caches;
- GPU rental pricing;
- ML.Energy benchmark data;
- token inference revenue estimates;
- Margot dissertation-derived curtailment metrics.

Relevant bundled data:

- 26 GPU models in `data/gpu_pricing.json`, scraped from Vast.ai-style spot
  bundles on 2026-06-01;
- 838 ML.Energy inference benchmark runs in `data/ml_energy_leaderboard.json`;
- 29 priced model/task entries in `data/model_revenue.json`, fetched
  2026-06-02;
- ERCOT load-zone caches for `LZ_AEN`, `LZ_CPS`, `LZ_HOUSTON`, `LZ_LCRA`,
  `LZ_NORTH`, `LZ_RAYBN`, `LZ_SOUTH`, and `LZ_WEST`;
- NYISO caches for the bundled zones;
- one full-system B300 node config from Ben's GPU server specs.

Core formulas:

```text
R_pe_facility = R_pe_network * (eta_network / eta_facility)
R_AI_rental = contract_rate_usd_per_gpu_hour * 1,000,000 / watts_per_gpu
R_AI_token = completion_price_usd_per_token / joules_per_token * 3.6e9
```

The simulator curtails mining when effective electricity price exceeds
facility `R_pe`, and curtails AI when effective electricity price exceeds
`R_AI`. It then computes mining profit on the remaining mining allocation plus
AI profit on the AI allocation.

Useful current data points from the JSON files:

- GPU rental floor examples by chip TDP:
  - Tesla V100: about `$76/MWh`;
  - RTX 4090: about `$297/MWh`;
  - H100 SXM: about `$2,096/MWh`;
  - B200: about `$4,487/MWh`;
  - B300 SXM6 AC: about `$4,848/MWh`;
  - H100 NVL: about `$5,352/MWh`.
- The B300 full-system node config corrects chip-TDP optimism:
  - 8 GPUs;
  - 12,584 W system draw;
  - 1,573 W effective per GPU;
  - `$6.7872/GPU-hour`;
  - about `$4,315/MWh` using full-system effective power.
- Token model/task `R_AI` values in the current JSON range from about
  `$568/MWh` to about `$27,140/MWh`, with an average near `$6,989/MWh`.

Those are gross comparators, not bankable revenue. Vast.ai-style minimum spot
prices are not contracts, chip TDP is not facility power, and token
revenue/MWh is not accepted-work revenue.

## What To Measure

The investor case needs a proof ladder. Each layer should have a state:
`public_claim`, `customer_reported`, `modeled`, `measured`, `verified`,
`accepted`, `paid`, or `settled`.

### 1. Site And Power Inventory

Measure:

- site IDs and regions;
- ISO, load zone, hub, settlement point, or bus;
- contracted MW;
- usable MW;
- module/container count;
- active load by mining, VPS, colo, AI, cooling, network, and idle;
- power contract type;
- all-in electricity price;
- demand charges and take-or-pay exposure;
- real-time price signal actually used by the operator;
- curtailment rights and obligations;
- reserve and demand-response eligibility.

Investor use:

- proves raw MW is not being presented as sellable AI capacity;
- supports forward-power and interconnection scenarios;
- establishes whether the site can even operate a mullet priority rule.

OpenAgents product surface surface:

- `PylonForwardPowerScenarioRecord`
- `MargotExportPacket`
- Pylon capacity funnel records

### 2. Mining Floor

Measure:

- ASIC fleet model and efficiency in J/TH;
- site hashrate;
- pool fee;
- mining uptime;
- mining revenue per MWh;
- mining gross and net profit per kWh/MWh;
- mining breakeven power price;
- curtailment windows;
- mining backfill revenue during AI ramp;
- mining opportunity cost when AI displaces mining.

Investor use:

- gives the economic floor;
- shows when AI genuinely clears the floor;
- protects the thesis from "AI revenue" that destroys a better mining or
  curtailment option.

OpenAgents product surface surface:

- Margot export mining floor fields;
- capacity funnel stage and dark-capacity reason refs;
- accepted outcomes per power when work displaces mining.

### 3. AI / Agentic Revenue

Measure separately:

- GPU rental floor per MWh;
- token inference revenue per MWh;
- CPU/container runtime revenue per MWh;
- accepted-artifact or accepted-outcome revenue per MWh;
- utilization;
- rejection rate;
- retry cost;
- grading/review/proof cost;
- external model cost;
- local runtime cost;
- host share and platform share;
- provider payable and provider settled value.

Investor use:

- distinguishes commodity GPU-hour resale from OpenAgents-native accepted
  outcomes;
- shows whether application-layer agent work clears mining, VPS, colo, and
  power costs;
- supports a data advantage claim only when outcomes are accepted and routed
  with receipts.

OpenAgents product surface surface:

- `OmniInvestorOutcomeEconomicsMetricRecord`
- `OmniOutcomePowerProductivityRecord`
- public proof bundles
- investor demo bundle export

### 4. Capacity Funnel And Dark Capacity

Measure:

- registered devices;
- benchmarked devices;
- eligible devices;
- assigned devices;
- running devices;
- artifact-producing devices;
- accepted-work devices;
- paid devices;
- settled devices;
- dark-capacity devices and MWh;
- dark reasons: no work assigned, missing payout target, not benchmarked, not
  eligible, trust block, low connectivity, insufficient liquidity, failed run,
  not accepted.

Investor use:

- prevents vanity node counts;
- converts "we have capacity" into a funnel that can be diligenced;
- makes dead capacity visible rather than quietly hiding it.

OpenAgents product surface surface:

- `accountPylonCapacityFunnel`
- investor demo bundle missing-evidence items

### 5. Energy Productivity

Measure:

- accepted outcomes per kWh;
- accepted outcomes per MWh;
- accepted revenue per kWh;
- accepted gross profit per kWh;
- provider payable per kWh;
- provider settled per kWh;
- dark-capacity MWh;
- modeled vs measured energy state.

Investor use:

- turns the pitch from "we have GPUs" into "this much energy became accepted
  useful work";
- creates a comparable KPI across mining sites, Pylons, GCP, SHC, Tinybox,
  consumer devices, and future pods.

OpenAgents product surface surface:

- `projectOmniOutcomePowerProductivity`

### 6. Flexibility And Power-Event Response

Measure:

- work-class flexibility: fixed, deferrable, interruptible, preemptible,
  opportunistic;
- interruption tolerance;
- checkpoint cadence;
- resume requirement;
- deadline window;
- verification after resume;
- replay cost;
- requested power response watts;
- actual power response watts;
- response ratio;
- lost-work cost;
- accepted-work impact after interruption.

Investor use:

- supports the Duke-style claim that small credible curtailment can unlock
  grid headroom;
- proves which AI workloads can behave like flexible load and which cannot;
- avoids importing Bitcoin mining's flexibility into AI workloads by
  assumption.

OpenAgents product surface surface:

- Pylon flexible-load profiles;
- Pylon flexible-load event telemetry.

### 7. Physical And SLA Readiness

Measure:

- cooling type;
- rack density;
- CRAC capacity;
- inlet temperature and thermal alarms;
- full-system power, not only chip TDP;
- PUE or site overhead;
- fiber and bandwidth;
- packet loss and RTT to target metros;
- remote-hands response;
- monitoring and incident process;
- Open Beta vs GA SLA boundary;
- customer isolation and security posture.

Investor use:

- tells the truth about mining sites that are power-rich but not AI-ready;
- makes incremental small GPU pods or agentic CPU work credible without
  pretending the site is a Tier 3 AI data center.

OpenAgents product surface surface:

- Margot export caveat refs;
- Pylon capacity and resource-mode records;
- future operator evidence bundles.

### 8. Emissions And Grid Alignment

Measure:

- locational marginal emissions where available;
- avoided emissions during actual curtailment;
- induced emissions from added AI load;
- curtailment effectiveness score;
- LME variability;
- curtailment by LME percentile;
- difference between price-responsive and emissions-responsive dispatch.

Investor use:

- prevents green overclaims;
- ties Margot's dissertation discipline to the investor story;
- shows whether flexibility improves grid/emissions outcomes or merely lowers
  operator cost.

OpenAgents product surface surface:

- Margot export caveats and provenance;
- future measured power and LME refs.

### 9. Capital And Payback

Measure:

- hardware capex or lease cost;
- cooling capex;
- fiber/network capex;
- installation and remote-hands cost;
- hardware depreciation;
- support cost;
- GPU utilization hurdle to beat mining;
- accepted-work rate hurdle to beat mining and VPS/colo;
- payback months by phase;
- bear/base/bull sensitivity.

Investor use:

- converts the thesis into a staged deployment plan;
- clarifies when to use SHC-owned compute, colocate known-good hardware, or
  build a hybrid pod.

OpenAgents product surface surface:

- Margot export scenario/diligence refs;
- investor demo bundle export;
- forward-power/interconnection scenarios where applicable.

## First Investor Proof Packet

The first packet should be a 30-day pilot report, not a deck-only claim.

Recommended packet sections:

- site and device inventory;
- provenance state of each fact;
- power contract and ERCOT/NYISO market context;
- mining floor and breakeven;
- VPS/colo opportunity cost if applicable;
- AI workload classes attempted;
- flexible-load profile for each class;
- capacity funnel counts;
- dark-capacity reason summary;
- accepted outcomes per kWh/MWh;
- accepted revenue and accepted gross profit;
- provider payable vs settled amounts;
- mining backfill revenue;
- curtailment/power-event response evidence;
- cooling/network/SLA blockers;
- Margot export packet with caveats;
- investor demo bundle readiness state;
- missing evidence list.

Minimum pilot design:

1. Run SHC-owned CPU/server agentic work first.
2. Colocate one known-good GPU server second.
3. Run 25-100 low-risk jobs across document processing, validation/eval,
   coding-agent support, embedding/indexing, and artifact checks.
4. Record energy, runtime, acceptance, retry, rejection, and payout state.
5. Compare every work window against mining, VPS/colo, power cost, and
   curtailment value.
6. Only then model a 10-25 kW AI-priority pod with mining backfill.

The packet is investor-ready only if every headline number carries its state:
modeled, measured, accepted, paid, or settled.

## Investor Narrative

The strongest narrative:

```text
Bitcoin mining created a liquid floor for energy-native computation.
AI creates higher-value digital work, but most AI work is not equally firm,
latency-sensitive, or infrastructure-ready.
OpenAgents designs agentic workloads that can run on awkward capacity, proves
accepted outcomes, and uses mining as the economic floor and backfill.
Margot's model makes the per-MWh economics honest.
OpenAgents product surface receipts make accepted work, dark capacity, payable, and settlement
auditable.
```

What investors should see:

- not raw MW;
- not generic GPU marketplace screenshots;
- not "miners become AI data centers" language;
- but measured conversion of constrained energy into accepted outcomes above a
  mining/VPS/curtailment floor.

## Claim Guardrails

Safe to say now:

- Margot's model gives OpenAgents the method to compare mining and AI in
  dollars per MWh.
- OpenAgents product surface now has read-only contracts for investor economics, power productivity,
  capacity funnel accounting, flex profiles, event telemetry, forward-power
  scenarios, and Margot export ingestion.
- Artanis can become the public steward for these proof packets after the
  production launch gate, using Forum posts and `/artanis` as public-safe
  summaries.
- SHC-style operators are plausible first pilots for mining-led AI, agentic
  CPU work, and known-good GPU colocation.

Do not say yet:

- a site is full mullet mining unless AI has explicit priority over mining and
  mining is measured as backfill;
- public portfolio kW equals sellable AI capacity;
- GPU chip TDP equals facility power;
- modeled AI revenue equals accepted-work revenue;
- accepted-work revenue equals settled provider payout;
- Forum rewards equal accepted-work payouts;
- emissions reductions are proven without LME/meter methodology;
- Artanis is continuously autonomous before #414 is satisfied.

## Next Work

Immediate OpenAgents product surface work should be:

1. Create a `mullet_mining_proof_packet` doc/contract shape that joins Margot
   exports, capacity funnel accounting, outcome-power productivity, investor
   economics, and public proof bundles.
2. Add a Margot packet example for a public-data-only ERCOT baseline and mark
   every value `modeled`.
3. Add an SHC data-request checklist from the Margot call and Mullet MVP spec.
4. Add a pilot scorecard template with work classes, energy use, accepted work,
   mining opportunity cost, dark capacity, and settlement state.
5. Let Artanis summarize only public-safe packet state once the Forum delivery
   and launch-gate issues are complete.

The first useful investor artifact should be a narrow, caveated proof packet
for one site or one device cohort. It should show what was measured, what was
modeled, what was accepted, what was paid or settled, and what remains blocked.
