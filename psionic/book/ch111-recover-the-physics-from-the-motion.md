# 111. Recover the physics from the motion

[Chapter 109](ch109-learn-a-physical-system.md) learned a neural approximation
to a solver. Here we keep the solver and learn two of its physical parameters.
We observe a mass moving under known forces, infer its spring stiffness and
damping, then predict new motion under different initial conditions and forces.

The parameters come from gradients through the exact, repeatedly applied RK4
transition. No surrogate is substituted. Mass stays exactly one kilogram;
initial states, applied forces and the 0.05-second step are known.

```shell
coax/bin/coax run psionic/examples/recover-the-physics.coax
coax/bin/coax run psionic/examples/recover-the-physics.coax --release
psionic/tests/physics/check-inverse.sh /tmp/inverse-native
```

The [example](../examples/recover-the-physics.coax),
[golden output](../fixtures/recover-the-physics.out),
[frozen experiment](../fixtures/physics/inverse/experiment.json) and
[full qualification record](../fixtures/physics/inverse/qualification.json)
include every noise/sampling case and the deliberately ambiguous case.

## An inverse problem needs known inputs

The forward problem knows spring stiffness `k` and damping `c` and predicts
motion. The inverse problem observes motion and asks which `k,c` explain it:

```jai
position' = velocity
velocity' = force - c * velocity - k * position
unknown = [k, c]
known = [mass=1, initial_position, initial_velocity, force_at_each_step, dt=0.05]
```

Spring remains in [0.5,4] N/m and damping in [0.1,1] N·s/m. Position, velocity and
force retain chapter 109's physical bounds. These are admitted numerical
coordinates with declared units, not a compiler units or boundedness proof.

We generate observations from the owned solver at `k=2.4,c=0.55`. The inference
code receives the observed states, known initial states and forces; the true
coefficients are not embedded in its objective program.

Three excited training trajectories begin at:

| Episode | Position, m | Velocity, m/s |
| --- | ---: | ---: |
|0|.8|0|
|1|0|.8|
|2|-.6|-.4|

Each takes 24 steps, lasting 1.2 seconds. Its piecewise-constant force is
`[0,.5,-.5][(tick/6+episode)%3]`, using integer division. Different initial
positions and velocities excite different combinations of spring and damping.
A small training loss alone will not prove that either coefficient is unique.

## Write the actual solver on a tape

[Chapter 104](ch104-gradients-as-a-program.md)'s F64 tape already differentiates
scalar programs. We explicitly construct its nodes for chapter 109's field,
intermediate states and left-associated RK4 sums. Parameter nodes 0 and 1 are
shared by every stage, step and episode, so the backward pass accumulates all
uses of each coefficient.

<!-- source: psionic/lib/physics/inverse-physics.coax step -->
```jai
// Node order exactly follows os_step, including its left-associated sums.
oi_rk4 :: (p:[]OiProgram,s:OiPair,u:int,two:int,half:int,full:int,sixth:int)->OiPair {
  a:=oi_rhs(p,s,u);b:=oi_rhs(p,oi_shift(p,s,a,half),u);c:=oi_rhs(p,oi_shift(p,s,b,half),u);
  d:=oi_rhs(p,oi_shift(p,s,c,full),u);
  bq:=oi_binary(p,TpOp.mul,two,b.q);cq:=oi_binary(p,TpOp.mul,two,c.q);
  bv:=oi_binary(p,TpOp.mul,two,b.v);cv:=oi_binary(p,TpOp.mul,two,c.v);
  qs:=oi_binary(p,TpOp.add,a.q,bq);qs=oi_binary(p,TpOp.add,qs,cq);qs=oi_binary(p,TpOp.add,qs,d.q);
  vs:=oi_binary(p,TpOp.add,a.v,bv);vs=oi_binary(p,TpOp.add,vs,cv);vs=oi_binary(p,TpOp.add,vs,d.v);
  dq:=oi_binary(p,TpOp.mul,sixth,qs);dv:=oi_binary(p,TpOp.mul,sixth,vs);
  return OiPair.{q=oi_binary(p,TpOp.add,s.q,dq),v=oi_binary(p,TpOp.add,s.v,dv)};
}
```

`OiPair` names two tape nodes. Helpers append the same multiply/add/subtract
operations as the original solver. This is a physics-specific program builder;
it does not automatically transform an arbitrary Coax procedure or implement
chapter 107's general function API.

