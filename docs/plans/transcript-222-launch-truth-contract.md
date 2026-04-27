# Transcript 222 Launch Truth Contract

Status: active  
Date: 2026-04-11

This document freezes the claim boundary for the Transcript 222 launch-hardening
program.

It exists for one reason: Transcript 222 made public statements about
distributed training, participant counts, payouts, weak-device contribution,
and public stats. Those statements are only useful if operators, engineers,
product, and public surfaces mean the same thing when they use the same words.

This is not a generic roadmap note. This is the canonical glossary and claim
sheet for:

- `openagents/docs/transcripts/222.md`
- `openagents/apps/nexus-control`
- `openagents.com` homepage and `/stats`
- payout-linked public counters
- later Transcript 222 launch-hardening issues under `workspace#12`

This document complements
[`compute-market-launch-truth-checklist.md`](./compute-market-launch-truth-checklist.md).
The canonical run contract that later stats, manifests, and closeouts should
reference is
[`compute-training-run-definition-contract.md`](./compute-training-run-definition-contract.md).
The operational launch envelope for those claims now lives in
[`transcript-222-training-launch-slos.md`](./transcript-222-training-launch-slos.md)
and
[`transcript-222-training-incident-taxonomy.md`](./transcript-222-training-incident-taxonomy.md).

## Current Public Truth Surfaces

Today the live public surfaces already expose presence and payout information.
They do not yet expose full training assignment, accepted-work, or
model-progress truth.

Current public stats fields that matter for Transcript 222 language are:

- `pylonsOnlineNow`
- `pylonsSeen24h`
- `pylonSessionsOnlineNow`
- `sellablePylonsOnlineNow`
- `recentPylons[*].readyModel`
- `recentPylons[*].runtimeState`
- `recentPylons[*].eligibleProductCount`
- `recentPylons[*].products`
- `nexusPayoutSatsPaidTotal`

The definitions below freeze what those fields are allowed to mean today and
what they are not allowed to mean yet.

The public Nexus homepage at `nexus.openagents.com/` now also renders WGPUI-led
training panels from the same underlying truth sources:

- `/api/stats` `training_public_state`
- `/api/homepage` `training_visualization`
- `/api/homepage` `training_nodes`
- `/api/homepage` `recent_trn_publications`

Those richer panels are presentational only. They do not create a second claim
contract. Any public run, window, validator, checkpoint, or payout statement on
that surface is still bound to the glossary below.

## Canonical Glossary

### Online node

An `online node` is a distinct provider identity with a fresh provider-presence
heartbeat inside the current Nexus TTL window.

Current public source:

- `pylonsOnlineNow`

An online node is allowed to mean:

- the node is presently visible to the hosted Nexus presence path
- the node has not aged out of the online window

An online node is not allowed to mean:

- the node was admitted to a training run
- the node was assigned real training work
- the node completed any work
- the node contributed accepted progress

### Seen-in-24h node

A `seen-in-24h node` is a distinct provider identity that posted presence
inside the last 24 hours.

Current public source:

- `pylonsSeen24h`

It is historical presence, not current workload truth.

### Session-online count

`pylonSessionsOnlineNow` is a session count, not a participant-count claim.

It is allowed to describe:

- live session cardinality
- session churn or multi-session behavior

It is not allowed to stand in for:

- distinct training contributors
- distinct assigned nodes
- distinct participants

### Sellable-online count

`sellablePylonsOnlineNow` is a compute-market sellable-product count under the
current product and capability posture.

It is not a training-admission metric.

It is not allowed to mean:

- admitted to the Transcript 222 training run
- eligible for the weak-device lane
- eligible for the strong-node training lane

### Admitted node

An `admitted node` is a node that Nexus has explicitly accepted as eligible for
one defined training work class under one defined run policy and capability
policy.

This is stronger than online presence and stronger than generic sellable
compute posture.

An admitted node must be backed by:

- a concrete run-definition or policy binding
- a capability check or policy override
- a durable Nexus decision that can be counted later

Current status:

- this is required launch truth
- it is not yet a public stats field
- it must not be inferred from `pylonsOnlineNow`, `sellablePylonsOnlineNow`,
  `readyModel`, `runtimeState`, or `products`

### Assigned node

An `assigned node` is an admitted node with an active or completed training
assignment, lease, or equivalent work grant for one run or window.

Assigned is stronger than admitted.

Assigned is allowed to mean:

- Nexus issued real work to the node
- the work had explicit inputs and closeout expectations

Assigned is not allowed to mean:

