# 112. Imitation before robotics scale

A robot policy maps an observation to an action. Before importing a large
vision-language-action model, we need a small example where the observations,
units, teacher, training split and meaning of success are all explicit. This
chapter builds one: a two-axis Cartesian point moves toward a target, learning
from demonstrations of a simple controller.

The [runnable example](../examples/imitation-before-robotics-scale.coax) trains
the policy in Coax and compares it with the teacher and a policy that does
nothing. A separate native exercise reads a tiny original NVIDIA SingleArm
data selection. These are two different systems: the owned simulation teaches
imitation and evaluation; the NVIDIA fixture teaches artifact conventions and
exact data admission. We do not feed undocumented Panda commands into our
simulator or claim that the learned point controller operates an arm.

## Define the system before collecting demonstrations

The profile is `robotics-cartesian-bc-f64-v1`. Its
[state and action types](../lib/robotics/types.coax) name a position, a goal and
a commanded velocity. All simulation arithmetic is binary64.

| Convention | Value |
| --- | --- |
| Position and target | Two Cartesian coordinates, metres, each in `[-1, 1]` |
| Observation | Current target error divided by a fixed `0.25 m` scale |
| Action | Velocity, bounded to `0.5 m/s` **per axis** |
| Action normalization | Divide velocity by `0.5 m/s` |
| Integration | Exactly `0.1 s` per tick; `position += dt * velocity` |
| Training | Four whole episodes, IDs 0–3; eight teacher steps each |
| Evaluation | Six different whole episodes, IDs 100–105; 96 ticks each |
| Success | Both final absolute axis errors are at most `0.01 m` |

Fixed scales are part of this profile. We do not estimate them using held-out
states. Error divided by `0.25 m` is dimensionless, as is velocity divided by
`0.5 m/s`. The policy has one affine head per axis: gain times normalized error
plus bias. Clipping its normalized output to `[-1, 1]` and multiplying by the
velocity scale produces a command with declared units:

<!-- source: psionic/lib/robotics/sim.coax action -->
```jai
rk_action :: (p:RkPolicy,s:RkState)->RkAction #export_c {
  if !rk_state_ok(s) {return RkAction.{status=RkStatus.state};}
  if !rk_policy_ok(p) {return RkAction.{status=RkStatus.policy};}
  nx:=(s.goal.x-s.position.x)/RK_POSITION_SCALE;ny:=(s.goal.y-s.position.y)/RK_POSITION_SCALE;
  ux:=p.x_gain*nx+p.x_bias;uy:=p.y_gain*ny+p.y_bias;
  x:=rk_clip(ux,1);y:=rk_clip(uy,1);
  return RkAction.{velocity=RkVec.{x=RK_SPEED*x.value,y=RK_SPEED*y.value},clipped=x.clipped || y.clipped};
}
```

The limits are componentwise; diagonal motion may have a larger Euclidean
speed. The [simulator](../lib/robotics/sim.coax) refuses invalid states, policies,
commands, another tick duration or a step outside its workspace. A refused
step returns the original state. That keeps an error from becoming a partial
movement. There are no obstacles, contacts, motors, images or delayed sensors
in this system.

## Learn the teacher instead of installing its answer

The teacher commands velocity equal to target error with proportional gain
`1/s`, clipped to the same per-axis speed bound. The
[authored episode table](../lib/robotics/owned-episodes.coax) precedes fitting.
Its four training episodes start with errors no larger than `0.25 m`. We
advance the teacher eight times in each episode and record normalized errors
and actions: 32 rows per axis.

For an unclipped demonstration, input is `error / 0.25` and target is
`error / 0.5`. The independently known solution is therefore normalized gain
`0.5` and bias zero. We use that answer to check the learned parameters; the
learner starts from its configured initialization and fits them from the rows.