The owned constructor accepts one to three episodes, one to 24 steps per episode
and a sampling stride that divides the step count. All observations, initial
states and forces must be finite and within the declared bounds. The maximum
case saves 72 states and observes all 72 of them in 3,687 nodes, below chapter 104's
4,096-node cap. Sampling every fourth next state uses 18 observations and 3,255 nodes.
The unsampled dynamics are still evaluated and differentiated.

Across 35 variant/parameter combinations, native checks compare every generated
state with the original `os_step`: 2,520 state pairs are bit identical. A separate
matrix-polynomial reference also checks all 5,040 state words, agreeing within
5.56e-16. These are separate comparisons, with distinct reference methods.

## Compare predicted motion with observations

Each observed time contributes both position and velocity residuals. Fixed
scales of one metre and one metre per second make the objective dimensionless:

```jai
loss = mean_over_observed_times(
    0.5 * ((predicted_position - observed_position) / 1m)^2
  + 0.5 * ((predicted_velocity - observed_velocity) / (1m/s))^2
)
```

The numerical scale factors are both one, so no fitted standardization enters
the code. A full-sampling case has 72 observation times. Stride 4 uses the next
states at 0.2, 0.4, ..., 1.2 seconds for each episode, 18 times total. The final tape
node averages over exactly those observations.

<!-- source: psionic/lib/physics/inverse-physics.coax objective -->
```jai
oi_value :: (program:OiProgram,spring:f64,damping:f64,gradient:bool)->OiValue #export_c {
  if !oi_program_ok(program) {return OiValue.{status=OsStatus.shape};}
  params:=[2]f64.[spring,damping];for x:params {if !os_finite(x) {return OiValue.{status=OsStatus.number};}}
  if spring<0.5 || spring>4 || damping<0.1 || damping>1 {return OiValue.{status=OsStatus.parameters};}
  nodes:=program.nodes;values:[4096]f64;
  if tp_forward(nodes[0..<program.count],params[..],values[..],false)!=TpStatus.ok {return OiValue.{status=OsStatus.number};}
  for i:0..<program.states {s:=OsState.{position=values[program.q_node[i]],velocity=values[program.v_node[i]]};if !os_state_ok(s) {return OiValue.{status=OsStatus.state,forward=true};}}
  result:=OiValue.{forward=true,loss=values[program.count-1],end=OsState.{position=values[program.q_node[program.states-1]],velocity=values[program.v_node[program.states-1]]}};
  if gradient {g:[2]f64;if tp_backward(nodes[0..<program.count],values[0..<program.count],g[..],false)!=TpStatus.ok {return OiValue.{status=OsStatus.number,forward=true};}result.gradient=g;result.reverse=true;}
  return result;
}
```

`tp_forward` saves values and `tp_backward` supplies the two gradients. We
start their accumulation at zero for every objective call. Physical bounds
are checked on every saved next state before a usable gradient is returned.
The result reports which forward/reverse passes completed, so the optimizer's
work record follows actual execution.

The derivative is of the smooth RK4 program inside its admitted domain.
Parameter projection is an optimizer decision outside that tape; this does
not claim a derivative through a refused state or a clipped update.

## Check gradients before trusting the fit

The independent reference uses the augmented linear-system matrix:

```jai
z = [position, velocity, 1]
A = [[0, 1, 0], [-k, -c, force], [0, 0, 0]]
z_next = (I + h*A + h^2*A^2/2 + h^3*A^3/6 + h^4*A^4/24) * z
d(A*z) = dA*z + A*dz
```

It propagates analytic stiffness/damping sensitivities through those matrix
powers and through successive steps. It does not construct TpNodes, call Coax
objectives, copy the product's RK stage sequence or fit using finite differences.
Across 35 cases, the tape gradients and analytic sensitivities differ by at most
2.78e-16, within the predeclared 1e-11 absolute budget.

We also central-difference the Coax objective at three interior parameter
points for every observation variant. This is a consistency check of its
forward and reverse paths, rather than an independent recovery proof:

| Difference step | Worst absolute gradient difference |
| --- | ---: |
|1e-2|4.34414e-6|
|1e-3|4.34408e-8|
|1e-4|4.34833e-10|
|1e-5|3.99662e-12|
|1e-6|3.32057e-11|
|1e-7|1.97906e-10|

The sweep shows truncation error falling and then floating-point subtraction
error rising. The fixed 1e-5 step is comfortably inside its frozen 1e-7 absolute
budget. At parameter-box endpoints the analytic comparison still applies;
we do not central-difference through an inadmissible parameter value.

## Fit under a stated work budget

The normal experiments begin at `k=1,c=0.9`. We freeze 255 projected-gradient
updates at rate 2, then evaluate the final loss once. There is no line search,
heldout model selection or early stopping:

<!-- source: psionic/lib/physics/inverse-fit.coax fit -->
```jai
oi_fit :: (program:OiProgram,initial:OsParams,updates:int)->OiFit #export_c {
  if !oi_fit_count_ok(updates) || !oi_program_ok(program) {return OiFit.{status=OsStatus.shape,params=initial};}
  if initial.mass!=1 {return OiFit.{status=OsStatus.parameters,params=initial};}
  fit:=OiFit.{params=initial};
  for iteration:0..<updates {
    g:=oi_value(program,fit.params.spring,fit.params.damping,true);if g.forward {fit.forward+=1;}if g.reverse {fit.reverse+=1;}
    if g.status!=OsStatus.ok {fit.status=g.status;fit.params=initial;return fit;}
    raw_k:=fit.params.spring-OI_RATE*g.gradient[0];raw_c:=fit.params.damping-OI_RATE*g.gradient[1];
    k:=oi_clamp(raw_k,0.5,4);c:=oi_clamp(raw_c,0.1,1);
    if k!=raw_k {fit.projected+=1;}if c!=raw_c {fit.projected+=1;}
    fit.params.spring=k;fit.params.damping=c;
  }
  last:=oi_value(program,fit.params.spring,fit.params.damping,false);if last.forward {fit.forward+=1;}if last.reverse {return OiFit.{status=OsStatus.shape,params=initial};}
  if last.status!=OsStatus.ok {fit.status=last.status;fit.params=initial;return fit;}
  fit.loss=last.loss;return fit;
}
```

If a late objective fails, the fit returns the original parameter pair with a
failure status and the completed-work counts. A partial optimization does not
become a successful published model. Separate checks force both clipping and
late physical failures.

The baseline evaluates a uniform 16×16 grid over the inclusive parameter box,
using only the same training objective. It keeps the first minimum on ties:

<!-- source: psionic/lib/physics/inverse-fit.coax grid -->
```jai
oi_grid :: (program:OiProgram,side:int)->OiFit #export_c {
  if !oi_grid_side_ok(side) || !oi_program_ok(program) {return OiFit.{status=OsStatus.shape};}
  fit:OiFit;best:f64;
  for row:0..<side {for column:0..<side {
    k:=oi_grid_k(row,side);c:=oi_grid_c(column,side);
    value:=oi_value(program,k,c,false);if value.forward {fit.forward+=1;}if value.reverse {return OiFit.{status=OsStatus.shape};}
    if value.status!=OsStatus.ok {return OiFit.{status=value.status,forward=fit.forward};}
    if fit.forward==1 || value.loss<best {best=value.loss;fit.params=OsParams.{spring=k,damping=c};}
  }}
  fit.loss=best;return fit;
}
```

Both methods complete 256 forward objectives. Gradient descent also completes
255 reverse passes; the grid completes none. This matches the forward-loss
budget, not total arithmetic or wall time. We report the extra reverse work
explicitly. An adaptive derivative-free method could use its budget differently;
this experiment compares with this particular uniform grid.

The independent Python reference separately reproduces both optimizers from
its analytic sensitivities and matrix-polynomial loss. Every fitted/grid
coefficient agrees within 5.56e-16, under the predeclared 1e-9 parameter budget.

## Change observation noise and sampling without retuning

Before fitting, we fix six experiments: noise amplitudes 0, 0.002 and 0.01, each
with stride 1 or 4. Position noise uses metres and velocity noise uses metres per
second. A counter-based uniform stream with seed 11100 adds bounded noise in
`[-amplitude,+amplitude)` to each next-state coordinate. Initial states remain
known exactly. Each stride sees the same underlying noisy trajectory before
selecting its observation times.

The fitted parameters then predict three new 128-step trajectories, lasting
6.4 seconds, from `(q,v)=(.7,-.15),(-.5,.35),(0,-.7)`. Their new force sequence
uses a 20-step pulse with levels .4, -.2 and 0. No heldout observation enters fitting
or grid selection. Both predicted and true motion evolve independently from
the same initial state; there are no teacher resets inside a forecast.

