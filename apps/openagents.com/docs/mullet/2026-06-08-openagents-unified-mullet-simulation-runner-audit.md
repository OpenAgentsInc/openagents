# OpenAgents product surface Unified Mullet Simulation Runner Audit

Date: 2026-06-08

Status: planning audit for a private admin-only OpenAgents product surface implementation.

Requested product surface: `/mullet`

Requested access: private, admin-only, limited to `chris@openagents.com`.

Account authority note: Christopher confirmed the intended operator email is
`chris@openagents.com`. The original request contained the misspelling
`chris@openaegnts.com`; that spelling must be denied by runtime policy and kept
out of allowlists.

## Goal

Build the unified mining, AI, and accepted-outcome simulation runner inside
OpenAgents product surface as an internal operator tool. The first version should let Christopher
run and compare scenarios that join:

- Margot Paez-style facility and Bitcoin-mining economics;
- raw GPU marketplace and token/API inference fallback values;
- OpenAgents accepted-outcome unit economics from the combined model plan;
- provider opportunity floors across mining, VPS, colo, raw GPU rental,
  curtailment, grid-service value, and idle;
- party-specific returns for OpenAgents, facility operator, hardware owner,
  validators, reviewers, and providers;
- proof, provenance, settlement, energy, and market-memory state.

The route should be an operator simulator, not a public claim surface and not a
runtime dispatch authority. A scenario may say "this modeled node would run
accepted work in this hour," but it must not by itself assign live work, mutate
providers, trigger payments, settle Bitcoin, publish investor claims, or mark
modeled values as measured truth.

## Source Scope

Primary root workspace sources:

- `../docs/mining/2026-06-08-openagents-combined-mining-ai-revenue-model-plan.md`
- `../docs/mining/README.md`
- `../docs/mining/MODEL1.md`
- `../docs/mining/DESIGN.md`
- `../docs/mining/MULLET_MINING_MVP_SPEC.md`
- `../docs/mining/2026-05-21-miner-profitability-and-agentic-ai-dashboard-mvp-spec.md`
- `../docs/mining/2026-05-20-openagents-small-lab-to-trillion-dollar-lab-roadmap.md`
- `../docs/mining/2026-05-19-compute-infra-ipp-mining-response.md`
- `../docs/mining/2026-05-11-openagents-inference-shift-pylon-stranded-compute.md`
- `../docs/mining/2026-04-11-consumer-compute-distributed-training-architecture-proposal.md`
- `../docs/mining/analysis0.md`
- `../docs/mining/analysis1.md`
- `../docs/mining/zitron-solution.md`
- `../projects/repos/oa_aibtc_model` as read-only Margot model reference.

Primary OpenAgents product surface sources:

