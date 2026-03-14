# Risk Market

This is the canonical status doc for the `Risk Market`.

For deeper background on prediction, coverage, and underwriting mechanics, also
see [../prediction-markets.md](../prediction-markets.md).

## Purpose

The Risk Market prices uncertainty, verification difficulty, and liability.

Kernel-facing objects:

- `CoverageOffer`
- `CoverageBinding`
- `PredictionPosition`
- `RiskClaim`
- `RiskSignal`

In practice, this market covers:

- prediction positions
- coverage and underwriting capacity
- claims and claim resolution
- policy-bearing market signals

## Current repo verdict

| Dimension | Status | Notes |
| --- | --- | --- |
| Product surface | not productized | there is no dedicated end-user risk-market lane in the desktop app today |
| Kernel authority | `implemented` starter slice | coverage, prediction, claims, and signals are live in kernel authority |
| Wire/proto | not yet dedicated | there is no checked-in `openagents.risk.v1` package yet; the broader plan points at `aegis` and policy packages |
| Local prototype | `implemented` | richer incidents, premiums, calibration, and policy integration still live partly in docs and desktop-local modeling |
| Planned | yes | underwriter accounts, broader market depth, claim payout productization, and fuller live policy integration remain planned |

## Implemented now

- place a `CoverageOffer`
- bind coverage into a `CoverageBinding`
- create a `PredictionPosition`
- create and resolve a `RiskClaim`
- publish a `RiskSignal`

These flows are implemented in:

- `crates/openagents-kernel-core/src/risk.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`

Authenticated HTTP mutation routes are live under:

- `POST /v1/kernel/risk/coverage_offers`
- `POST /v1/kernel/risk/coverage_bindings`
- `POST /v1/kernel/risk/positions`
- `POST /v1/kernel/risk/claims`
- `POST /v1/kernel/risk/claims/{claim_id}/resolve`
- `POST /v1/kernel/risk/signals`

The backend also already computes risk-facing snapshot metrics such as:

- open coverage offers
- active coverage bindings
- open prediction positions
- coverage concentration
- capital coverage ratio

## Local prototype or partial only

- broader incident and premium modeling
- richer calibration semantics and policy integration
- prediction and coverage as a deeper market information layer rather than only
  starter authority objects
- broader underwriting and risk-control framing in the kernel specs

## Not implemented yet

- underwriter accounts as a full end-user market lane
- broad market-depth and discovery UX
- a dedicated checked-in risk proto package or final public package mapping
- fuller live policy integration where market signals directly gate product
  behavior through a completed authoritative path
- product-facing claim payout and operator workflows beyond the starter slice

## Current repo truth lives in

- `crates/openagents-kernel-core/src/risk.rs`
- `crates/openagents-kernel-core/src/authority.rs`
- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`
- [../prediction-markets.md](../prediction-markets.md)
- [../economy-kernel.md](../economy-kernel.md)
- [../economy-kernel-proto.md](../economy-kernel-proto.md)

## Boundary notes

- risk prices uncertainty and liability; it does not replace verification
- risk can influence compute, labor, and liquidity policy, but it remains a
  separate market layer
- coverage markets provide bounded liability; prediction positions provide
  information signals
