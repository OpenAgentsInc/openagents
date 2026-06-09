# Mullet Operator Runbook

This runbook is for the private `/mullet` unified simulation runner in OpenAgents product surface.
Only the confirmed admin account `chris@openagents.com` may use the browser
route or `/api/mullet/*` endpoints.

## Authority Boundary

Mullet is simulation-only. A scenario, run, dispatch result, export packet, or
attached ref does not authorize live work assignment, provider mutation, wallet
spend, Bitcoin settlement, payout closeout, public claim promotion, Forum
posting, or customer-visible projection.

Treat every value as one of these states:

- `modeled`: calculated or operator-entered assumption.
- `measured`: backed by telemetry or a facility/provider record.
- `verified`: independently checked evidence.
- `accepted`: accepted-work closeout proof exists.
- `paid`: payment proof exists.
- `settled`: settlement receipt exists.

Do not promote modeled output as fact. Do not infer proof refs from modeled
dispatch. Attach only refs that already exist.

## Create A Tinybox Scenario

1. Open `/mullet` as `chris@openagents.com`.
2. Select one of the Tinybox templates:
   - `Tinybox SHC power`
   - `Tinybox residential`
   - `Tinybox West Texas miner-site power`
3. Review the assumption groups:
   - facility
   - power
   - mining fleet
   - hardware
   - work class
   - provider floor
   - party split
   - capital
4. For every edited assumption, update the source label and provenance state.
5. Keep unproven values as `modeled` or `estimated`.

The browser workbench is private and local to the route state. Persisted
scenario creation uses:

```bash
POST /api/mullet/scenarios
```

with a `scenario` body conforming to `MulletScenario`.

## Run A Simulation

Create a run from a persisted scenario:

```bash
POST /api/mullet/runs
```

Minimal body:

```json
{
  "scenarioId": "mullet_scenario_id"
}
```

Optional typed attachments:

```json
{
  "scenarioId": "mullet_scenario_id",
  "proofPackets": [],
  "energyTelemetry": [],
  "marketMemory": [],
  "providerSettlementState": "not_payable",
  "powerDataState": "modeled"
}
```

Only attach proof packets, telemetry, market memory, or settlement refs that
already exist. The runner copies supplied refs into the run; it does not invent
them from modeled values.

## Interpret Dispatch

The dispatch table compares candidate modes:

- accepted work
- mining
- raw GPU market
- token/API inference
- curtailment
- idle

Read the selected mode together with the gates:

- readiness
- demand
- provider floor

Accepted-outcome metrics show outcomes/kWh, outcomes/MWh, revenue/MWh,
margin/MWh, provider payout/MWh, and breakeven accepted outcomes/day. Party
returns count buyer revenue once under OpenAgents and show all other rows as
payout shares from that same buyer revenue.

Market-memory rows are directional modeled memory. They are not runtime truth,
settlement evidence, or proof of future demand.

## Export A Private Packet

Generate a Markdown export:

```bash
POST /api/mullet/runs/<runId>/export
```

Body:

```json
{
  "format": "markdown"
}
```

Generate a JSON export:

```json
{
  "format": "json"
}
```

Exports are private by default and marked as not public claim projections. The
exporter labels modeled, measured, verified, accepted, paid, and settled values
separately, includes only attached proof/telemetry/settlement/market-memory
refs, and runs a redaction check before metadata persistence.

The redaction gate rejects raw prompts, raw traces, customer data, private
artifacts, private repo refs, wallet material, payment preimages, invoices,
provider secrets, raw logs, and raw timestamps.

Read latest export metadata:

```bash
GET /api/mullet/runs/<runId>/export
```

## Deploy Gate

Before deploying OpenAgents product surface, run:

```bash
bun run check:deploy
```

That gate includes the private browser route checks and the Mullet API smoke for
denied non-admin access plus allowed `chris@openagents.com` access.