- the work was accepted
- the work advanced model state

### Real work

`Real work` means assigned execution with all of the following:

- explicit run or window context
- explicit inputs or artifact references
- defined output expectations
- retained receipts, proofs, or closeout path
- acceptance or refusal semantics

Real work does not include:

- placeholder liveness payouts
- generic online-heartbeat presence
- undifferentiated "node is connected" status

### Weak-device work

`Weak-device work` means a real assigned lane whose hardware floor is below the
strong dense-training floor.

For launch-hardening purposes, weak-device work is a real work class. It is not
marketing filler. But it still has to be described accurately by what it does.

Default launch interpretation:

- `validation_replay` is the default weak-device lane until a later issue
  explicitly changes that contract

Weak-device work may be:

- supporting work
- accepted work

Weak-device work is not automatically:

- model-progress-bearing work
- dense local-update training

### Launch default weak-device lane

For launch week, the default weak-device lane is `validation_replay`.

That freeze means:

- weak-device launch truth does not depend on grouped-replica stage execution
- weak-device launch truth does not depend on adapter training landing first
- the public weak-device claim is satisfied when weaker nodes receive real
  replay assignments, return retained validator artifacts, and reach accepted
  or payout-eligible closeout under the defined validator path

This is the current honest launch choice because the existing runtime and
handoff work already supports the bounded validator replay path, while
grouped-replica stage execution still belongs to the next expansion step rather
than the minimum launch contract.

The current weak-device floor is therefore:

- enough network posture to reach the admitted training coordinator
- enough durable local storage to materialize the challenged receipt,
  artifact-manifest, and checkpoint family used by replay
- one admitted replay capability posture on the node record

It is explicitly not:

- a claim that every weak device is a dense trainer
- a claim that weak-device work advances canonical model state directly
- a claim that grouped-replica execution is required before Transcript 222 can
  be described honestly

Grouped-replica stage execution and the existing Psionic weak-device accepted
outcome proof remain relevant evidence for the broader architecture, but they
are not the default launch lane and they are not the minimum public proof bar
for next week.

### Accepted work

`Accepted work` is real work whose output has been accepted into Nexus truth by
the relevant closeout, verdict, or receipt path and is eligible for the
documented payout basis of that lane.

Accepted work is stronger than assigned work.

Accepted work must not be inferred from:

- process launch
- local runtime completion alone
- payout placeholder ticks

### Accepted-progress contributor

An `accepted-progress contributor` is a node with at least one accepted work
outcome in the counted run or window family.

This is the internal/backend count family that backs the public
`participants` label. Public launch copy should say `participants`, not
`accepted-progress contributors`.

### Participant

A `participant` is a distinct Pylon/provider identity that completed real
compute work for one defined language-model training run and whose work was
accepted by Nexus closeout truth.

For the first participant-count record, a participant may have completed
model-progress work or verifier/support work, as long as the work was:

- assigned under one run id;
- executed through the Psionic/Pylon training path;
- tied to explicit inputs, expected outputs, artifacts, receipts, and
  closeout;
- accepted by Nexus.

A participant is not:

- an online node;
- a seen-in-24h node;
- a sellable node;
- a presence session;
- a placeholder payout recipient;
- a node that merely downloaded or opened Pylon.

The phrase "by number of participants" is allowed only when "participant"
means accepted real compute work under one run id. It must never be inferred
from online Pylons, seen-in-24h Pylons, sellable Pylons, generic payout totals,
Discord members, downloads, presence sessions, or app sessions.

Internal mapping:

- Public label: participants
- Internal source of truth: `training_accepted_contributors`
- Public label: model-progress participants
- Internal source of truth: `training_model_progress_contributors`

### Supporting work

`Supporting work` is real assigned work that helps the run execute, validate,
recover, or stay auditable without directly changing canonical model state.

Examples:

- validation replay
- proof generation
- closeout verification
- artifact rematerialization used for validator or recovery paths

Supporting work can still be real work and can still be paid.

### Model-progress-bearing work

`Model-progress-bearing work` is accepted work that directly advances canonical
training state.

Examples:

- accepted local-update contribution
- accepted aggregate contribution
- accepted checkpoint promotion step
- any accepted work whose closeout changes the canonical next training state

Supporting work and model-progress-bearing work must stay separate in public
stats and public language.

### Largest-by-online contributors

`Largest by online contributors` means largest by fresh online-node count.

This claim is presence-only.

### Largest-by-assigned participants

`Largest by assigned participants` means largest by distinct assigned-node
count for a defined run or window family.

