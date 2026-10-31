# State and force sensitivities of the owned RK4 plant

The [runnable example](../../examples/own-plant-sensitivities.coax) connects
chapter 107’s function transforms to chapter 109’s smooth dynamics. Product
arithmetic and differentiation are Coax. The [frozen experiment](../../fixtures/physics/oscillator-functions/experiment.json)
and [qualification](../../fixtures/physics/oscillator-functions/qualification.json)
keep formal calculus and physical-model approximation separate.

The plant state is position/velocity. The graph temporarily augments it with
force: `z=[q,v,u]`, with derivative row `u'=0`, so one action is held across all
four RK stages. The 3×3 field matrix has rows `[0,1,0]`, `[-k,-c,1]`, `[0,0,0]`.
Mass is fixed at 1. This algebraic F32 realization uses the same RK4 method but
has different staging/association from the authoritative F64 `os_step`.

`of_pack` admits starting parameters/state/force/dt and target/weights. It
returns 22 F32 inputs: q/v/u; nine matrix entries; half/full/sixth-step and 2;
three negative target coordinates; and three weights. Starting values retain
the chapter 109 bounds. Weights are finite in [0,4], with at least one positive.
Numeric coordinates use reference scales 1m, 1m/s and 1N; cost weights are
dimensionless. These are declared conventions, not compiler unit/frame proof.

| `of_function` mode | Exact input span | Returned components |
| --- | --- | --- |
| `OF_STEP` | first 16 packed values | next q/v and carried force |
| `OF_JACOBIAN` | first 16 | 3×3 partial matrix for q/v/u only |
| `OF_COST` | all 22 | weighted squared q-target/v-target/force cost |
| `OF_GRADIENT` | all 22 | three q/v/u partials of that cost |
| `OF_HESSIAN` | all 22 | 3×3 second partials for q/v/u |

The graph uses only permanent-IR matmul/add/multiply/sum. Partial descriptors
select the three varying coordinates; generic 107 machinery computes every
partial/product. Matrix, time, target and weights are fixed context. The
separately packed half/full/sixth constants are correlated by `of_pack`;
free derivatives of those packed inputs are not physical dt/parameter gradients.
Chapter 111’s F64 tape supplies the separately qualified parameter-recovery path.

The raw Fx step is a planning/derivative model. Actual execution still calls
`os_step`, including its next-state admission. The qualified cases stay inside
those bounds. Refusal, clipping, contact and other discontinuities are outside
this derivative profile. Formal F64 derivatives at stored F32 primals do not
differentiate literal float rounding. No pretrained GR00T embodiment or own
robot backend is qualified by this scalar experiment.

The independent oracle derives the RK4 transition as a matrix polynomial,
then derives `J=M`, `g=2*Mᵀ*W*e` and `H=2*Mᵀ*W*M`. It independently emulates
F32 forward stages, uses original F64 physical parameters for physical checks,
and compares direct force finite differences. 45 points and 18 ordered batches
cover shared context with mapped q/v/u, poisoning all ignored batch slots.
The native transcript has 180 exact F32 primal values, 1,485 formal comparisons,
540 exact batch values and 180 force finite-difference comparisons. 202/202 root
mutants die. Each native host mode passes 2,592 assertions with identical payloads.
The mathematical oracle runs once; later host modes reuse it only after exact
byte comparison. The receipt states generated-Coax and native-host modes separately.

```shell
coax/bin/coax run psionic/examples/own-plant-sensitivities.coax
coax/bin/coax run psionic/examples/own-plant-sensitivities.coax --release
psionic/check.sh --only physics/oscillator-functions
```
