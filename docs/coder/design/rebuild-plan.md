# Coder agent and terminal plan

Status: target design. The [architecture](typesafe-agent-analysis.md),
[roadmap](typesafe-agent-roadmap.md), and
[AI programming design](../../optimization/README.md) define the agent.
Coder is the first specialization of general agent infrastructure.

## Agent structure

Use one host execution path for interactive and automated tasks. The host
owns task state, evidence, authority, scheduling, reservations, effects,
verification, and durable outcomes. The terminal presents and controls that
same task, rather than implementing another agent.

Define useful AI operations through semantic signatures and exact
implementations. Typed judgments, generation, retrieval, and bounded
composition are available implementation materials. Model-specific wording
and inference structure can change through measured optimization while
protected control remains stable.

## Typed judgments and generation

Use typed decisions for bounded uncertain questions where they earn their
cost. Preserve abstention, refusal, model identity, and consuming-policy
identity. A probability is neither permission nor proof of correctness.

Use generation for explanations, patches, summaries, and open-ended proposals.
Validate structured outputs against their schemas and check actual operation
arguments at dispatch. Both decision and generation calls share disclosure,
accounting, tracing, and cancellation boundaries.

## Evidence and useful work

Build source captures and task frames with explicit version, scope, retention,
and completeness. Construct recipient-specific context and preserve mandatory
instructions. Native tools perform admitted reads, searches, edits, and checks.
Programs compose those operations under typed dataflow and shared bounds.

Independent acceptance checks exact artifacts and source state. Keep completion,
verification, and integration separate. Preserve unknown outcomes through
recovery rather than guessing that a timed-out effect did not happen.

## Learning and adoption

DSPy/GEPA authoring runs through a bounded bridge that emits supported inert
definitions and assets. Product execution remains Rust. A foreign runtime
object cannot bypass package validation or become executable on installation.

Materialize each exact candidate, measure it through Gym and domain evaluators,
confirm under a frozen policy, and adopt an eligible pin through host policy.
Active tasks retain their admitted implementation. Source or composition
search cannot modify the grader, labels, grants, or protected control.

## Terminal experience

Provide a readable conversation, clear composition and cancellation controls,
bounded scrollback, and consistent evidence/approval/result views. Show current
task intent, active work, budget, sources, and verification. Render partial
output as provisional and preserve a complete final artifact when available.

Explain selected evidence and omitted coverage without presenting relevance
as model attention. Distinguish a candidate, a confirmed result, an adopted
version, and an unknown outcome. Keep technical compiler/protocol details in
an optional inspection view.

## Delivery and evaluation

Deliver one complete coding workflow before expanding breadth. Exercise the
same semantic and protocol contracts with a non-coding fixture. Evaluate task
success, harmful errors, interruption burden, latency, and full cost.

The [consolidated proposed issues](../../optimization/proposed-issues.md)
contain implementation dependencies and acceptance. They are unfiled planning
records; implementation and paid experiments are deferred.
