# Weak Devices And Work Classes FAQ

This document closes `OpenAgentsInc/openagents#4320`.

It is the public explanation layer for the current Transcript 222 launch
posture. It should stay aligned with:

- `docs/plans/transcript-222-launch-truth-contract.md`
- `docs/pylon/distributed-training-launch-status.md`
- `docs/reports/pylon/2026-04-12-transcript-222-rehearsal-gates.md`

## Is my weaker device "training the model" right now?

Not in the same way as an island-grade strong node.

The current default weaker-device launch lane is `validation_replay`. That is
real accepted work. It matters to the training run. But it is a
`participation_only` lane, not the default dense local-update lane.

That means a weaker device can be part of the live decentralized training run
without pretending to be the same thing as a strong island doing
model-progress-bearing local updates.

## What does a weaker device do by default?

For the current launch window, the default weaker-device work class is:

- `validation_replay`

That work class is expected to:

- receive real assignments from `Nexus`
- fetch the needed artifacts automatically
- run replay or validation work through the admitted runtime path
- return retained proof, receipt, and closeout evidence
- count toward weak-device assigned and accepted contributor totals
- qualify for accepted-work payout when the work is accepted

## Why did my machine get `validation_replay` instead of the strong lane?

Because work is assigned by capability tier and work-class eligibility, not by
"node is online."

`Nexus` uses the admitted capability envelope to match work against things like:

- backend family
- available memory
- throughput band
- replay capability
- eligible work classes
- eligible replica types

If your machine is a good fit for replay and validation but not for the dense
local-update lane, the honest assignment is `validation_replay`.

## Does weaker-device work still count?

Yes.

It counts in the right bucket:

- assigned contributor counts
- accepted contributor counts
- weak-device accepted-work counts
- accepted-work payout totals

It does not automatically count as:

- model-progress-bearing contribution
- dense full-model local-update training

Those are different counters on purpose.

## What counts as accepted work?

Accepted work is not the same as:

- being online
- launching a local process
- finishing local runtime work with no accepted closeout

Accepted work means the output was accepted into `Nexus` truth through the
relevant closeout, validator, or receipt path and is eligible for the payout
basis of that work class.

## When do sats get paid?

The launch contract is:

- sats follow accepted work
- sats do not follow mere uptime
- accepted-work sats are separate from placeholder or beta counters

So the right sentence is:

> nodes earn bitcoin for accepted work

Not:

> nodes earn bitcoin for being online

## Why are there different counters for online, assigned, accepted, and model progress?

Because those are different facts.

- `online` means a node is present now
- `assigned` means `Nexus` issued real work to that node
- `accepted` means the node has accepted work in retained coordinator truth
- `model progress` means the accepted work belongs to a progress-bearing class

If one number tried to stand in for all of those, the public story would stop
being honest.

## Can weaker devices do more than replay later?

Yes, but that depends on the admitted work classes and the machine contract.

The broader architecture already supports more than one role for smaller or
non-island machines, including things like:

- `evaluation`
- `adapter_training`
- `grouped_replica_stage_execution`

For the current launch window, the default public weak-device proof is still
`validation_replay`. That keeps the claim honest and matched to the retained
evidence.

## How does `a1_minimal_distributed_lm_001` map work classes?

The A1 minimal distributed LM run uses existing work classes:

- tokenization shard validation, validation replay, checkpoint verification,
  proof generation, closeout verification, and artifact rematerialization use
  `validation_replay`
- eval batches use `evaluation`
- tiny local-update training uses `small_model_local_training`
- trusted aggregation uses `aggregation`
- checkpoint promotion uses `checkpoint_promotion`

`validation_replay` and `evaluation` are participation-only. They can count as
participants after Nexus accepts the closeout, but they do not count as
model-progress participants. `small_model_local_training` is the first A1 local
update class and can count as model progress only when the accepted output
enters canonical training state. `aggregation` and `checkpoint_promotion` are
checkpoint-advance classes.

A1 Pylon manifests also carry explicit artifact ids. Support/verifier
assignments upload `support_bundle` artifacts with artifact class `proof`.
Local-update assignments upload `local_update` artifacts. Both use Nexus
signed-access routes; weak devices are not expected to write local-update
artifacts just to count as participants.

## Does every connected node do the same work?

No.

That is the point of work classes and capability tiers. Different machines are
supposed to get different assignments that fit the machine.

The public stats are expected to make that legible instead of hiding it behind
one generic presence number.

## Where should I look to understand what the network is doing?

The current public truth comes from the `Nexus` training and payout surfaces:

- assigned contributors
- accepted contributors
- weak-device accepted contributors
- model-progress contributors
- accepted-work payout totals

Those are the counters that matter for the launch story, not generic online
presence by itself.