This claim requires durable assignment accounting.

### Largest-by-participants

`Largest by participants` means largest by distinct participant count for a
defined language-model training run.

This claim requires accepted closeout truth, not presence.

### Largest-by-model-progress participants

`Largest by model-progress participants` means largest by distinct participants
whose accepted work advanced canonical model state.

This is the strongest participant-count claim in the Transcript 222 family.

## Current Field-To-Claim Mapping

The current live public fields support only the following honest statements.

### `pylonsOnlineNow`

Allowed:

- how many provider identities are online now
- how large the current online Pylon network is

Not allowed:

- how many nodes are doing real work
- how many nodes are doing training work
- how many nodes are assigned
- how many nodes are participants

### `pylonsSeen24h`

Allowed:

- how many provider identities were seen in the last 24 hours

Not allowed:

- 24-hour contributor count
- 24-hour assigned count
- 24-hour accepted-work count

### `recentPylons[*].readyModel`, `runtimeState`, `eligibleProductCount`, `products`

Allowed:

- capability hints
- product eligibility hints
- presence-adjacent diagnostic state

Not allowed:

- proof of training admission
- proof of training assignment
- proof of accepted work

### `nexusPayoutSatsPaidTotal`

Allowed:

- total sats dispatched by the hosted Nexus treasury under the current payout
  posture

Not allowed:

- accepted-work payout total
- training-only payout total
- weak-device accepted-work payout total
- strong-lane accepted-work payout total

This number may include placeholder, accepted-work, and beta-bonus payouts and
must still be described that way.

### Split payout fields

Allowed:

- `nexusAcceptedWorkPayoutSatsPaidTotal`
- `nexusPlaceholderPayoutSatsPaidTotal`
- `nexusBetaBonusPayoutSatsPaidTotal`
- `nexusWeakDeviceAcceptedWorkPayoutSatsPaidTotal`
- `nexusStrongLaneAcceptedWorkPayoutSatsPaidTotal`

These fields now expose payout-class truth on the public stats path. They are
the canonical public counters for Transcript 222 payout qualification.

Interpretation:

- `nexusWeakDeviceAcceptedWorkPayoutSatsPaidTotal` covers accepted supporting
  work that is explicitly weak-device-bearing
- `nexusStrongLaneAcceptedWorkPayoutSatsPaidTotal` covers accepted
  progress-bearing strong-lane work such as the current model-update closeout
  lanes

Important qualifier:

- these counters are still settled by the shared hosted Nexus treasury loop
- they are not a second training-only wallet or ledger
- the current launch-default accepted-work basis still fans out from the shared
  `payout_sats_per_window` treasury setting

So the public split is real now, but the settlement substrate remains one
hosted treasury system.

## Canonical Claim Rules

### Public statements that are allowed today

- `The Pylon network has N nodes online right now.`
- `Nexus has seen N distinct Pylons in the last 24 hours.`
- `The hosted Nexus treasury has paid out N sats total.`
- `The current public stats path shows presence and payout truth, not yet full
  training assignment or accepted-work truth.`

### Public statements that are not allowed yet

- `All online nodes are contributing to the training run.`
- `All online nodes are doing real work.`
- `The current payout counter reflects accepted training work only.`
- `Weak-device nodes are advancing the model state` unless that lane is
  explicitly defined as model-progress-bearing and exposed as such.
- `This is the largest decentralized training run in the world` unless the
  statement is explicitly qualified by the counted family and that counted
  family is published.
- `This is the world's largest distributed language-model training run by
  number of participants` unless participants means distinct Pylons/providers
  with accepted real Psionic/Pylon compute work under one run id.

### Required qualifier rule for "largest run" language

Any "largest" claim must explicitly name one of these count families:

- online contributors
- assigned participants
- participants
- model-progress participants

If the qualifier is absent, the claim is not allowed.

If the system cannot publish the counted family, the claim is not allowed.

## Payout Truth Rules

Payout language must follow the operational order defined in the April 9 Nexus
payout continuity analysis:

1. VM-local completed send lines
2. VM-local treasury status
3. public treasury status
4. public stats payload
5. website counters

Transcript 222 launch language therefore must separate:

- payout continuity
- public payout display
- accepted-work payout accounting

Even with explicit payout classes now exposed, public wording must not collapse
those three layers into one number. The generic treasury total, the split
accepted-work totals, and the website presentation remain distinct truth
surfaces.

## Fields Required Before Widening Claims

The public stats contract now exposes these participant-count fields on the
canonical `/api/stats` path:

- `training_admitted_contributors`
- `training_assigned_contributors`
- `training_accepted_contributors`
- `training_model_progress_contributors`
- `training_weak_device_assigned_contributors`
- `training_weak_device_accepted_contributors`

These fields are the current source of truth for participant-count claims. They
must be used instead of inferring contribution from `pylonsOnlineNow`,
`pylonsSeen24h`, `sellablePylonsOnlineNow`, or generic payout totals.

Public labels must map to internal fields as follows:

- `Participants` maps to `training_accepted_contributors`.
- `Model-progress participants` maps to
  `training_model_progress_contributors`.

The public label `participants` is allowed only when it means accepted real
compute work under one run id. It must never be inferred from online Pylons,
seen-in-24h Pylons, sellable Pylons, generic payout totals, Discord members,
downloads, presence sessions, or app sessions.

### A1 Minimal Participant Claim Gate

The machine-readable gate for the first participant-count record attempt is:

- `GET /api/training/runs/<trainingRunId>/claim-gates`

For `a1_minimal_distributed_lm_001`, this endpoint is the canonical
pass/fail report for public "largest" claim language. It deliberately returns
`unqualified_largest_claim_allowed: false` even when a qualified claim passes.

The participant gate maps public language to internal truth as follows:

- Public label: `Participants`
- Internal source of truth: `training_accepted_contributors`
- Minimum target: `201+`

The gate for "largest by number of participants" passes only when all of the
following are true:

- one run id is being evaluated;
- `training_accepted_contributors >= 201`;
- at least `201` distinct Pylon/provider identities have accepted real
  Psionic/Pylon compute receipts for that run;
- Nexus closeout truth accepted the work;
- public run/window/checkpoint lineage exists;
- the A1 minimal distributed LM run definition is present.

The model-progress participant gate maps public language to internal truth as
follows:

- Public label: `Model-progress participants`
- Internal source of truth: `training_model_progress_contributors`
- Minimum target: `201+`

The gate for "largest by number of model-progress participants" passes only
when the participant gate evidence is backed by accepted local-update or
checkpoint-advance work that entered canonical aggregate/promotion lineage,
with a promoted checkpoint ref, validation loss, and retained promotion
receipt.

The gate must not use these excluded counter sources as claim evidence:

- `pylonsOnlineNow`
- `pylonsSeen24h`
- `sellablePylonsOnlineNow`
- `pylonSessionsOnlineNow`
- presence sessions
- generic payout totals
- downloads
- Discord members

The public stats contract now also exposes run/window lineage fields on the
canonical `/api/stats` path under `training_public_state`:

- `training_public_state.default_run_id`
- `training_public_state.default_network_id`
- `training_public_state.active_run_id`
- `training_public_state.active_window_id`
- `training_public_state.default_work_class`
- `training_public_state.work_classes[*].work_class`
- `training_public_state.runs[*].training_run_id`
- `training_public_state.runs[*].current_window_id`
- `training_public_state.runs[*].latest_checkpoint_ref`
- `training_public_state.runs[*].latest_aggregate_ref`
- `training_public_state.runs[*].latest_promoted_checkpoint_ref`
- `training_public_state.windows[*].window_id`
- `training_public_state.windows[*].status`
- `training_public_state.windows[*].base_checkpoint_ref`
- `training_public_state.windows[*].accepted_aggregate_id`
- `training_public_state.windows[*].output_checkpoint_ref`
- `training_public_state.windows[*].promoted_checkpoint_ref`

These fields are the source of truth for Transcript 222 run/window/work-class
and lineage claims. Website and product surfaces should project from this
single payload instead of rebuilding public run state from separate endpoints.

The payout split fields above are now present and should be used instead of
overloading `nexusPayoutSatsPaidTotal`.

## Downstream Issues That Must Use This Contract

These issues should treat this document as the claim vocabulary for their work:

- `openagents#4305`
- `openagents#4308`
- `openagents#4309`
- `openagents#4310`
- `openagents#4311`
- `openagents#4312`
- `openagents#4313`
- `openagents#4315`
- `openagents#4320`
- `openagents.com#10`
- `openagents.com#11`

## Launch-Honesty Failure Modes

- Presence is presented as contribution.
- Assignment is presented as acceptance.
- Accepted supporting work is presented as model progress.
- Placeholder sats are presented as accepted-work sats.
- Website counters are treated as stronger truth than treasury runtime during
  incidents.
- "Largest run" language omits the counted family.

If any of those happen, Transcript 222 truth is broken even if the underlying
runtime is making progress.
