# Artanis/Pylon Comparative Economics Evidence Packets

Date: 2026-06-06

Issue: #415 / `ARTANIS-029`

Status: implemented as a read-only packet contract and projection.

## Purpose

Artanis needs a dedicated evidence packet for comparing Pylon capacity against
Bitcoin mining, GPU rental, token inference, node/system-power-adjusted
economics, and accepted outcomes in common per-MWh units.

This closes the gap left by the Margot export ingestion contract. Margot can
ingest simulator exports, but Artanis also needs a structured packet that joins
the simulator provenance to GPU rental samples, OpenRouter and ML.Energy token
inputs, Pylon node power, ERCOT/NYISO windows, mining counterfactuals, and
accepted-work evidence.

## Implementation

Code lives in:

- `workers/api/src/artanis-pylon-comparative-economics.ts`
- `workers/api/src/artanis-pylon-comparative-economics.test.ts`

The packet is a contract and projection only. It does not run Margot, mutate
markets, dispatch Pylon work, spend bitcoin, charge buyers, settle providers,
or upgrade public claims.

## Packet Contents

Each packet records:

- Margot provenance: repo ref, commit ref, normalized export refs, data
  timestamp refs, source URL refs, and caveats;
- GPU rental evidence: Vast.ai-style sample timestamp, GPU model, sample size,
  dollars per GPU-hour, TDP/source, and derived dollars per MWh;
- token economics evidence: OpenRouter model price timestamp, raw price unit,
  display unit, ML.Energy run/task/GPU/J-token/tokens-sec/stability fields,
  and derived dollars per MWh;
- throughput-calculator evidence as modeled-only refs for calculator URL,
  query params, model, hardware, context, quantization, and caveats;
- Pylon capacity evidence: node/cohort, GPU count/model, VRAM, interconnect,
  runtime/framework, resource mode, system power, chip TDP, effective watts per
  GPU, PDU/IPMI/meter availability, PUE assumption, availability window, cost
  term, and idle/dark-capacity refs;
- ERCOT/NYISO or unsupported power-market windows with LMP, zone/settlement
  point, refresh refs, missing-data flags, sources, and caveats;
- mining counterfactuals with ASIC model, efficiency, capacity, revenue per
  MWh, margin per MWh, pool/firmware/ops assumptions, and curtailment policy;
- accepted-work evidence with assignment, run, artifact, grading, acceptance,
  rejection, retry, closeout, economics, payable, settled, and settlement refs.

Claim states stay separate:

- `modeled`
- `measured`
- `payable`
- `settled`
- `blocked`
- `stale`
- `unsupported`

## Public Projection

The public projection can show:

- mining floor dollars per MWh;
- GPU rental floor dollars per MWh;
- token inference floor dollars per MWh only after unit audit verification;
- node-power-adjusted floor dollars per MWh only when the denominator is not
  chip TDP;
- accepted-outcome value dollars per MWh;
- power cost dollars per MWh;
- idle/dark-capacity refs;
- denominator refs;
- public blocker refs;
- public caveats and source refs.

The public projection redacts private repo access, raw meter/provider/customer/
workroom/wallet/log material, private URLs, raw command output, secrets, and
private evidence refs.

## Operator Projection

Operator and private projections retain safe private refs by reference. They
still reject secrets, raw wallet/payment material, raw meter telemetry, private
repo URLs, raw timestamps in refs, and provider secret material.

The operator projection is for diligence and review. It is not authority to
dispatch work, spend bitcoin, settle providers, or publish stronger claims.

## Unit And Denominator Rules

Token $/MWh rows cannot appear on public, agent, or customer projections until
the packet marks `unitAuditState: verified`. This protects the OpenRouter
raw-price unit, display unit, ML.Energy J/token unit, and sample arithmetic.

Chip-TDP, node-system-power, measured PDU/IPMI, metered facility, and
PUE-adjusted denominators remain distinct. If the Pylon capacity denominator is
`chip_tdp`, the node-power-adjusted floor is projected as null and the packet
adds a public blocker instead of pretending chip-only energy is facility or
node energy.

Unsupported markets such as PJM require an explicit unsupported-market caveat
ref before projection.

## Accepted Work Boundary

Accepted outcome evidence keeps payable and settled states separate.

- `payable` requires provider payable value and cannot carry settled provider
  value.
- `settled` requires provider settled value and settlement refs.
- provider settled value cannot exceed provider payable value.

The packet can retain settlement refs, but it cannot create settlement,
dispatch payouts, charge buyers, or mutate accepted-work records.

## Verification

Focused tests cover:

- valid modeled ERCOT and NYISO packets;
- public redaction and operator private refs;
- token unit-audit blocking and verified token projection;
- unsupported-market caveats;
- stale source blockers;
- chip-TDP denominator blocking;
- accepted-work payable versus settled separation;
- unsafe refs, private URLs, raw material, mutable authority, and invalid
  numbers.

## Current Claim

Implemented #415 makes it safe for Artanis to collect comparative-economics
packets. It does not prove live outcomes-per-kWh economics yet. Measured and
settled public claims still require real Pylon node telemetry, accepted-work
receipts, payment/payout evidence, and settlement receipt chains.
