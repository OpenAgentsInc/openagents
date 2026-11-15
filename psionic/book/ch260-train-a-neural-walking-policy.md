# 260. Train a neural walking policy

> **Status: unimplemented — design draft.** This chapter specifies a neural
> actor/critic and PPO update built from shared Psionic components. No neural
> walking result, new optimizer or device-training profile is implemented here.

The scalar LQR experiment in [chapter 171](ch171-learn-a-gait-in-many-worlds.md)
made the learning arithmetic inspectable. A robot needs a vector of action
means and a critic that learns useful values. We now connect those networks
to the collector, retain the old distribution and differentiate the real
objective instead of fitting recorded actions as an accidental substitute.

The first neural integration runs on the analytic plant from chapter 259.
Only a later run on an admitted articulated/contact biped earns a walking
claim. Both use the same learner and carry separate experiment identities.

## A network small enough to audit

Use separate actor and critic programs with one ReLU hidden layer of width 16.
For a proposed 61-observation, 14-action reference profile, the actor has
61 × 16 + 16 + 16 × 14 + 14 = 1,230 affine parameters. Fourteen learned
log-standard-deviations bring it to 1,244. A same-input critic has
61 × 16 + 16 + 16 × 1 + 1 = 1,009 parameters. Total: **2,253**.

That parameter count fits the current 4,096-value checkpoint core, but does
not prove graph, tensor-arena or execution admission. Check both programs
against the 32-node IR and actual operator limits. Begin with four observations
and two actions, then admit the complete reference shape. Do not raise a
compiler budget or fixed capacity just to make the initial example compile.

Construct networks through [`module.coax`](../lib/module.coax), use the shared
[autodiff](../lib/autodiff.coax) and [trainer](../lib/trainer.coax), and extend
their public interfaces only where this consumer demonstrates a missing
operation. ReLU avoids making a new activation a prerequisite. Another policy
architecture or imported graph has its own operator requirements.

## The action is a joint distribution

The first actor uses a diagonal Gaussian over normalized latent actions.
Reuse the existing scalar density and sampling components per coordinate,
then sum log probabilities in a fixed order. Compute **one** probability
ratio per complete action vector, followed by **one** clipped PPO objective.
Clipping each coordinate's ratio defines a different algorithm.

For an illustrative two-action row, take unit standard deviations, zero means
and sampled actions [1, −1]. The score with respect to the two means is
[1, −1]; both log-standard-deviation scores are zero. Suppose the new/old
joint-density ratio is 1.1, the advantage is 2 and epsilon is 0.2:

```text
policy loss             = -min(1.1 * 2, 1.1 * 2) = -2.2
d(loss)/d(new log prob)  = -1.1 * 2 = -2.2
d(loss)/d(mean)          = [-2.2, +2.2].
```

At ratio 1.3 the positive-advantage branch clips, so that policy contribution
has zero gradient under the declared convention. Negative advantages exercise
the other branch. Reuse the existing PPO knot convention; do not reconstruct
branch selection from rounded product equality.

The environment receives the bounded action map, while the policy likelihood
uses the original sampled latent action. Store both. Do not pretend that a
clipped action has the same continuous density as the Gaussian sample.
Report saturation and its effect on learning.

## Backward without inventing another framework

Calculate the vector density's output sensitivities through independently
checked shared components. Feed the resulting mean/value sensitivities into
the neural programs. If the module runner lacks a general seeded VJP, add one
to the shared runner and prove it against a detached weighted-output scalar
loss using existing multiply/sum nodes. Do not hide PPO behind squared-error
training merely because that loss is already convenient.

For the first critic choose half squared error. Value 0.5 and detached target
1.5 give loss 0.5 and derivative −1. Old log probabilities, sampled actions,
advantages, targets and frozen normalization are detached. Specify batch
averaging, value coefficient, entropy coefficient, gradient clipping, optimizer
and shuffle order. Initially set entropy weight to zero; adding an entropy
bonus is a separately checked objective term.

Reuse [`tr_next`, `tr_add`, `tr_apply`](../lib/trainer.coax) and AdamW through
their existing staged update contract. A refused row or nonfinite gradient
publishes no partial update. Checkpoints retain actor/critic, optimizer,
collector/plant state, normalizers, shuffle and counter state. Extend storage
with semantically named versioned robotics records; an existing tokenizer-ID
field is not a place to hide a normalizer identity.

## Learning evidence

Before a learning run, compare forward values, vector-density scores and
selected full-network gradients with hand calculations, finite differences
away from knots and an independent reference. Test duplicated batch rows,
permutations under the declared reduction policy, clipped branches, illegal
ratios and output preservation on refusal.

Freeze three training seeds, update budget, evaluation suite and baselines.
Keep every curve and final artifact, including a run that fails its goal.
The analytic-plant stage checks learning and exact same-profile resume.
The biped stage checks commanded walking on held-out conditions, with actual
contacts and sensors. Report physical steps, action updates and learner
updates separately. Device acceleration follows a passing CPU reference.

[RB-04 through RB-07](../docs/robotics/microduck-implementation-roadmap.md#rb-04-vector-policy-density)
split density, shared backward integration, CPU learning and biped integration
into independent changes. [Chapter 261](ch261-teach-skills-and-switch-between-them.md)
adds skills without turning their selector into an unrecorded second controller.
