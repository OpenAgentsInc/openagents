# Accepted Outcomes Per Power

Date: 2026-06-06

Status: implemented for issue #363.

## Purpose

Accepted outcomes per power gives Omni a read-only way to explain work
productivity against energy use without overstating grid, wallet, provider
settlement, or public proof claims.

The implementation lives in
`workers/api/src/omni-outcome-power-productivity.ts`.

## Contract

`OmniOutcomePowerProductivityRecord` records:

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
- model refs;
- measured meter refs;
- dark-capacity reason refs;
- settlement refs;
- source, caveat, and workroom refs;
- read-only authority flags.

`projectOmniOutcomePowerProductivity` groups records by work kind and returns a
total projection. It derives:

- energy kWh and MWh;
- dark-capacity MWh;
- accepted outcomes per kWh;
- accepted outcomes per MWh;
- accepted revenue per kWh;
- accepted gross profit per kWh;
- provider payable per kWh;
- measured and modeled energy claim flags;
- settlement claim flags for the current audience.

## Unit Rules

Records store energy as integer watt-hours. Projections display:

- kWh as watt-hours divided by 1,000;
- MWh as watt-hours divided by 1,000,000;
- per-kWh values only when the energy denominator is known and greater than
  zero.

Unknown energy produces `null` per-energy metrics. This prevents an accepted
outcome from appearing energy-efficient when no meter or model evidence exists.

## Claim Boundaries

The model separates:

- modeled energy;
- measured energy;
- accepted outcomes;
- provider payable value;
- provider settled value;
- settlement receipts.

Measured energy claims require visible measured-energy refs. Modeled energy
claims require model refs. Provider settlement claims require a settled state,
positive provider settled value, and settlement refs visible to the projection
audience.

Dark-capacity MWh is an accounting signal, not a settled grid-service value.
Dark-capacity energy requires dark-capacity reason refs and should be presented
with caveats when meter evidence is incomplete.

## Authority Boundary

The projection cannot:

- mutate energy meters;
- spend from a wallet;
- dispatch a payout;
- upgrade power-market claims;
- mutate provider settlement;
- upgrade public claims.

## Tests

`workers/api/src/omni-outcome-power-productivity.test.ts` covers:

- unit calculations for kWh and MWh;
- accepted outcomes per kWh and per MWh;
- accepted revenue, accepted gross profit, and provider payable per kWh;
- dark-capacity MWh;
- zero and unknown energy handling;
- public redaction of private meter, settlement, and workroom refs;
- modeled, measured, mixed, and not-settled claim labels;
- false authority, missing evidence, false settlement, dark-capacity overclaim,
  raw telemetry, wallet/payment, provider, and raw timestamp rejection.