[Behavior cloning](../lib/robotics/learn.coax) reuses
[chapter 70's](ch070-learning-with-arrays.md) checked tabular SGD. Each affine
head receives 128 full-batch epochs, batch size 32, learning rate 1, no decay
and fixed shuffle seed 112. Together the two heads perform 256 updates. A
failed training receipt cannot publish a policy.

<!-- source: psionic/examples/imitation-before-robotics-scale.coax fit -->
```jai
    demonstrations:=rk_demonstrations();
    #assert demonstrations.episodes==4 && demonstrations.count==32;
    fit:=rk_train();
    #assert fit.status==RkStatus.ok && fit.steps==256;
    #assert abs(fit.policy.x_gain-0.5)<1e-10 && abs(fit.policy.y_gain-0.5)<1e-10;
    #assert abs(fit.policy.x_bias)<1e-10 && abs(fit.policy.y_bias)<1e-10;
    print("demonstrations: % episodes, % rows\n",demonstrations.episodes,demonstrations.count);
    print("training: % updates\n",fit.steps);
    print("normalized gains, millionths: % %\n",rk_example_round(fit.policy.x_gain,1000000),rk_example_round(fit.policy.y_gain,1000000));
```

This is a deliberately small function class. Its independently learned axes
and explicit saturation can reproduce this teacher even at the larger errors
in evaluation. That is evidence for this pipeline and controller, rather than
evidence of general manipulation skill. Whole held-out episodes are separate
from training; randomly splitting adjacent frames of the same demonstration
would not give that separation.

## A good next action is different from a successful rollout

We keep two states during evaluation. The **teacher state** follows expert
actions, and measures each candidate's next-action error on the same inputs.
The **closed-loop state** follows the candidate's own actions and measures
where that policy actually ends up. A small teacher-forced error does not
guarantee a stable rollout: its mistakes may change future observations.

<!-- source: psionic/lib/robotics/evaluate.coax evaluate -->
```jai
rk_measure :: (p:RkPolicy,expert:bool)->RkEval {
  if !rk_policy_ok(p) {return RkEval.{status=RkStatus.policy};}
  successes:=0;total:=0.0;peak:RkPeak;
  for episode:100..<106 {
    initial:=rk_episode(episode).state;teacher:=initial;closed:=initial;
    for tick:0..<RK_HORIZON {
      truth:=rk_expert(teacher);predicted:=ifx expert then truth else rk_action(p,teacher);
      dx:=predicted.velocity.x-truth.velocity.x;dy:=predicted.velocity.y-truth.velocity.y;
      total+=dx*dx+dy*dy;
      teacher=rk_step(teacher,truth.velocity,RK_DT).state;
      chosen:=ifx expert then rk_expert(closed) else rk_action(p,closed);
      next:=rk_step(closed,chosen.velocity,RK_DT);
      if next.status!=RkStatus.ok {return RkEval.{status=next.status};}
      closed=next.state;
    }
    ex:=abs(closed.goal.x-closed.position.x);ey:=abs(closed.goal.y-closed.position.y);
    peak=rk_peak(peak,ex,ey,episode);
    if rk_success(ex) && rk_success(ey) {successes+=1;}
  }
  return RkEval.{episodes=6,successes=successes,steps=6*RK_HORIZON,mse=total/f64(2*6*RK_HORIZON),final_error_max=peak.error,worst_episode=peak.episode,worst_axis=peak.axis};
}
```

The velocity MSE averages both coordinates at all 576 expert-trajectory states.
The final error is the largest absolute axis error across the six independent
candidate rollouts. The evaluation includes a zero-action policy and the
teacher, so success has a useful lower reference and a checkable attainable
reference:

| Policy | Teacher-forced velocity MSE, `(m/s)^2` | Successful episodes | Worst final axis error |
| --- | --- | --- | --- |
| Zero | `0.057325929036138298` | 0/6 | `2 m` |
| Learned | About `2.95e-34` | 6/6 | About `0.0004775024754 m` |
| Expert | `0` | 6/6 | About `0.0004775024754 m` |

The expert's worst initial error is `2 m`. Thirty saturated ticks reduce it
to `0.5 m`; the remaining 66 ticks multiply it by `0.9` each time. Thus the
independent final-error expectation is `0.5 * 0.9^66 m`, about 478 micrometres.
The learned policy reaches the same rounded result. A zero command leaves the
initial error unchanged. These calculations give us expected output before
running the program.

<!-- source: psionic/examples/imitation-before-robotics-scale.coax compare -->
```jai
    zero:=rk_evaluate(RkPolicy.{});
    learned:=rk_evaluate(fit.policy);
    expert:=rk_evaluate_expert();
    #assert zero.status==RkStatus.ok && learned.status==RkStatus.ok && expert.status==RkStatus.ok;
    #assert zero.successes==0 && learned.successes==6 && expert.successes==6;
    #assert zero.steps==576 && learned.steps==576 && expert.steps==576;
    #assert rk_example_round(zero.mse,1000000)==57326 && learned.mse<1e-20 && expert.mse==0;
    #assert rk_example_round(zero.final_error_max,1000000)==2000000;
    #assert rk_example_round(learned.final_error_max,1000000)==478 && rk_example_round(expert.final_error_max,1000000)==478;
    rk_example_show("zero",zero);rk_example_show("learned",learned);rk_example_show("expert",expert);
```

The [expected transcript](../fixtures/imitation-before-robotics-scale.out)
prints rounded integer metrics so the book does not depend on incidental
float formatting. The numerical checks retain tighter tolerances. It also
exercises a saturated action, an ordinary tick and a refused time step:

<!-- source: psionic/examples/imitation-before-robotics-scale.coax tick -->
```jai
    far:=rk_action(fit.policy,rk_episode(105).state);
    #assert far.status==RkStatus.ok && far.clipped;
    print("saturated action stays within declared speed: %\n",abs(far.velocity.x)<=0.5 && abs(far.velocity.y)<=0.5);
    state:=RkState.{position=RkVec.{x=-0.25,y=0.25}};
    action:=rk_action(fit.policy,state);
    next:=rk_step(state,action.velocity,0.1);
    #assert action.status==RkStatus.ok && next.status==RkStatus.ok;
    print("one fixed tick advances the state: %\n",abs(next.state.position.x+0.225)<1e-10 && abs(next.state.position.y-0.225)<1e-10);
    refused:=rk_step(state,action.velocity,0.2);
    #assert refused.status==RkStatus.time;
    print("wrong tick refuses without changing state: %\n",refused.state.position.x==state.position.x && refused.state.position.y==state.position.y);
```

## Read a small original NVIDIA artifact

NVIDIA's [pinned SingleArm dataset](https://huggingface.co/datasets/nvidia/PhysicalAI-Robotics-Manipulation-SingleArm/tree/73c00c18e7ca108e91846294b20a1d7cd5de9340)
contains Isaac Sim-generated Franka Panda demonstrations. We retain one
28,043-byte original `panda-open-drawer` Parquet episode, its original metadata
and task description, and an inert five-row binary32 export. The
[fixture attribution and manifest](../fixtures/robotics/singlearm-panda-open-drawer/README.md)
pin source revision, file hashes, extraction and CC BY 4.0 attribution.

This is where concrete artifact inspection matters. The card describes 81
state dimensions for this subset, but both its exact `info.json` and the
original Parquet have **25**. Actions have eight channels: seven named joint
position deltas and a gripper channel. Metadata identifies LeRobot v2.1, 30 Hz
and a 133-frame original episode. Our profile checks the names and dimensions
from those files rather than accepting the card's different width:

<!-- source: psionic/lib/robotics/singlearm.coax metadata -->
```jai
sr_info :: (text:string)->SrInfo #export_c {
  if text.count>8192 {return SrInfo.{status=SrStatus.capacity};}
  d:[1]Jv;nodes:[768]JvNode;
  if !jv_parse(d[..],text,nodes[..]) || jv_kind(d[0],0)!=JV_OBJECT {return SrInfo.{status=SrStatus.shape};}
  v:=d[0];features:=sr_member(v,0,"features");states:=SR_STATE_NAMES;actions:=SR_ACTION_NAMES;
  if !sr_string(v,sr_member(v,0,"codebase_version"),"v2.1") || !sr_string(v,sr_member(v,0,"robot_type"),"panda") || sr_integer(v,sr_member(v,0,"fps"))!=30 || sr_integer(v,sr_member(v,0,"total_episodes"))!=1273 || sr_integer(v,sr_member(v,0,"total_frames"))!=154256 || !sr_feature(v,sr_member(v,features,"observation.state"),states[..]) || !sr_feature(v,sr_member(v,features,"action"),actions[..]) {return SrInfo.{status=SrStatus.profile};}
  return SrInfo.{states=25,actions=8,fps=30,episodes=1273,frames=154256};
}
```

The [Coax excerpt reader](../lib/robotics/singlearm.coax) admits a bounded
packet of one to eight rows and at most 8,192 input bytes. Unsigned F32 bit words
preserve recorded values exactly, including signed zero. Values must be
finite and within its inert-data domain `[-1000, 1000]`; these are not robot
joint or command limits. Frame IDs must increase,
and each timestamp must agree with `frame / 30` within `2^-16 s`. This is a
declared binary tolerance, not a claim that rounded timestamps are exact
real-valued multiples of `1/30`. A refused read publishes no partial row.

Our selected frames are 0, 1, 2, 3 and 132. There is a gap before the last
frame; these are not five consecutive actions for a replay. The independent
[development oracle](../tests/robotics/singlearm_reference.py) uses an existing
PyArrow installation to read the **original Parquet**, verifies original hashes
and compares all 170 selected F32 values with both the JSON export and
[independent C tables](../tests/robotics/singlearm-goldens.h). Ordinary native
checks compare the actual Coax decoder's output against those tables. Neither
PyArrow nor a Parquet decoder is part of the shipped implementation or ordinary
fixture checks.

The decoder validates the declared profile and source digest; a digest string
cannot authenticate arbitrary newly supplied row values. The original-file
comparison establishes the provenance of this particular committed fixture.
Nor do field names establish controller scaling, SI units or the physical
meaning of every gripper sign. The owned simulation's scales cannot be applied
to Panda data by guessing. No Panda dynamics, image/depth decoding, VLA
inference, or robot command execution is claimed here.

## Run the chapter

The pure VM example trains only on owned demonstrations:

```shell
coax/bin/coax run psionic/examples/imitation-before-robotics-scale.coax
coax/bin/coax run psionic/examples/imitation-before-robotics-scale.coax --release
```

The [native simulation checks](../tests/robotics/README.md) independently
recompute demonstrations, actions, integration, learned parameters and both
evaluation paths in debug, release and ASan/UBSan. The original-data reader
has its own focused native check:

```shell
psionic/tests/robotics/check-native.sh /tmp/robotics-native
psionic/tests/robotics/check-singlearm.sh /tmp/singlearm-native
```

The simulation, learning, evaluation and data-admission files are authority
roots with focused zero-survivor mutation. Existing tabular-training
dependencies keep their previous qualification. We do not run a large model,
download the full dataset or start Isaac Sim for these checks.

The next robotics chapter can compose a specific GR00T embodiment only after
its actual artifacts, terms, vision/language path, action normalization and
flow-matching execution are qualified. A later simulated control loop must add
observation timing, action chunks, resets and recovery. Our six successes are
a finite owned-controller result; they establish neither real-robot transfer
nor GR00T support.
