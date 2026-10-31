# 109. Learn a physical system

We now own a small physical world. A one-kilogram mass moves along one axis,
attached to a spring and a damper, with an external force we choose. A checked
Coax solver generates trajectories. Two small neural networks learn its next
state from examples, and then run on their own predictions.

This is a useful first plant for later control experiments. It gives us known
physics, exact reset, repeatable forcing and an independent analytic reference.
It does not replace the articulated Panda and contact simulation in
[chapter 169](ch169-close-the-simulated-control-loop.md).

```shell
coax/bin/coax run psionic/examples/learn-a-physical-system.coax
coax/bin/coax run psionic/examples/learn-a-physical-system.coax --release
psionic/tests/physics/check-oscillator.sh /tmp/oscillator-native
```

The [example](../examples/learn-a-physical-system.coax) fits four scalar heads
and compares the resulting methods against mean and persistence baselines.
Its [expected output](../fixtures/learn-a-physical-system.out) is computed from
an independent implementation. The complete
[qualification record](../fixtures/physics/oscillator/qualification.json)
retains every split and method, including the comparisons the neural network
loses.

## First make the teacher trustworthy

The physical equation is:

```jai
mass = 1 kg
position' = velocity
velocity' = force - damping * velocity - spring * position
```

Position is in metres, velocity in metres per second, force in newtons, spring
stiffness in newtons per metre and damping in newton-seconds per metre. Mass is
fixed at exactly one kilogram. These are our declared units and equations;
they are not inferred from a downloaded robotics dataset.

The [profile](../lib/physics/oscillator-types.coax) accepts spring stiffness
from 0.5 to 4, damping from 0.1 to 1, position within ±2, velocity within ±4 and force
within ±2, inclusively. A step is between 2^-10 and 0.05 seconds. Every input must
be finite. An invalid request returns a status and the original state. A
numerically valid step that would leave the state bounds is also refused.

<!-- source: psionic/lib/physics/oscillator.coax field -->
```jai
os_rhs :: (p:OsParams,s:OsState,force:f64)->OsState {
  return OsState.{position=s.velocity,velocity=(force-p.damping*s.velocity)-p.spring*s.position};
}
```

The force stays constant through each complete step. We use classical
fourth-order Runge–Kutta, evaluating that field four times. The second and
third evaluations use half-step intermediate states; the fourth uses a full
step. The weighted sum produces the new position and velocity.

<!-- source: psionic/lib/physics/oscillator.coax step -->
```jai
os_step :: (p:OsParams,s:OsState,force:f64,dt:f64)->OsStep #export_c {
  status:=os_admit(p,s,force,dt);if status!=OsStatus.ok {return OsStep.{status=status,state=s};}
  a:=os_rhs(p,s,force);b:=os_rhs(p,os_shift(s,a,dt/2),force);
  c:=os_rhs(p,os_shift(s,b,dt/2),force);d:=os_rhs(p,os_shift(s,c,dt),force);
  next:=OsState.{position=s.position+(dt/6)*(((a.position+2*b.position)+2*c.position)+d.position),velocity=s.velocity+(dt/6)*(((a.velocity+2*b.velocity)+2*c.velocity)+d.velocity)};
  if !os_state_ok(next) {return OsStep.{status=OsStatus.state,state=s};}return OsStep.{state=next};
}
```

All arithmetic here is F64. No random force, hidden wall clock, external
simulator, automatic clipping or learned parameter enters the teacher.
`os_rollout` accepts at most 128 forces, stages the initial state and every
result, and publishes them together only if the whole rollout succeeds. It
leaves extra output storage untouched. A failure on the last force therefore
cannot publish a plausible-looking partial trajectory.

The independent oracle uses two different constructions. A C reference checks
individual steps against the analytic constant-force underdamped solution. A
Python reference evaluates the augmented matrix polynomial:

```jai
z = [position, velocity, 1]
A = [[0, 1, 0], [-spring, -damping, force], [0, 0, 0]]
z_next = (I + h*A + h^2*A^2/2 + h^3*A^3/6 + h^4*A^4/24) * z
```

That polynomial agrees with RK4 on this linear field without copying its four
stages. Across 5,400 admitted steps, the largest error against the analytic
solution is 3.76e-7 in either state coordinate, under the predeclared 1e-5
absolute budget. Halving the step twice at a fixed one-second horizon reduces
the vector error by 15.986 and 15.996, as expected for fourth-order convergence.
Another 2,304 unforced steps check nonincreasing mechanical energy. These checks
establish numerical behavior for the declared profile, rather than asserting
that any discretized physical system is exact.

## Freeze whole experiments before learning

The [experiment contract](../fixtures/physics/oscillator/experiment.json) was
written before fitting. It fixes every parameter, initial condition, forcing
rule, seed and training setting. There is no random row split:

| Split | Whole episode IDs | Steps per episode | Transitions | Forcing family |
| --- | --- | ---: | ---: | --- |
| Training | 0–7 | 32 | 256 | Three-level square sequence |
| Validation | 100–103 | 128 | 512 | Five-level triangle sequence |
| Test | 200–203 | 128 | 512 | Pulse sequence |

