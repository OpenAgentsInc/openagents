# Prospective C/D validation of the common scheduled PD law

This is a new fixed-context validation, not another gain search. The C/D descriptor was sealed before the earlier original-case brake result. All common gains, rate-error schedule coefficients, F64 grouping, 500 Hz step, reference limits, goals and parity budgets remain fixed. The old Part A nine-pair study still has no feasible winner.

Context IDs are `pose*4 + scenario`, with poses C/D as 0/1 and scenarios hold/nominal/pulse/brake as 0/1/2/3. C brake starts joint 7 at +2 rad/s; D starts at −2. The scenario-2 external joint-2 pulse is C +3 and D −3 J/rad at ticks 200..249. The new Coax wrapper replaces D's pulse before publishing or applying the command. There is no legacy brake damping override.

`experiment.json`, `factory-cases.f64` and the analytic reference receipt bind all eight reset/gain/edge/finger/pulse factories before effects. Reference generation is independent tooling, not the Coax plant. `reference-samples.bin` preserves all 184-byte typed-reference coefficients and reference tick/plan length; `reference-states.f64` holds the analytic 21-field frames. The pure example queries factory/reference metadata only.

The new Coax modules live in `tests/robotics/`; they are an experimental consumer, not a general controller API. They expose the 144B initial state, 744B command, 976B step and 184B reference. The command snapshots all nine raw positions/rates. Shapes precede context/tick admission. Existing stage and underlying status are preserved; refusals leave complete caller outputs unchanged. Pa actuator predictions are diagnostics and are not added a second time to the plant.

Scoring initializes all nine peak speeds, previous-rate lineage and source margins from each actual C/D reset. Margins include the initial and every admitted post-state. Control uses reference k; the post-state score uses reference k+1. Metric cards carry pose IDs 0/1 in this NEW namespace and scenario IDs 0..3; pose labels are not gain candidates. The ordered 16B two-pose verdict ANDs valid/feasible observations. Parametric helpers and metric PODs are unsealed and are not trajectory or authority certificates.

All four scenarios of each pose must satisfy position error ≤0.01 rad and rate error ≤0.05 rad/s for every post-state 626..750. Hold also requires peak drift ≤0.005 rad over the full run. Nominal SSE must strictly beat the analytic stationary comparator. Goals are separate from numerical parity; a behavior miss is retained rather than relaxed.

## Qualification

Sixteen new source groups, with 263 actual emitted root mutants killed and zero survivors: control131, score45, public87. Two new E0330 unit witnesses reject angle-as-rate and rate-as-acceleration. Initial observational and compile-only failures remain in the source history. The source proof includes one isolated successful D-negative-pulse step, separate from canonical native experiment counts. Older scalar, controller and plant source gates are carried unchanged.

The native experiment is restricted to one SDK 6,000-step capture and one own fresh-feedback 6,000-step run, eight contexts×750 each, with no shadow plant, scalar62 replay, rendering or NN. Further host-mode checks consume cached observations. The first canonical runs and independent audit passed all eight strict goals in both SDK and own datasets. The existing capture/native receipts remain byte-exact; cached host modes consume observations without repeated physics.

This fixed unconstrained profile does not establish contacts, collision safety, hardware behavior, general stability, generic importing or compiler-enforced bounded control. Mutation counts cover actual emitted sites, not every Quantity operator token. Source-visible helpers require their declared fixed-context, finite-state and coherent-reference preconditions.

## Measured fixed-context results

The first canonical SDK capture and own fresh-feedback run each completed exactly 6,000 steps (eight resets × 750), once each. There is no shadow plant. Every fixed case is valid and feasible in both datasets; the ordered two-pose verdict is `[1,1]`. The terminal ratio is the maximum of `abs(q_error)/0.01` and `abs(rate_error)/0.05` over the frozen final 125 post-states; a ratio ≤1 passes.

| Pose | Scenario | SDK terminal ratio | Own terminal ratio | Controller-clipped coordinates | Goals |
| --- | --- | ---: | ---: | ---: | --- |
| C | hold | 0.0000868377 | 0.0000868377 | 0 | pass in both |
| C | nominal | 0.7936766163 | 0.7936766163 | 0 | pass in both |
| C | pulse | 0.7798300623 | 0.7798300623 | 0 | pass in both |
| C | brake | 0.2356208390 | 0.2356208390 | 34 | pass in both |
| D | hold | 0.0000424217 | 0.0000424217 | 0 | pass in both |
| D | nominal | 0.9710715211 | 0.9710715211 | 0 | pass in both |
| D | pulse | 0.9483775273 | 0.9483775273 | 0 | pass in both |
| D | brake | 0.2478364627 | 0.2478364627 | 35 | pass in both |

The closest case is D nominal: SDK ratio 0.9710715211, leaving 0.0289284789 normalized margin. This is a measured pass at two fixed poses and a fixed horizon, not a global stability certificate. All actuator-stage control/force clipping counters are zero; the brake has 34 controller-clipped coordinates on C and 35 on D. The nominal stationary-SSE comparator and hold-drift goals pass separately. No acceptance goal was changed after measurement.

Three cached host profiles agree byte for byte without repeating the plant runs. Status, clipping, reference and pulse metadata match exactly; all 12,000 schedule records match the independent law and there are zero cross-stream regime differences. SDK-versus-own differences are at most 4.441e−16 rad in arm position, 4.111e−15 rad/s in arm velocity and 6.040e−14 J/rad in control. The independent audit observes 210,000 SDK PD fields (maximum error 4.619e−14) and 54,000 actuator fields (numerically exact). Case metrics differ by at most 6.040e−14 within the frozen mixed budget; 27 trace and 11 score reader poisons are refused.

The C/D brake peak arm effective accelerations are 41.6233 and 41.0303 rad/s², measured as state `Δv/0.002`, not SDK `qacc`. Reference rate/acceleration caps do not bound those actual plant quantities. A terminal-window pass does not assert permanent settling, collision protection, or hardware behavior.

The focused gate is `check-panda-scheduled-pd-validation.sh`; it uses committed cached traces and has no SDK or private-asset dependency. The additive adapter is syntax/plan/stub qualified here; its numerical wrapper path is not rerun. Its underlying measured capture/native/audit results are the carried evidence.
