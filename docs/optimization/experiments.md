# Experiments, Gym, and promotion

Status: planned integration, not a new benchmark result.

## Measurement identities and partition roles

Keep suite items and labels, implementation configuration, and acceptance
policy independently identified. NIP-OPT pins the complete implementation:
instructions, demonstrations, question sets, adapters, inference settings,
model targets, and executable code. Link those identities to Gym measurements
and domain evaluation evidence. Decision accuracy alone cannot evaluate a
complete agent.

A Gym adapter must map suite partitions to their actual study roles. Names
such as development, calibration, and locked are insufficient: record exact
membership and actual access. A partition used to fit probabilities or choose
a candidate is development evidence for that study. EVAL development contains
search and selection; held-out contains reserved confirmation cases.

Use one authoritative ledger for confirmation access and exposure. A
distributed runner needs serialized consumption of the same allowance;
independent local file locks do not coordinate separate machines.

## Admit a concrete study

Freeze the semantic task, baseline, allowed changes, data rights, metric and
grader identities, acceptance policy, target environment, and all budgets.
Establish measurement headroom and uncertainty before spending on search.
Specify whether the goal is accuracy, harmful error reduction, latency,
cost, or a declared constrained tradeoff. A model judge is another pinned
component with limitations, not ground truth by definition.

Separate student inference, proposal/reflection inference, and grading.
Each can have a different model, recipient, cost, and disclosure policy.
Private source traces cannot go to an external reflector because the
student model was already allowed to see them. Reflection can use authorized
inputs, outputs, tool observations, and diagnostic feedback; private model
chain-of-thought is neither required nor a portable evidence contract.

The proposer receives only admitted development material. Candidates cannot
read the grader's expected answers, alter the runner, expand tool grants,
or lower an acceptance threshold. Keep full-system success and adverse
outcomes visible even when a cheap module metric helps guide search.

## Execute what changed

Materialize a fresh immutable candidate. Record the full loaded closure,
host executable, model target and observed identity, run configuration, and
input/context identities. Verify that it is the candidate proposed, before
attributing the score to it.

A candidate filename or label does not establish activation. Use a canary
fixture that proves the runner reads the changed surface, plus identity checks
for every trial. Scoring an unchanged seed cannot establish a candidate's
quality. A remote receipt remains an attributable claim at its stated assurance.

Record cache configuration and hits, environment resets, randomization,
attempt ordering, failures, and provider changes. Interleave or otherwise
control comparisons where temporal effects matter. Never treat cached results
or deduplicated repetitions as independent fresh samples.

Track proposal, reflection, student, grader, tool, build, storage, and cleanup
costs under the study's shared reservation. Unknown cost remains unknown.
Record both total development cost and expected runtime cost; metric-call
counts are insufficient to establish either.

## Select, confirm, and adopt

Selection data is part of adaptive search. Commit the selected candidate,
comparison, and confirmation policy before exposing reserved confirmation
evidence. Count confirmation reads and downstream use of aggregate feedback.
A failed confirmation does not permit endless retries against the same
“unseen” cases.

Acceptance includes uncertainty, per-family regressions, adverse error
directions, and domain outcomes. A pass on a local module may still fail the
whole task. A study can produce no improvement, insufficient evidence, or an
uneconomical improvement; all are useful results.

Promotion is a separate host/operator decision scoped to the exact
implementation, workload, model target, recipients, and time/expiry policy.
Use a new EXT release or local pin, optional separately admitted shadow/canary
work, and a still-eligible rollback target. Running tasks retain their pins.
A provider change or meaningful workload drift triggers requalification under
policy; a stable API schema does not establish stable quality.

Online adaptation, federated learning, automatic data collection, and weight
training are not implied. They need their own approved objectives, authority,
privacy, budgets, and evaluation. The initial integration is a bounded
development loop, with no automatic rewriting of production behavior.

## The first complete experiment

Choose one operation with a known failure population and measurable headroom.
Include a hand-authored baseline and a simple search baseline alongside a
pinned DSPy or GEPA optimizer. Keep the experiment small enough to inspect
every loaded candidate, cost, and failure. No paid experiment is authorized
by this documentation.

Then run a complete coding workflow and a non-coding fixture through the same
contracts. The non-coding case proves protocol generality; it does not claim
production readiness. Expand to strategy and composition search only after
the candidate/export/evaluation/promotion loop is real.

Acceptance thresholds and minimum detectable effects belong to the workload,
sampling design, and frozen policy. Do not prescribe one universal improvement
floor or assume that a small suite can distinguish the desired gain.