Each split uses different spring constants, damping coefficients, initial
positions and initial velocities. The held-out episodes also use different
forcing schedules. All coefficients remain within the admitted physical
range. This tests new parameter/initial-condition/forcing groups inside that
range; it does not establish arbitrary extrapolation beyond it.

The fixed step is 0.05 seconds. Training trajectories last 1.6 seconds and the
held-out rollouts last 6.4 seconds. A training row contains five observed
features and two labels:

```jai
features = [position, velocity, force, spring, damping]
target_position = (next_position - position) / 0.05
target_velocity = (next_velocity - velocity) / 0.05
```

The target is an average transition increment per second. It is not simply
the continuous field at the beginning of the step: RK4 has already accounted
for changes inside that interval. The surrogate therefore learns this fixed
0.05-second discrete transition. Changing its step at inference would require
a different contract and evidence.

Feature and target means and population standard deviations come only from
the 256 training rows. Training uses only those rows. The unit checks replace
all other feature/label rows and require bit-identical fitted statistics and
parameters. Transforming a held-out row with a training scale never fits a
new scale from that row.

## Learn two state increments

We reuse [chapter 103](ch103-learning-with-arrays.md)'s checked F64 trainer.
There are two affine heads, one for each target, and two separate tanh MLP
heads. Each MLP has five inputs, eight hidden units and 57 parameters; the pair
has 114. The affine pair has 12 parameters.

The training recipe is fixed: 256 epochs, batches of 32, learning rate 0.03 and
weight-only decay 0.0001. Each head makes 2,048 minibatch updates; all four make
8,192. MLP initialization seeds are 10910/10911 and shuffle seeds are 10920/10921.
Affine heads start at zero under chapter 103's initialization rule. The
recorded affine seed fields 10900/10901 do not change that zero initialization.
Each head's Fisher–Yates shuffle counter persists across epochs.

Every method predicts a complete next position/velocity pair. Method 0 adds
the mean training increment, method 1 keeps the previous state, method 2 uses
the affine heads and method 3 uses the MLP heads:

<!-- source: psionic/lib/physics/oscillator-learn.coax predict -->
```jai
oe_predict :: (fit:OeFit,kind:int,p:OsParams,s:OsState,force:f64)->OsStep #export_c {
  status:=os_admit(p,s,force,OS_DT);if status!=OsStatus.ok {return OsStep.{status=status,state=s};}
  if !oe_fit_ok(fit) || kind<0 || kind>3 {return OsStep.{status=OsStatus.parameters,state=s};}
  if kind==1 {return OsStep.{state=s};}
  affine:=fit.affine;mlp:=fit.mlp;raw:=[5]f64.[s.position,s.velocity,force,p.spring,p.damping];x:[5]f64;for i:0..<5 {x[i]=(raw[i]-fit.mean[i])/fit.sd[i];}delta:[2]f64;
  for axis:0..<2 {
    delta[axis]=fit.target_mean[axis];
    if kind==2 {delta[axis]+=fit.target_sd[axis]*tb_output(oe_model(kind),affine[axis*6..<axis*6+6],x[..]);}
    if kind==3 {delta[axis]+=fit.target_sd[axis]*tb_output(oe_model(kind),mlp[axis*57..<axis*57+57],x[..]);}
  }
  next:=OsState.{position=s.position+OS_DT*delta[0],velocity=s.velocity+OS_DT*delta[1]};
  if !os_state_ok(next) {return OsStep.{status=OsStatus.state,state=s};}return OsStep.{state=next};
}
```

The spring and damping are supplied as features. The network must learn their
interactions with position and velocity; an affine head cannot directly
represent products such as spring times position. This tiny teacher is already
cheap: the experiment establishes a learned approximation, with no claim that
the MLP runs faster than RK4.

The neural network is trained from scratch on our solver's data. Its gradients
come from chapter 103's supervised trainer. This chapter neither differentiates
through the simulator nor depends on unfinished general function transforms.
Recovering the spring and damping directly from motion is chapter 111's next
experiment.

## One-step accuracy is a different question from rollout accuracy

A one-step prediction starts from the true previous state every time. It asks
whether the model approximates one transition near the teacher's trajectory.
An independent rollout starts from the same initial state once and then uses
its own predicted state at every following step. It sees the same declared
forcing sequence and parameter values, but no intermediate teacher state.

