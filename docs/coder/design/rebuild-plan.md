# Coder shared runtime and product-suite plan

Status: target design. The [architecture](typesafe-agent-analysis.md),
[roadmap](typesafe-agent-roadmap.md), and
[AI programming design](../../optimization/README.md) define the agent.
Coder is the first specialization of general agent infrastructure.

The [product-suite delivery plan](typesafe-product-suite.md) applies the full
source proposal and episodes 275–281. Terminal and headless are the current
interfaces; mobile, web, managed workers, optional sync, and Coder OS extend
the same product. Build thin views and host adapters around one task runtime.
Keep agent labor on its [high-priority parallel track](../../agents/market-infrastructure.md).

## Agent structure

Use one host execution path for interactive and automated tasks. The host
owns task state, evidence, authority, scheduling, reservations, effects,
verification, and durable outcomes. The terminal presents and controls that
same task, rather than implementing another agent.

Give native child tasks stable identities before dispatch, with explicit
progress, cancellation, dependencies, and integration. Pairing a device grants
only the selected view or control powers. Execution transfer also requires
fenced ownership, exact artifacts, an admitted destination, and reconciliation
of uncertain effects. A disconnected client must not create a second writer.

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

Load operation descriptors, schemas, and manuals progressively. Known program
steps remain host-driven; generation can propose arguments for a small
admitted tool set during open-ended work. Preserve mandatory scoped instructions
outside optional relevance ranking, and expire skill hooks explicitly.
Compare cache reuse with context reconstruction using complete task cost.
Use typed intermediate state and hierarchical history to avoid repeated
reading without hiding stale or omitted evidence.

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

Carry those views into mobile and web clients with working indicators and
complete retained transcripts. Keep local-only use, trusted-device links,
and hosted sync explicit. Add computer and browser control through admitted
capabilities. Buyers and providers of agent labor use the same task, artifact,
verification, and acceptance views; payment remains separately authorized.

## Delivery and evaluation

Deliver one complete coding workflow before expanding breadth. Exercise the
same semantic and protocol contracts with a non-coding fixture. Evaluate task
success, harmful errors, interruption burden, latency, and full cost.

Deliver deterministic context, discovery, scoped guidance, and task-state
features before searching their policies automatically. Test desktop-to-phone
continuation, cross-client cancellation, duplicate commands, and ownership
loss. Measure accepted throughput across local and remote workers, including
build slots, disk pressure, queue time, briefing, and integration. Background
views share permitted evidence and a lower-priority budget; their cost and
foreground interference count.

The [consolidated proposed issues](../../optimization/proposed-issues.md)
contain optimization dependencies and acceptance. They and the suite plan's
milestones are unfiled planning records; this documentation does not implement
them or schedule paid experiments.
