# Margot Export Ingestion Contract

Status: implemented for issue #367 / `OPENAGENTS-LATE-007`.

## Purpose

Margot-style facility simulator exports are now modeled as read-only Omni
packets that can be joined to power, capacity, accepted-work, and investor
proof surfaces without letting the simulator mutate OpenAgents product surface state.

The contract is designed for the current flexible-compute economics loop:

- mining floor per MWh;
- GPU rental floor per MWh;
- token inference floor per MWh;
- node/system-power-adjusted AI floor per MWh;
- OpenAgents accepted-work assumption value per MWh;
- grid-service and curtailment value assumptions per MWh;
- power cost per MWh;
- market and dispatch-policy labels;
- provenance, source, caveat, data-rights, scenario, diligence, and settlement
  refs; and
- public, customer, agent, team, operator, and private projections.

The implementation lives in:

- `workers/api/src/margot-export-ingestion.ts`
- `workers/api/src/margot-export-ingestion.test.ts`

## Model Boundaries

The ingestion contract is intentionally not a trading, grid-program, wallet,
or settlement authority.

Every packet carries a read-only authority record that must preserve all of
these false-authority boundaries:

- no accepted-work mutation;
- no financial advice;
- no grid participation;
- no live wallet spend;
- no market-data mutation;
- no public claim upgrade; and
- no settlement mutation.

If any of those boundaries are relaxed in a packet, projection throws
`MargotExportPacketUnsafe`.

## Accepted Markets

The first supported electricity-market labels are:

- `ercot`
- `nyiso`

The `unsupported` market label is allowed only when the packet carries an
explicit unsupported-market caveat ref. This reflects the current Margot
synthesis: ERCOT and NYISO are usable source lanes today; other markets need
operator review before they can be treated as supported simulator inputs.

## Claim States

The packet preserves three claim states:

- `modeled`: simulator output only, useful as a first comparator;
- `measured`: supported by measurement/provenance refs and allowed to make an
  accepted-work lane claim when accepted-work assumptions are present; and
- `settled`: allowed only when public-safe settlement receipt refs are present.

Even when the claim state is `settled`, the packet still has no settlement
mutation authority. It can preserve receipt refs; it cannot create or update
receipts.

## Required Refs

A valid packet must include:

- caveat refs;
- provenance refs;
- source refs; and
- next-diligence refs.

If `acceptedWorkCentsPerMwh` is greater than zero, the packet must also include
accepted-outcome assumption refs. If the claim state is `settled`, it must
include settlement receipt refs.

This keeps investor and operator surfaces from displaying value estimates
without visible caveats and follow-up work.

## Safety Filters

The projection rejects or redacts material that does not belong in an Omni
simulator packet:

- private customer or provider material;
- wallet, invoice, payment, preimage, payout, or MDK secret-shaped refs;
- trading-account or trading-order refs;
- raw export, market, power, meter, telemetry, provider, runner, prompt, or
  webhook material;
- private repository URLs;
- access tokens, OAuth material, cookies, or `sk-*` keys; and
- raw ISO timestamps inside refs.

Audience projections redact private assumption, caveat, data-rights,
diligence, provenance, scenario, settlement, and source refs before public,
agent, customer, or team surfaces can show them.

## Why This Matters

Margot's current simulator compares mining, GPU-rental, token-inference, and
node-power economics in common per-MWh units. The important caveat is that GPU
chip TDP is not full-facility power. Real facility power includes the server,
CPU, memory, storage, networking, fans, cooling, PSU/PDU losses, and site
overhead.

This contract lets OpenAgents product surface ingest those first comparator values while preserving
that caveat. It also leaves room for future accepted-outcome revenue,
grid-service value, measured SHC/Pylon telemetry, and settlement receipts to
appear as separate evidence-backed lanes rather than being collapsed into one
overconfident number.

## Follow-Up Data Collection

GitHub issue
[#415](https://github.com/OpenAgentsInc/openagents/issues/415)
implemented the Artanis/Pylon comparative-economics evidence packet in
`workers/api/src/artanis-pylon-comparative-economics.ts`. That packet is the
next contract after Margot ingestion for collecting all data needed to
substantiate the outcomes-per-kWh thesis:

- Margot repo commit, export refs, data timestamps, and source caveats;
- Vast.ai GPU rental sample metadata, TDP/source labels, and derived dollars
  per MWh;
- OpenRouter price units, ML.Energy benchmark rows, token-throughput
  calculator refs, and token dollars-per-MWh unit audits;
- Pylon node/cohort system power, measured-meter availability, resource mode,
  runtime/framework, and cost terms;
- ERCOT/NYISO LMP windows and unsupported-market caveats;
- mining counterfactuals for the same windows; and
- accepted-work, payable, and settlement refs when real work runs.

Margot export packets remain modeled comparator inputs by themselves. The #415
packet can join those inputs to Pylon capacity, token-unit audits, power-market
windows, mining counterfactuals, and accepted-work evidence. It still does not
make definitive outcomes-per-kWh proof until the packet carries measured node
power, accepted-work receipts, payout/payment evidence, and settlement refs
with the corresponding claim states.

## Tests

Coverage includes:

- valid modeled ERCOT packet projection;
- measured and settled claim-state separation;
- unsupported-market caveat enforcement;
- public redaction of private refs;
- required caveat/provenance/source/diligence/assumption/settlement refs;
- false-authority rejection; and
- unsafe ref and invalid per-MWh value rejection.
