# Forward-Power And Interconnection Scenarios

Date: 2026-06-06

Status: implemented for issue #366 / `OPENAGENTS-LATE-006`.

## Purpose

Forward-power and interconnection scenarios let operators model already
purchased or otherwise available unused power, candidate flexible workloads,
avoided upgrade costs, avoided delays, proof-of-response history, and caveats.

The implementation lives in
`workers/api/src/pylon-forward-power-scenarios.ts`.

This is a diligence and projection contract. It is not financial advice, power
trading, grid-program participation, interconnection mutation, capacity
dispatch, or settlement authority.

## Scenario Shape

`PylonForwardPowerScenarioRecord` records:

- scenario kind;
- claim state;
- facility ref;
- unused power watt-hours;
- workload fit basis points;
- avoided upgrade cost;
- avoided delay days;
- avoided delay cost;
- assumption refs;
- caveat refs;
- workload fit refs;
- interconnection refs;
- proof-of-response refs;
- contract refs;
- settlement refs;
- evidence and source refs;
- read-only authority.

Scenario kind can be:

- `forward_power_window`;
- `interconnection_value`.

Claim state can be:

- `modeled`;
- `measured`;
- `contracted`;
- `settled`.

## Derived Projection

`projectPylonForwardPowerScenario` derives:

- unused power MWh;
- workload fit percent;
- avoided cost total;
- avoided upgrade value claim flag;
- avoided delay value claim flag;
- measured power claim flag;
- settlement claim flag;
- friendly display times;
- public/team/operator redaction.

## Evidence Rules

The contract rejects overclaims:

- every scenario requires assumption refs and caveat refs;
- avoided upgrade or delay value requires interconnection refs;
- measured and settled states require proof-of-response refs;
- contracted and settled states require contract refs;
- settled states require settlement refs;
- workload fit cannot exceed 100%;
- all value fields must be non-negative integers.

## Authority Boundary

Scenario records cannot:

- dispatch capacity;
- give financial advice;
- participate in grid programs;
- mutate interconnection state;
- trade power;
- upgrade public claims;
- mutate settlement.

## Investor Diligence

Before using a scenario in investor material, an operator should confirm:

- the power inventory source;
- the workload-fit assumptions;
- interconnection queue, tariff, or facility assumptions;
- proof-of-response history;
- whether avoided costs are modeled, measured, contracted, or settled;
- what evidence remains missing.

## Tests

`workers/api/src/pylon-forward-power-scenarios.test.ts` covers:

- modeled scenario projection;
- modeled, measured, contracted, and settled claim separation;
- unused power MWh, workload fit percent, and avoided-cost derivation;
- public redaction of private facility, contract, interconnection, proof, and
  settlement refs;
- assumption, caveat, interconnection, proof, contract, settlement, and
  workload-fit validation;
- rejection of financial advice, trading, grid participation, interconnection
  mutation, settlement mutation, public-claim upgrade, and dispatch authority;
- rejection of private power, contract, payment, trading, provider, raw
  telemetry, and raw timestamp refs.