- `docs/mining/2026-06-06-mullet-mining-investor-measurement-audit.md`
- `docs/mining/2026-06-06-outcomes-per-watt-investor-essay.md`
- `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- `INVARIANTS.md`
- `apps/web/src/route.ts`
- `apps/web/src/product-policy.ts`
- `apps/web/src/routing/startup.ts`
- `workers/api/src/admin-access.test.ts`
- `workers/api/src/admin-overview-routes.ts`

Effect guidance checked with `effect-solutions list` and
`effect-solutions show data-modeling services-and-layers error-handling
testing`.

## Product Requirement

`/mullet` should be a private OpenAgents product surface route for one operator to run scenario
simulations and generate internal diligence packets.

The first useful product workflow:

1. Open `/mullet`.
2. Confirm the authenticated session is the configured Christopher admin
   account.
3. Select a scenario template:
   - Tinybox at SHC-style power;
   - Tinybox at residential power;
   - Tinybox at West Texas miner-site power;
   - 100 MW facility with 80 percent mining and 20 percent AI allocation;
   - SHC CPU/VPS/colo site;
   - miner-site GPU island.
4. Edit assumptions with provenance attached to every input.
5. Run a simulation.
6. Compare hourly dispatch across:
   - mine;
   - raw GPU market;
   - token/API inference;
   - OpenAgents accepted work;
   - curtail;
   - idle or reserve.
7. View outputs by party:
   - OpenAgents revenue, COGS, margin, risk reserve;
   - provider payout and provider net;
   - facility/operator revenue;
   - hardware-owner payback, IRR, NPV;
   - validators and reviewers payout;
   - accepted outcomes per kWh/MWh.
8. Export an internal packet that keeps modeled, measured, accepted, paid, and
   settled values separate.

The route should favor dense operator information over marketing language:
scenario controls, dispatch table, breakeven table, sensitivity deltas,
provenance badges, and export actions.

## Access Contract

Access must be enforced on both sides:

- Browser route gate: `/mullet` appears only for an authenticated, onboarded,
  admin user whose email matches the approved Christopher account.
- Worker API gate: every `/api/mullet/*` handler must require a browser session
  and repeat the email allowlist check. Browser gating is not authority.

The existing OpenAgents product surface pattern already has:

- `OPENAGENTS_ADMIN_EMAILS` in `workers/api/src/index.ts`, currently tested as
  exactly `['chris@openagents.com']`;
- `isOpenAgentsAdminEmail(...)` with case-insensitive matching;
- `auth.isAdmin` and `loggedInAdminAccessAllowed(...)` in browser product
  policy;
- `/api/admin/overview` route tests for unauthorized, forbidden, and accepted
  admin requests.

The mullet implementation should either:

1. reuse the existing admin allowlist after confirming the correct email is
   `chris@openagents.com`; or
2. add a narrower `OPENAGENTS_MULLET_EMAILS` config boundary and test that it
   contains exactly the intended account.

Do not hide this behind a UI-only check. Do not use query parameters, route
fragments, local storage, or feature flags as authority for access.

## URL And Route Shape

Browser route:

```text
/mullet
```

API routes:

```text
GET  /api/mullet/bootstrap
GET  /api/mullet/scenarios
POST /api/mullet/scenarios
GET  /api/mullet/scenarios/:scenarioId
POST /api/mullet/runs
GET  /api/mullet/runs/:runId
GET  /api/mullet/runs/:runId/export
```

No public URLs should carry simulation state in query parameters. If the UI
later needs to deep-link to a run, prefer a path route such as
`/mullet/runs/:runId` only after the browser router, clean-URL policy, and API
auth tests are updated.

## Authority Boundaries

The runner must be simulation-only until a separate approved path exists.

Allowed:

- create, update, and delete private scenarios;
- run deterministic simulations;
- store modeled results;
- import approved baseline CSV/JSON fixtures;
- export private internal diligence packets;
- attach proof packet references when they already exist;
- mark values as modeled, measured, verified, accepted, payable, or settled
  according to evidence refs.

Denied:

- assigning live Pylon work;
- mutating provider state;
- creating or paying Lightning invoices;
- settling Bitcoin payouts;
- posting Forum claims;
- promoting public investor claims;
- claiming measured power from modeled assumptions;
- treating Margot gross revenue, Vast-style GPU prices, token revenue, or
  OpenAgents demand assumptions as settled truth.

The `INVARIANTS.md` section named "Mullet Simulation Runner Authority" is the
runtime-policy source for this boundary. Mullet records are private simulation
evidence and cannot authorize dispatch, payout, settlement, public claims,
provider mutation, or work acceptance.

## Domain Model

The core model should be Schema-first and shared between the Worker and browser.
The cleanest placement is a new workspace package:

```text
packages/mullet-schema/
```

The pure calculation engine can either live beside it:

```text
packages/mullet-sim/
```

or as a Worker-owned module if the browser never evaluates simulations locally.
The better long-term shape is a pure package with no Cloudflare or DOM
dependencies, then Worker routes persist and execute runs through services.

### Branded primitives

Use branded Schema primitives for semantically meaningful values:

```text
ScenarioId
SimulationRunId
FacilityId
SiteId
NodeId
WorkClassId
ProofPacketId
MarketMemoryId
EnergyTelemetryRecordId
UsdCents
UsdPerMWh
UsdPerKWh
Watts
Kilowatts
Megawatts
KWh
MWh
Percent
Confidence
IsoTimestamp
```

Do not pass raw `number` everywhere. Mixing kWh, MWh, watts, dollars per hour,
and dollars per accepted outcome is the biggest modeling failure mode.

### Variants

Use tagged variants, not boolean bags:

```text
ProvenanceLevel =
  public_claim
  | customer_reported
  | manual_input
  | estimated
  | modeled
  | forecast
  | observed
  | measured
  | verified
  | accepted
  | paid
  | settled
  | placeholder

SiteClassification =
  mining_only
  | mining_led_ai_pilot_not_mullet
  | balanced_hybrid
  | mullet_ai_led_mining_backfill
  | colo_only_candidate
  | neither_no_fit

CapacityLifecycleState =
  discovered
  | enrolled
  | eligible
  | admitted
  | assigned
  | completed
  | accepted
  | rejected
  | settled
  | payout_proven

DispatchMode =
  mine
  | raw_gpu_market
  | token_api_inference
  | openagents_accepted_work
  | curtail
  | idle
  | reserve
```

### Records

Minimum record set:

```text
ProvenancedValue<T>
Scenario
Facility
PhysicalReadinessProfile
MiningFleet
ComputeNode
RuntimeBenchmark
WorkClass
WorkClassFlexibility
ProviderBidPolicy
PartySplit
HourlyCandidateMode
HourlyDispatchResult
CapitalReturnSummary
AcceptedWorkProofPacket
MarketMemory
EnergyTelemetryRecord
SimulationRun
SimulationRunExport
```

Every scenario input that can become a partner-facing number should carry:

```text
value
unit
provenance
confidence
source
last_updated
needs_diligence
```

## Simulation Layers

The runner should compute in layers so each piece can be tested independently.

### Layer 1: site and energy

Inputs:

- facility capacity;
- market, zone, hub, settlement point, or bus;
- effective power price;
- fixed, blended, real-time, gas-site, or behind-the-meter contract shape;
- curtailment policy;
- grid-service or reserve value;
- PUE, cooling multiplier, remote hands, and site ops cost.

Outputs:

- effective electricity price;
- available MW;
- consumed MWh;
- curtailed MW;
- power cost;
- grid value;
- provenance and confidence.

### Layer 2: mining floor

Inputs:

- ASIC model;
- hashrate;
- efficiency;
- BTC price;
- hashprice or network revenue per energy;
- pool fee;
- firmware and ops cost;
- capex and depreciation.

Outputs:

- mining revenue per MWh;
- mining margin per MWh;
- mining breakeven power price;
- mining floor variants:
  - gross revenue;
  - contribution margin;
  - net after depreciation;
  - operator hurdle.

### Layer 3: hardware and runtime

Inputs:

- node type;
- hardware owner;
- facility operator;
- GPU count, model, VRAM, interconnect;
- CPU, RAM, storage, network;
- idle power and load power;
- capex and depreciation;
- fallback-market eligibility;
- trust tier and readiness state;
- measured runtime benchmarks.

Outputs:

- attempts per hour;
- kWh per attempt;
- wall-clock time;
- failure rate;
- fallback value;
- support burden.

### Layer 4: work-class economics

Inputs:

- buyer price;
- acceptance rate;
- demand backlog or fill;
- model/API costs;
- workroom runtime;
- provider runtime;
- validator count and payouts;
- grader cost;
- human review minutes;
- storage and settlement cost;
- retries, failures, support, and risk reserve;
- interruption contract.

Outputs:

- cost per attempt;
- cost per accepted outcome;
- recommended buyer price;
- OpenAgents margin;
- provider payable;
- accepted outcomes per kWh/MWh.

### Layer 5: dispatch and capital returns

For every hour and candidate mode:

- compute candidate revenue, COGS, provider payout, margin, and risk-adjusted
  value;
- enforce work-class flexibility, privacy, SLA, state-locality, and readiness
  gates;
- compare against mining floor, raw GPU floor, VPS/colo floor,
  curtailment/grid-service value, and idle;
- select the winning mode;
- compute party-specific payback, IRR, NPV, and downside protection.

The dispatch result should explain why a mode won or lost. A decision with no
reason code is not useful for diligence.

## Effect Architecture

Use the repo's Effect-native end-state patterns.

### Packages

Recommended package split:

```text
packages/mullet-schema/
  src/index.ts
  src/index.test.ts

packages/mullet-sim/
  src/facility.ts
  src/mining.ts
  src/hardware.ts
  src/work-class.ts
  src/provider-floor.ts
  src/dispatch.ts
  src/capital.ts
  src/index.ts
  src/*.test.ts
```

Worker integration:

```text
workers/api/src/mullet/
  errors.ts
  repository.ts
  routes.ts
  service.ts
  export.ts
  fixtures.ts
```

Browser integration:

```text
apps/web/src/page/loggedIn/mullet/
  model.ts
  message.ts
  update.ts
  view.ts
  transitions.ts
```

### Services

Define services with `Context.Service` and implement with layers:

```text
MulletAccessPolicy
MulletScenarioRepository
MulletSimulationEngine
MulletBaselineImporter
MulletRunRepository
MulletExportService
```

Service methods should return typed `Effect` values and tagged errors. Do not
return `Response` from domain services. HTTP mapping belongs at the route
boundary.

### Errors

Use `Schema.TaggedErrorClass` for expected failures:

```text
MulletUnauthorized
MulletForbidden
MulletInvalidScenario
MulletScenarioNotFound
MulletRunNotFound
MulletSimulationFailed
MulletStorageError
MulletImportError
MulletExportRedactionError
MulletUnsupportedAssumption
```

Map those errors once in `workers/api/src/mullet/routes.ts`.

### Runtime primitives

Use `Clock`, `Effect.uuid`, injected repositories, and typed config services.
Do not use raw `Date.now()`, `new Date()`, `Math.random()`, raw `JSON.parse`,
direct D1 access in domain modules, or new `Effect.runPromise` bridges outside
the established Worker boundary.

### JSON and persistence

Use Schema decoders for stored scenario payloads and import files. Do not store
unbounded raw uploaded CSV/JSON blobs as the authoritative model. Keep original
import refs if needed, but persist normalized, decoded records with source and
provenance metadata.

## Persistence

Use D1 for saved private scenarios and runs.

Initial tables:

```text
mullet_scenarios
mullet_simulation_runs
mullet_run_hourly_results
mullet_run_candidate_modes
mullet_run_exports
```

Optional later tables:

```text
mullet_baseline_imports
mullet_market_memory
mullet_energy_telemetry_refs
mullet_proof_packet_refs
```

Scenario and run rows should include:

- owner user id;
- owner email;
- created and updated timestamps from Effect time services;
- private visibility only;
- schema version;
- source authority refs;
- provenance summary;
- redaction status for exports.

Do not store secrets, wallet material, raw prompts, raw customer artifacts,
private repo contents, payment preimages, or unbounded logs in mullet tables.

## Browser/Foldkit Plan

Add a `MulletRoute` at `/mullet`, include it in `LoggedInRoute` and
`AppRoute`, and gate it through `routeAllowedForLoggedInAuth`.

The browser model should use tagged states:

```text
MulletRouteState =
  idle
  | loading
  | loaded
  | running
  | failed
```

Separate child messages should describe facts:

```text
RequestedLoadMulletBootstrap
LoadedMulletBootstrap
FailedLoadMulletBootstrap
UpdatedMulletScenarioInput
RequestedRunMulletScenario
LoadedMulletRun
FailedRunMulletScenario
RequestedExportMulletRun
LoadedMulletExport
FailedExportMulletRun
```

The route should use existing admin/workroom layout primitives and generated
icons only. Do not build a public landing page. This is an operator console.

## Initial UI Shape

The first UI can be practical and compact:

- header: scenario name, privacy state, run status, last run time;
- left rail: scenario templates and saved scenarios;
- assumptions panel: facility, mining fleet, hardware, work class, provider
  floor, party split, capital assumptions;
- dispatch panel: hourly mode table with mode, revenue/MWh, margin/MWh,
  accepted outcomes/MWh, reason code;
- party returns panel: OpenAgents, provider, facility operator, hardware owner;
- sensitivity panel: acceptance rate, demand fill, electricity price,
  hashprice, raw GPU rate, review cost, provider minimum bid;
- proof/provenance panel: modeled values, measured refs, accepted refs,
  settlement refs, missing diligence;
- export action: private Markdown/JSON packet.

Empty states must be honest:

- no measured energy;
- no accepted-work demand;
- no settlement evidence;
- no Margot baseline import;
- no readiness proof;
- no payout proof.

## Test Plan

Minimum tests for implementation:

- browser route parses `/mullet`;
- logged-out visits to `/mullet` redirect through the normal auth path;
- non-admin logged-in users cannot access `/mullet`;
- wrong admin email cannot access `/mullet`;
- configured Christopher email can access `/mullet`;
- `/api/mullet/*` returns 401 without session;
- `/api/mullet/*` returns 403 for non-allowed email;
- scenario Schema rejects unit mistakes and missing provenance;
- pure sim reproduces frozen Margot baseline fixtures within tolerance;
- pure sim reproduces `MODEL1.md` work-class cost fixtures;
- dispatch chooses mining when accepted-work demand is zero;
- dispatch chooses accepted work only when backlog, eligibility, margin, and
  provider floors clear;
- export redaction rejects private artifacts, wallet material, raw logs, and
  customer data;
- architecture guardrail does not gain new `Effect.runPromise`, raw Env,
  direct D1, raw JSON, raw time, or service/domain HTTP response exceptions.

## Suggested Issue Roadmap

### Issue 1: Lock source authority and runtime invariant

Create the implementation source-authority doc, add a mullet invariant section,
and decide the exact allowed email. This issue should resolve the
`chris@openaegnts.com` versus `chris@openagents.com` discrepancy before any
route is reachable.

Acceptance:

- `INVARIANTS.md` says mullet records are private simulation evidence only.
- The exact allowed email is documented and tested.
- No route or API exists yet.

### Issue 2: Create shared mullet schema package

Add `packages/mullet-schema` with branded units, IDs, provenance records,
scenario records, work-class records, dispatch records, proof packets, market
memory, and energy telemetry refs.

Acceptance:

- Schema decode/encode tests cover a full Tinybox scenario.
- Unit-brand tests prevent kWh/MWh and dollar/hour/dollar/outcome confusion.
- No Cloudflare or DOM imports exist in the package.

### Issue 3: Build pure simulation core

Add `packages/mullet-sim` with pure calculation modules for facility, mining,
hardware, work-class economics, provider floors, dispatch, and capital returns.

Acceptance:

- Frozen fixture from `MODEL1.md` reproduces documented cost per accepted
  outcome and recommended price values.
- Frozen Margot-style fixture reproduces current mining/raw-AI KPIs within
  explicit tolerance.
- Dispatch reason codes are exhaustive.

### Issue 4: Add private D1 persistence

Add D1 migrations and Worker repository services for scenarios and runs.

Acceptance:

- Repository methods are Effect services with tagged errors.
- Stored JSON is decoded through Schema.
- No raw private artifacts, wallet material, or raw logs are stored.

### Issue 5: Add `/api/mullet/*` routes

Add Worker routes for bootstrap, scenario CRUD, simulation run creation, run
lookup, and private export lookup.

Acceptance:

- 401/403/200 tests mirror the admin overview route pattern.
- HTTP mapping stays in route modules.
- No domain service returns `Response`.
- No new zero-tech-debt architecture budget increases.

### Issue 6: Add browser `/mullet` route shell

Add Foldkit route, startup gate, initial commands, model, messages, update, and
view shell.

Acceptance:

- Route parsing test covers `/mullet`.
- Startup tests cover logged-out, non-admin, and allowed-admin behavior.
- Route appears only for the allowed operator.
- UI is dense operator tooling, not a public landing page.

### Issue 7: Build scenario editor and templates

Add editable scenario templates for Tinybox, SHC, miner GPU island, and 100 MW
80/20 facility.

Acceptance:

- Every editable assumption has provenance.
- Inputs reject unsupported unit combinations.
- Empty states show missing measured energy, demand, proof, and settlement.

### Issue 8: Render dispatch and returns

Add hourly candidate-mode table, selected dispatch table, party return panels,
and sensitivity output.

Acceptance:

- Displays mine, raw GPU, token/API, accepted work, curtail, idle/reserve.
- Shows accepted outcomes per kWh/MWh and dollars per MWh.
- Shows party split without double-counting buyer revenue.
- Shows decision-flip assumptions.

### Issue 9: Add private exports

Generate Markdown and JSON internal packets from scenario runs.

Acceptance:

- Export labels modeled, measured, accepted, paid, and settled separately.
- Export redaction test rejects private and secret-shaped data.
- Export does not become public claim projection.

### Issue 10: Add proof, telemetry, and market-memory refs

Connect modeled runs to existing accepted-work, energy, settlement, and market
memory refs when available.

Acceptance:

- Proof packet refs can be attached but not fabricated.
- Market-memory updates are modeled separately from runtime truth.
- Energy telemetry distinguishes measured data from counterfactuals.

### Issue 11: Add deploy smoke and operator runbook

Add a runbook for using `/mullet` and a deploy-safe smoke for the private route.

Acceptance:

- Smoke verifies denied access for non-admin and allowed access for the
  configured Christopher account.
- Runbook explains how to create a Tinybox scenario, run it, and export a
  packet.
- `bun run check:deploy` remains the deploy gate.

## Open Decisions

1. Decide whether `packages/mullet-sim` should run only on the Worker or also
   locally in the browser for draft previews.
2. Decide which Margot baseline fixtures should be frozen first.
3. Decide whether imports should accept CSV in v1 or only normalized JSON.
4. Decide whether `/mullet/runs/:runId` should exist in v1 or whether `/mullet`
   should be the only browser route until run sharing is needed.
5. Decide which fields, if any, can become public-safe investor export material.

## First Implementation Slice

The narrowest useful slice is:

```text
/mullet route
admin-only access
Tinybox scenario template
pure accepted-outcome + provider-floor calculation
no D1 persistence
no live imports
no exports
no dispatch side effects
```

The more durable first slice is:

```text
schema package
pure sim package
D1 scenario/run persistence
private API
Foldkit route shell
Tinybox and 100 MW fixtures
private Markdown export
```

The durable slice is more work, but it avoids a throwaway calculator and gives
OpenAgents product surface a real foundation for the unified runner Christopher wants.
