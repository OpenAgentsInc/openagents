# Flexible-load operator proof report (MODELED) — 2026-06-19

Weekend promise assault, DE-10. This is the **modeled operator proof report** the
`energy.flexible_load_proof.v1` verification gate names: a comparison of accepted
outcomes, mining, grid service, AI-load smoothing, forward-purchased power
capture, curtailment, reserve, and idle states with explicit evidence labels.

It does NOT flip the promise. `energy.flexible_load_proof.v1` STAYS **planned**.
Every dollar figure here is **MODELED** (an operational estimate), not measured
operator telemetry, and is labeled as such per the unsafeCopy boundary. Per the
hard rule, a green flip needs measured-or-explicitly-modeled operator proof with
real flexible-load event history + owner sign-off.

> Energy models are operational estimates, not investment, grid, utility, or
> financial advice (authorityBoundary).

## Method and provenance

The state economics below are **modeled** from two studied references (read-only,
not vendored): the facility-revenue simulator
`projects/repos/oa_aibtc_model` (ERCOT/NYISO hourly LMP, ASIC economics,
curtailment-policy engine, GPU/token AI-revenue model from ML.Energy J/token and
OpenRouter prices) and the ERCOT API surface `projects/ercot`. The only
**receipt-backed** quantity OpenAgents itself currently holds is the
accepted-outcome numerator from the first settled labor job (#4777), surfaced as
a yellow, caveated metric at `/api/public/metrics/accepted-outcomes-per-kwh`
(`metrics.accepted_outcomes_per_kwh.v1`).

Evidence-state labels used per row:

- **receipt_backed** — derived from an OpenAgents settled receipt.
- **modeled** — an operational estimate from the studied references / public
  market data; NOT measured OpenAgents telemetry.

## State comparison (dollars per MWh consumed/avoided)

A single ~1 MW flexible facility; ERCOT-style hourly LMP regime; figures are
per-MWh and **modeled** unless marked otherwise. They are illustrative of the
*ordering and structure* of states, not a measured operator result.

| State | What the load does | Modeled $/MWh basis | Evidence state |
|---|---|---|---|
| Accepted outcomes | Run verified accepted-work units (the OpenAgents product) | Revenue per accepted outcome ÷ energy per outcome; seed AO/kWh ≈ 0.033 from #4777 (modeled 100 W, ~30 min) | receipt_backed (numerator) + modeled (energy) |
| AI inference compute | Serve gateway/open-model inference | sell = provider/serving cost × margin; $/MWh from ML.Energy J/token × token price | modeled |
| Bitcoin mining | Hash at nameplate when power is cheap | block-by-block reward × hashprice − energy cost; floor revenue per MWh | modeled |
| Grid service (ancillary/DR) | Offer curtailable load for response/reserve | capacity payment + energy back-off credit during DR events | modeled |
| AI-load smoothing | Shift compute off price peaks into troughs | avoided peak $/MWh − trough $/MWh, net of deferral cost | modeled |
| Forward-purchased power capture | Consume against a forward-bought block | spot − forward strike, captured when spot > strike | modeled |
| Curtailment | Reduce/stop load on a price/grid signal | avoided energy cost at the curtailment-hour LMP (a SAVING, not revenue) | modeled |
| Reserve (spinning/standby) | Hold capacity ready to ramp | standby capacity payment − holding cost | modeled |
| Idle | No productive load | $0 revenue; only fixed/standby cost — the baseline floor | modeled (floor) |

Structural ordering the model encodes (not a measured claim): accepted outcomes
and AI inference are the highest-value uses of a kWh when demand exists; mining is
the floor that monetizes otherwise-idle cheap power; grid service / curtailment /
reserve convert *flexibility itself* into value during high-price or grid-stress
hours; idle is the baseline the others must beat. The flexible-load thesis is
that intelligently switching among these states beats any single static state.

## Flexible-load event history (status)

- **Receipt-backed events:** 1 accepted-outcome datapoint (#4777), energy
  modeled. No real curtailment/grid/forward-capture events with OpenAgents
  telemetry exist yet.
- **Planned real-event source:** the training marathon's scheduled curtailment
  drill (`training.marathon_operations.v1`) — shed part of the fleet on schedule,
  resume from sealed checkpoints, publish the receipt. That receipt is the
  intended first real flexible-load event for this report.

## What this changes

- `energy.flexible_load_proof.v1` — **planned → planned.** This supplies the
  modeled operator proof report comparing all enumerated states with evidence
  labels (one of the named blockers), explicitly modeled. Still missing for
  green: energy-market ingestion wired to a live source, work-class flex profiles
  from real telemetry, and a real flexible-load event history (the curtailment
  drill receipt). No state flip; receipt-first + owner sign-off remain required.
- `metrics.accepted_outcomes_per_kwh.v1` — unchanged (yellow); referenced here as
  the one receipt-backed quantity feeding this report.
