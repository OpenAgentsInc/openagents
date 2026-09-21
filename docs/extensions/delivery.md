# Delivery and evaluation

Status: target implementation and acceptance plan. The
[protocol specifications](../../nips/openagents/README.md) define v1 contracts.
The [consolidated proposed issues](../optimization/proposed-issues.md) hold
the unfiled work for full AI programming and optimization integration.

## Delivery sequence

1. Define semantic contracts, typed operation descriptors, component schemas,
   and exact dependency closure. Resolve them without effects.
2. Implement host binding, grant, presence, and enforcement plans. Require a
   supported mechanism for every hard bound and effect.
3. Run typed programs and supported operations under shared reservations,
   durable outcomes, and explicit verification.
4. Add Wasm packet execution with pure and snapshot-read profiles, bounded
   memory/imports, and retained original evidence.
5. Implement inert package installation, atomic locks, revocation, and scoped
   update/adoption policy.
6. Connect bounded DSPy/GEPA authoring to complete implementation artifacts,
   actual candidate materialization, Gym/domain evaluation, and confirmation.
7. Add local and relay discovery, encrypted artifact exchange, and complete
   client/worker conformance without making network access mandatory.
8. Deliver a complete coding workflow and a non-coding reference profile.
   Evaluate the whole task and expand only where measured value justifies it.

## Acceptance for implementations

### Authority and validation

Reject unknown schemas, effects, strategies, step kinds, imports, and required
bounds. Separate description, presence, selection, and admission. A plugin,
model, compiler, optimizer, package publisher, or evaluator cannot grant itself
authority. Resolve every operation to an independently admitted binding.

Pin executable bytes, argument/configuration assets, schemas, instructions,
examples, consuming policies, model targets, and inference settings. Record
which ones actually load. A valid source or package hash alone does not prove
the candidate under evaluation ran.

### Guest and evidence boundary

Validate packet sizes, memory ranges, allocation ownership, imports, start
functions, and traps. Bound compilation as well as execution. Snapshot handles
are scoped to an invocation; virtual entries do not authorize live API calls
or ambient filesystem access. Missing provenance or incomplete captures stay
visible in derivatives.

Keep originals under retention policy. Context builders include mandatory
constraints and enforce recipient disclosure independently of relevance scores.
A generated representation cannot silently become authoritative source data.

### Lifecycle and recovery

Installation performs no inference, probes, builds, services, or publication.
Interrupted installation preserves the active lock. Active runs retain pins.
Revocation prevents new admission and follows explicit cancellation policy
for active work. Uninstall and cleanup do not reactivate components.

Before effects, durably reserve and record intent. Unknown outcomes retain
their evidence and unsettled budget until reconciliation. A timeout, transport
acknowledgment, or optimizer verdict cannot establish remote nonexecution.

### Optimization and evaluation

Separate task meaning from allowed search surfaces. Protect labels, graders,
confirmation data, grants, and acceptance policies from candidates. Isolate
source/build search and apply the same contract to generated and hand-authored
implementations.

Record the actual algorithm, compiler/exporter, target, loaded candidate, data
access, and complete cost. Selection evidence is development evidence.
Independent confirmation and operator adoption are separate steps. All failed,
refused, cancelled, and unknown attempts remain attributable.

### Surfaces and observability

Interactive and automated clients use the same host admission boundary.
Show component type, active version, unavailable capability, pending approval,
unknown outcome, and verification state in terms useful for the task.
An optional advanced view can expose study, build, and dependency identities.

A selected candidate is not a verified winner. Explain workload scope,
uncertainty, cost, and deployment eligibility. No background finding, public
listing, or high score silently updates running behavior.

## Evaluation and default admission

Establish useful baseline behavior before searching. Evaluate complete tasks
as well as individual operations. Match source inputs, execution conditions,
budget, recipients, and verification, or disclose differences and narrow the
claim. Include harmful errors, abstention, escalation, stale evidence, unknown
effects, and preparation/assimilation cost.

A coding profile needs code acceptance and independent checks. A document or
research profile needs its own source and quality criteria. Schema conformance
does not establish either. Admission thresholds are workload-specific and
supported by an evaluation capable of detecting the intended gain.

The [experiment lifecycle](../optimization/experiments.md) defines partition
access, materialization, accounting, confirmation, and adoption. No paid
experiment or deployment occurs merely because this plan is specified.
