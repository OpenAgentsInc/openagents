# RB-02 scalar servo contract

`scalar-servo-play-f64-v1` is an illustrative CPU numerical profile. Its
coefficients are authored teaching inputs, not Microduck measurements, an
identified motor, firmware compatibility or a physical permission. The exact
Fraction oracle and its 56×17 reference table were frozen before the Coax
implementation. No upstream implementation or experiment output generates
the expected answers.

## Equations and operation order

All feedback uses motor-side position `q` and rate `v`. For the delayed
position target `u`, compute these separately, in this order:

```text
P = kp * (u - q)
D = 0 - kd * v
requested = P + D
limit = effort * voltage
delivered = clamp(requested, -limit, limit)
direction = +1 if v > 0, -1 if v < 0, 0 otherwise
passive = (0 - viscous * v) - coulomb * direction
net = delivered + passive
a = net / inertia
v_next = v + dt * a
q_next = q + dt * v_next
output_next = clamp(output_previous, q_next - play, q_next + play)
output_rate = (output_next - output_previous) / dt
```

The strict clipping flag is false at equality. The shared finite clamp
preserves the incoming signed zero at equality. All numeric inputs receive
IEEE finite-first admission. Passive power is nonpositive for this declared
viscous/Coulomb law; this does not prove energy decay for the integrated,
actively driven system. Coulomb friction is zero at zero speed, with no
stiction solve. Explicit integration can cross zero speed.

`voltage` is a supplied constant fraction of available effort. It models
neither battery dynamics nor back-EMF. `play` is a scalar observation play
operator with memory: it keeps the old output inside the new motor-centered
interval and moves it to the nearest boundary otherwise. It applies no load
reaction and transfers no inertia. Its output is separately observable; it
does not replace motor-side feedback or claim a mechanical gearbox model.
No Stribeck law, measured identification or randomization is implied.

Quantities reuse `rotor-pd-types.coax`: explicit radian coordinates,
generalized force J/rad, inertia J·s²/rad² and compatible gains/rates. The
usual N·m teaching numbers are in that one-radian angular basis. In contrast
to `rpd_step_owned`, damping here contributes explicit passive force; it is
not folded into an effective inertia.

## Admission and state

| Field | Inclusive numerical box |
| --- | --- |
| inertia | 10⁻⁶ to 100 |
| kp | 0 to 10,000 |
| kd, viscous, coulomb | 0 to 1,000 |
| effort | 10⁻⁶ to 1,000 |
| voltage | 0 to 1 |
| dt | 10⁻⁶ to 0.1 seconds |
| play half width | 0 to 1 radian |
| delay | 0 to 16 accepted updates |
| motor/output position and pending targets | −100 to 100 radians |
| motor rate | −10,000 to 10,000 radians/second |
| accepted-step counter | 0 to 2,147,483,647 |

These are numerical boxes, not operating limits for a robot. The bounded
input arithmetic remains finite (requested effort at most 12,000,000,
passive magnitude at most 10,001,000, acceleration below 1.1×10¹³).
A candidate position/rate outside its box refuses the whole step. We do not
silently clip a numerical failure into a physical joint limit.

`RsaSnapshot` contains all nine floating parameters, delay, motor position,
motor rate, output position, 16 pending targets, queue cursor and accepted
step count. Output must lie in `[q-play,q+play]`. Inactive pending slots are
zero and also receive finite/range admission. Delay zero requires cursor
zero; positive delay requires `0 <= cursor < delay`.

Initialization fills every active pending slot with the caller's declared
initial target. At positive delay, a step reads the cursor's old target,
replaces that slot with the incoming target and advances modulo delay.
At delay zero the incoming target applies immediately. The maximum counter
can be restored but cannot step again. Rejected inputs and derived-state
failures publish no force, cursor, counter or output prefix.

`rsa_restore` validates and copies one complete in-memory snapshot. This is
not a persistent checkpoint, experiment identity or authentication. RB-06
must bind it and the expected configuration into a registered companion
record; dumping a Coax struct is not a codec. There are no hidden counters,
random samples, external callbacks or device effects in this profile.

## Frozen references and gates

`psionic/tests/robotics/actuator_oracle.py` computes all reference steps using
exact rational arithmetic. `reference.json` retains each fraction; the
generated Coax vectors round each answer once to F64. The comparison budget
was fixed at `2e-12 + 2e-12 * abs(reference)` before product implementation.
The three cases cover force reversal, delay, output play, negative initial
velocity, derivative feedback, passive losses and reduced voltage.

The registered dispatcher is:

```shell
coax/remote/remote.sh run -- bash psionic/check.sh --only robotics/actuator
```

It runs debug/release checks, root mutation, the frozen oracle, two compiler
unit refusals, four native modes including ASan/UBSan and the exact example
transcript. Full numerical/refusal evidence belongs in the
[qualification record](../../../docs/verification/robotics-actuator-rb02-2026-11-15.md).
