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
- distinct accepted contributors

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

This is the minimum count family required before claiming "contributors to the
run" in a strong sense.

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

### Largest-by-assigned contributors

`Largest by assigned contributors` means largest by distinct assigned-node
count for a defined run or window family.

This claim requires durable assignment accounting.

### Largest-by-accepted contributors

`Largest by accepted contributors` means largest by distinct
accepted-progress-contributor count for a defined run or window family.

This claim requires accepted closeout truth, not presence.

### Largest-by-model-progress contributors

`Largest by model-progress contributors` means largest by distinct
contributors whose accepted work advanced canonical model state.

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
- how many nodes are accepted contributors

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

Until the payout path is split explicitly, this number may include placeholder
or liveness payouts and must be described that way.

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

### Required qualifier rule for "largest run" language

Any "largest" claim must explicitly name one of these count families:

- online contributors
- assigned contributors
- accepted contributors
- model-progress contributors

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

Until explicit payout classes exist, public wording must not collapse those
three into one number.

## Fields Required Before Widening Claims

The public stats contract must grow these fields before stronger Transcript 222
claims are allowed:

- admitted contributor count
- assigned contributor count
- accepted contributor count
- model-progress contributor count
- weak-device assigned contributor count
- weak-device accepted contributor count
- accepted-work sats total
- placeholder sats total
- beta-bonus sats total
- active run id or run family
- active window id or window family

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