<!-- source: psionic/lib/physics/oscillator-evaluate.coax evaluation -->
```jai
oe_evaluate :: (fit:OeFit,kind:int,split:int)->OeMetric #export_c {
  if kind<0 || kind>3 || split<0 || split>2 {return OeMetric.{status=OsStatus.shape};}
  if !oe_fit_ok(fit) {return OeMetric.{status=OsStatus.parameters};}
  m:OeMetric;count:=ifx split==0 then 8 else 4;first:=ifx split==0 then 0 else (ifx split==1 then 100 else 200);
  for group:0..<count {
    e:=oe_episode(first+group);truth:=e.initial;learned:=e.initial;alive:=true;m.episodes+=1;
    for tick:0..<e.steps {
      u:=oe_force(e.id,tick);next:=os_step(e.params,truth,u,OS_DT);if next.status!=OsStatus.ok {return OeMetric.{status=next.status};}
      one:=oe_predict(fit,kind,e.params,truth,u);
      if one.status==OsStatus.ok {q:=one.state.position-next.state.position;v:=one.state.velocity-next.state.velocity;m.one_q+=q*q;m.one_v+=v*v;m.one_rows+=1;}else {m.one_refused+=1;}
      if alive {
        own:=oe_predict(fit,kind,e.params,learned,u);
        if own.status!=OsStatus.ok {alive=false;m.roll_refused+=1;if m.first_episode<0 {m.first_episode=e.id;m.first_tick=tick;}}
        else {learned=own.state;q:=learned.position-next.state.position;v:=learned.velocity-next.state.velocity;m.roll_q+=q*q;m.roll_v+=v*v;m.max_q=oe_larger(m.max_q,abs(q));m.max_v=oe_larger(m.max_v,abs(v));m.roll_rows+=1;}
      }
      truth=next.state;
    }
    if alive {m.completed+=1;}
  }
  return m;
}
```

The evaluator keeps those states separate. It accumulates squared position
and velocity errors and their explicit accepted-row counts. RMSE is the square
root of each sum divided by its own count. It also records peak absolute errors,
completed episodes and the first refused episode/tick.

A refused one-step prediction is counted rather than scored as a successful
persistence prediction. A refused rollout stops permanently; its accepted
prefix and refusal remain visible. It does not restart from the teacher or
clip its prediction into range. Deliberate failure checks exercise those paths.
All methods in the frozen full experiment happen to complete every episode.

The measured results are:

| Split | Method | One-step position RMSE, m | One-step velocity RMSE, m/s | Rollout position RMSE, m | Rollout velocity RMSE, m/s | Complete |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Train | Mean increment | .025912 | .040662 | .429534 | .548895 | 8/8 |
| Train | Persistence | .025924 | .040666 | .429808 | .548564 | 8/8 |
| Train | Affine | .000450 | .017817 | .137937 | .160483 | 8/8 |
| Train | MLP | .001305 | .004037 | .021996 | .033309 | 8/8 |
| Validation | Mean increment | .019145 | .034729 | .649898 | .398526 | 4/4 |
| Validation | Persistence | .019166 | .034711 | .645638 | .393213 | 4/4 |
| Validation | Affine | .000383 | .015222 | .200133 | .260319 | 4/4 |
| Validation | MLP | .001705 | .008055 | .113540 | .090369 | 4/4 |
| Test | Mean increment | .014698 | .026781 | .604206 | .325772 | 4/4 |
| Test | Persistence | .014688 | .026778 | .597703 | .321339 | 4/4 |
| Test | Affine | .000388 | .015414 | .187958 | .212786 | 4/4 |
| Test | MLP | .002035 | .011290 | .138474 | .122190 | 4/4 |

The MLP improves repeated held-out rollouts over all three baselines. Affine
wins one-step position error, including on the test groups. The distinction is
physical: velocity error feeds later position error. A model can predict one
position increment very accurately while accumulating poor future velocities.
The generalization gap is also substantial: MLP position rollout RMSE rises
from 0.021996 metres on training episodes to 0.138474 metres on test episodes. Its
largest test errors are 0.329235 metres and 0.451655 metres per second. Completing
a bounded rollout is not the same as meeting a useful control tolerance.

We keep those results. Validation did not select a different architecture,
training duration or learning rate, and test results did not trigger a second
search. A later improvement should be a separately recorded experiment.

## Check the complete construction

The new roots contain 16 authored checks and kill 485/485 changed-root mutants
with zero survivors. Debug, release and ASan+UBSan native runs independently
check the analytic solver, storage/refusal boundaries and energy/refinement
properties. Their serialized solver, dataset, fitted parameter and metric
words are bit identical on the qualified host and compiler profile.

The separate Python implementation reconstructs all 8,960 feature/target values,
1,280 group IDs, 140 fitted statistics/parameters and 12 metric records. Maximum
absolute differences are 4.44e-15 for data, 1.55e-15 for the fitted values and
5.69e-14 for metric sums/peaks, under the
[predeclared numerical budgets](../fixtures/physics/oscillator/verification-contract.json).
It uses its own matrix-polynomial teacher, scaling, random counter, shuffle,
SGD and forward/rollout code. The oracle and native C harness are development
tooling; the runnable model needs only Coax and the host C compiler.

The [test notes](../tests/physics/OSCILLATOR.md) give the exact focused commands.
No unchanged global gates or original-weight CPU inference were needed.

This gives later chapters a small owned plant with declared dynamics and
repeatable reset, plus a measured learned approximation that can be wrong.
A two-axis plant can compose two such states with a new contract and checks.
Articulated dynamics, contact, a Panda-compatible renderer, compiler boundedness,
real-time deadlines and differentiable simulator parameters require their own
evidence; this chapter claims none of them.
