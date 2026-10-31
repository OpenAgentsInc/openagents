# Fixed Panda grip-position IK

This is a pure, source-bound seven-hinge kinematic query. The public target is
one `Point3(PfkWorld, Metre, f64)`. Seeds and results use typed radians. The
pinned compiled model supplies the inclusive joint ranges; no caller model or
joint-limit table is accepted.

The F64 profile fixes damping at 0.05 metres after normalizing each Jacobian
column by exactly one radian. It caps the full update by its largest absolute
coefficient at 0.1 radians, then projects coordinates into the source ranges.
The solver accepts update budgets 0 through 96 and succeeds at Euclidean
position error at most 1e-6 metres. Targets must be finite, within ten metres
per world coordinate. `not_converged` means this fixed seed/budget/profile
exhausted; it does not prove global unreachability.

`pik_step` publishes one admitted geometric candidate, including a fresh grip
position, its residual and the number of projected coordinates. `pik_solve`
accumulates those counts and publishes only a converged result. Every non-OK
path preserves the entire caller output. Raw adapters explicitly interpret
radian and world-metre coefficients. Their helpers remain source-visible
trusted arithmetic, not a sealed provenance or physical command boundary.

`experiment.json` and `cases.bin` were frozen before implementation. They
contain 49 contexts: home; seven targets from authenticated retained fresh SDK
PcSamples; 24 independent matrix-generated reachable targets; three explicit
nonconvergence/budget cases; and fourteen exact source-range endpoints.
The retained samples bind positions and their matching coordinate tuples. All
facts are bundled numerical references. No new SDK capture, neural execution,
rendering or simulation is part of this component.

`checks-qualification.json` records 12 focused source groups/38 cases, the two
new roots' actual emitted mutation counts (102 and 152, both zero survivors),
four meaningful E0317 unit/frame/scale refusals, and the debug/release example.
Initial incomplete observations and fixture/setup failures are retained. The
native receipt is `native-qualification.json`: its independently authored
homogeneous-matrix/tangent + pivoted-Gaussian iteration compares every new
trajectory with the frozen 2e-10 absolute/zero-relative coordinate, position
and residual budgets and exact counts. Existing FK/Jacobian qualification is
carried rather than rerun.

Run the new offline native/unit proof into a new directory on the qualified
Darwin arm64 scalar-libm host:

```shell
COAX=coax/bin/coax psionic/tests/robotics/panda_ik_check.sh /tmp/new-panda-ik-proof
coax/bin/coax run psionic/examples/panda-ik.coax
```

The host profile disables fast math/contraction and uses scalar libm sin/cos.
Cross-target determinism remains separate. Orientation and nullspace IK,
collision checking, RRT-Connect, trajectory timing, dynamics/PD rollout,
compiler-enforced `#bounded`, real actuator admission and a complete robot
backend remain open. A candidate within source ranges can still collide.

## Qualified native outcomes

The original 49-case experiment stays unchanged. A separately frozen
`oracle-profile.json` adds the existing active-limit contract case before
comparison: it converges in eleven updates with eleven projections. The
50-context reference reports **47 successful solves and three exhausted
budgets**, retaining every outcome.

Debug, release and fail-fast ASan/UBSan each pass **15,507 assertions** over
**363 events**: 50 full solves, 35 standalone steps, 228 trajectory updates and
50 terminal zero-budget queries. Status, update and projection fields match
exactly. Maximum coordinate error is 1.22e-14 rad, position error 4.22e-15 m
and residual error 9.00e-16 m, inside the frozen 2e-10 absolute budgets.
All owned event streams match byte for byte. Fourteen source-limit boundaries,
six target boundaries, 50 IEEE poisons, eight overlapping-input/output cases,
complete typed/raw input snapshots and ten late actual/expected reader poisons
pass. The native receipt retains the README's execution-time identity as
historical documentation; this outcome paragraph is a metadata followup.

The final new-root mutation baselines execute the current checks. The earlier
focused source log has the same 12 groups/38 cases and precedes one final
observation-only change: an out-of-box target is also tested with zero update
budget, proving admission occurs before exhaustion. No product arithmetic or
frozen corpus changed in that observation followup. Counts describe emitted
sites; dependency proofs retain their own historical scopes.