<!-- source: psionic/lib/physics/inverse-evaluate.coax forecast -->
```jai
oi_forecast :: (params:OsParams)->OiEval #export_c {
  result:OiEval;qs:=[3]f64.[0.7,-0.5,0];vs:=[3]f64.[-0.15,0.35,-0.7];
  for episode:0..<3 {
    truth:=OsState.{position=qs[episode],velocity=vs[episode]};own:=truth;
    for tick:0..<128 {
      u:=oi_holdout_force(episode,tick);next:=os_step(oi_truth(),truth,u,OS_DT);predicted:=os_step(params,own,u,OS_DT);
      if next.status!=OsStatus.ok {return OiEval.{status=next.status};}if predicted.status!=OsStatus.ok {return OiEval.{status=predicted.status};}
      truth=next.state;own=predicted.state;q:=own.position-truth.position;v:=own.velocity-truth.velocity;
      result.q_square+=q*q;result.v_square+=v*v;result.max_q=oi_peak(result.max_q,abs(q));result.max_v=oi_peak(result.max_v,abs(v));result.rows+=1;
    }
  }
  return result;
}
```

All six normal experiments finish their heldout 384 states. The measurements are:

| Noise amplitude | Stride | Observations | Recovered k | Recovered c | Heldout position RMSE, m | Heldout velocity RMSE, m/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
|0|1|72|2.399999995|.549999995|1.26e-9|1.84e-9|
|0|4|18|2.399999999|.549999999|2.05e-10|3.02e-10|
|.002|1|72|2.400837582|.550594731|.000161386|.000241727|
|.002|4|18|2.401312703|.549266727|.000304483|.000450337|
|.01|1|72|2.404191158|.552971765|.000804784|.001205676|
|.01|4|18|2.406566067|.546337862|.001524468|.002257117|

The clean full-sampling estimate recovers both coefficients within 5.1e-9. This
is synthetic motion generated by the same solver family and step that we fit,
with known mass, forcing and initial conditions. Its tiny clean forecast error
is numerical reconstruction evidence, not nanometre accuracy on a real mass.

The uniform grid selects `k=2.366666667,c=0.52` in all six cases. Its heldout
position/velocity RMSE is 0.007813726 metres and 0.011529825 metres per second.
The gradient fit does better here, while spending the additional reverse passes.
The complete record also retains all training losses and peak forecast errors.

The noisy sparse observations produce larger parameter and forecast errors
in this frozen sample. One fixed noise stream does not establish statistical
robustness, and fewer clean observations need not be worse in every case.
No result triggered a new training recipe or parameter search.

## Motion can leave a coefficient unidentified

Now hold the mass at a nonzero equilibrium: position .5, velocity 0, constant
force 1.2 and stiffness 2.4. The spring exactly balances the force. Damping
multiplies zero velocity, so changing it cannot change this motion:

```jai
force - k * position = 1.2 - 2.4 * 0.5 = 0
c * velocity = c * 0 = 0
c = 0.2 and c = 0.9 produce identical observed trajectories
```

Both parameter choices have exactly zero training loss and zero damping
gradient. Starting this demonstration at `k=2.4,c=0.9` leaves damping at .9,
rather than recovering the generating value .55. Its training fit is perfect,
but the new moving trajectories have position/velocity RMSE 0.065606 metres
and 0.092152 metres per second. The grid's estimate for this separate equilibrium
case is `k=2.366666667,c=1`, with still larger forecast errors.

This is a concrete ambiguity, not an optimizer defect. Excited trajectories
recover the known coefficients in our frozen cases, but that is not a proof
of global uniqueness for every possible observation sequence. More fitting
iterations cannot supply information absent from the motion.

## Keep numerical proof and coverage proof distinct

Fifteen authored checks kill 354/354 changed-root mutants with zero survivors.
Native debug, release and ASan+UBSan pass 9,326 assertions each, including 666
nonfinite data cases, six nonfinite parameter cases, exact original-solver
state comparisons and all seven fit/grid/forecast experiments. Serialized
state, data, gradient/finite-difference and fit/forecast payloads are bit
identical across those native modes on the qualified host/profile.

After the independent numerical passes, review added explicit transcript
coverage guards: exactly 35 unique variant/parameter gradient records, all 252
finite-difference pairs, seven ordered fit records and their expected node and
observation counts. A separate new gate checks those guards, rejects five
truncated/duplicate cases and independently checks all 5,040 state values. It
preserves the earlier numerical records and proves their numeric-function ASTs
unchanged, rather than repeating passing model/native/mutation work. The
[qualification receipt](../fixtures/physics/inverse/qualification.json) records
both stages and their source/payload hashes. Both example modes match the
independently generated golden transcript.

The [test notes](../tests/physics/INVERSE.md) explain the focused reproduction
commands and numerical boundaries. Python and native reference code are
development tooling. The model and inference logic remain Coax.

We have differentiated an owned smooth one-axis simulator and recovered its
coefficients under stated observations. This does not implement articulated
or contact dynamics, a Panda replacement, general function transforms, compiler
boundedness, typed clocks, hardware control or real-world system identification.
Those require their own contracts and evidence.
