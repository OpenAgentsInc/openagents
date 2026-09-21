# Proposed issues for full integration

Status: unfiled proposals. These IDs are local planning labels, not GitHub
issue numbers. Implementation is deferred. This is the consolidated backlog
for the [architecture](architecture.md), [experiment lifecycle](experiments.md),
and [OpenAgents protocol set](../../nips/openagents/README.md).

The release target is one complete, attributable loop: define a semantic task,
materialize candidate implementations, evaluate them under a frozen study,
confirm the selected candidate, and adopt an eligible version under operator
policy. Coding is the first workload; a non-coding fixture exercises the same
contracts. More optimizer APIs alone do not meet that target.

## OPT-01: Define semantic contracts and complete implementation identity

**Scope:** NIP-OPT, PRG, EXT, shared contracts; Rust domain types and validators.

Implement semantic AI signatures, target identities, supported implementation
entries, and bounded dependency resolution. Distinguish semantic AI signatures,
operation descriptors, and cryptographic signatures in APIs and interfaces.

**Acceptance:** Round-trip valid fixtures; reject signature/schema drift,
unknown fields/strategies, floating dependencies, cycles, and missing assets.
Changes to instructions, examples, policies, renderers, model targets, or
inference configuration change the appropriate implementation identity.
Validation is inert and grants nothing. Include a non-coding signature.

**Dependencies:** None.

## OPT-02: Enforce study plans, data rights, and exposure accounting

**Scope:** NIP-OPT, CTX, POL; study admission and protected data access.

Implement frozen study/search-space/data plans and independent rights for
evaluation, reflection, prompt/example optimization, training, and export.
Authorize each recipient and retain restrictions on derivatives.

**Acceptance:** Search, selection, confirmation, and exclusions are exact,
nonoverlapping memberships with declared leakage groups and exposure. A
proposer cannot read confirmation labels or grader state. Concurrent workers
cannot spend one confirmation allowance twice. Reusing results for tuning
records exposure and cannot reset it by renaming the suite.

**Dependencies:** OPT-01.

## OPT-03: Build a bounded DSPy and GEPA authoring bridge

**Scope:** Offline infrastructure, compiler/exporter operations, CAP and EXT.

Pin upstream dependencies and implement adapters from semantic contracts and
admitted data to DSPy programs and GEPA evaluation interfaces. Support an
explicit initial set of instruction, demonstration, and parameter surfaces.
Keep authoring, optimization, and inference execution as separate roles.

**Acceptance:** Export a complete inert implementation and provenance record
that the Rust host can validate and execute. Reject unsupported modules,
hidden defaults, ambient tool access, executable deserialization, and missing
dependencies. Demonstrate that the named algorithm actually runs. A simple
manual/search baseline uses the same runner. No Python product runtime or
new always-on optimization service is required.

**Dependencies:** OPT-01, OPT-02.

## OPT-04: Materialize and execute the exact candidate

**Scope:** NIP-OPT, CAP, PRG, CJ; host dispatcher and execution bindings.

Implement AI implementation invocation, complete loaded-asset identity,
model/recipient recording, and schema/effect enforcement for the supported
decision-function, program, and operation entries.

**Acceptance:** A changed-surface canary demonstrates actual activation.
Executing the seed while naming a candidate fails attribution. Wrong model,
renderer, configuration, or dependency is rejected or explicitly unverifiable
under assurance policy. Remote claims remain distinguishable from local
verification. Unsupported strategies fail before effects. The same output
schema does not bypass the semantic or authority checks.

**Dependencies:** OPT-01; OPT-03 for exported fixtures.

## OPT-05: Connect Gym and domain evaluators to frozen studies

**Scope:** NIP-EVAL, OPT; suite adapters, metrics, comparison, and admission.

Map suite/label, question/implementation, and acceptance identities independently.
Map partitions by exact membership and access purpose. Add whole-task outcomes,
per-family errors, abstention, calibration where relevant, and cost/latency.

**Acceptance:** Module and system objectives are distinct; a local gain cannot
mask a failed task. Missing results and failed attempts retain their declared
denominators. The evaluator refuses mismatched candidates and contaminated
confirmation claims. Model judges are pinned and isolated from candidates.
Uncertainty and measurement headroom inform workload-specific acceptance;
no universal threshold is embedded. Rejudging recorded evidence is labeled
separately from executing a new candidate.

**Dependencies:** OPT-01, OPT-02, OPT-04.

## OPT-06: Account for and recover the complete study

**Scope:** CAP, RUN, COORD, CJ, OPT; reservations and durable controllers.

Bound proposal, reflection, student, judge, build, tool, storage, and cleanup
work. Persist intent, candidate/trial identity, receipts, and unresolved usage.
Support cancellation and resume without resetting study or exposure state.

**Acceptance:** Exhausting a shared allowance stops new dispatch. Hidden
reflection calls cannot escape accounting. Unknown charges remain reserved
until reconciled. Crash/cancellation fixtures preserve candidates, negative
outcomes, and confirmation consumption. Retransmissions do not run twice;
independent repetitions are never coalesced into one measurement. Cache policy,
execution order, and environment resets are observable.

**Dependencies:** OPT-02, OPT-04, OPT-05.

## OPT-07: Implement pure protocol and relay conformance

**Scope:** Every OpenAgents NIP and the shared contracts.

Implement the complete v1 schemas and role-specific validation, encrypted
artifact transport, public releases/revocations, evaluation publications,
discovery, and job families. Maintain a fixture matrix per protocol and role.

**Acceptance:** Validate signature/recipient/request binding, exact references,
strict fields, bounded parsing, and conflicting records. Private artifacts
remain private under subscription, ID lookup, COUNT, search, and live fanout.
Relays do not claim semantic or runtime enforcement. Only configured, tested
roles are advertised. Exercise all NIP-OPT records through local storage and
the shared artifact envelope, with no optimizer-specific event kinds.

**Dependencies:** OPT-01, OPT-02; OPT-04–06 for host/worker conformance.

## OPT-08: Adopt, observe, and roll back eligible implementations

**Scope:** EVAL, EXT, POL; installation, operator policy, and client surfaces.

Implement distinct selection, confirmation, release, installation, and adoption
states. Bind adoption to exact implementation, workload, model assurance,
recipients, and expiry. Show active identity and the evidence for its scope.

**Acceptance:** A successful optimizer cannot activate itself. Adoption affects
new admissions; in-flight work keeps its pin. Failed installation preserves
the active eligible version. Rollback refuses revoked/ineligible versions.
Shadow/canary work has explicit data and budget authority. Interfaces explain
no improvement, insufficient evidence, unknown cost, and model drift without
presenting a selected candidate as a verified winner.

**Dependencies:** OPT-05–07.

## OPT-09: Optimize context, retrieval, and representations

**Scope:** CTX and semantic implementations for evidence preparation.

Compare deterministic retrieval, typed per-item judgments, joint selection,
and bounded expansion under a common evidence contract. Include state-update
proposals, summaries, and history representations where supported.

**Acceptance:** Mandatory instructions and provenance survive every candidate.
Measure retrieval versus selection recall, sufficiency failures, expansion,
staleness, disclosure, and full-task outcomes. An unavailable representation
has an explicit fallback/refusal. Token reduction alone cannot pass adoption.

**Dependencies:** OPT-03–08.

## OPT-10: Optimize operation selection, routing, and inference strategies

**Scope:** CAP, PRG, EXT, POL; selectors and model-backed operations.

Search descriptor rendering, optional guidance, shortlisting, model choice,
escalation, batching, and supported generation/inference strategies. Preserve
the semantic contract and mechanical eligibility checks.

**Acceptance:** Evaluate missed useful operations, false activation, argument
errors, cache observations, failed escalations, task quality, and total cost.
A new strategy cannot widen recipients or tool grants. The same signature
works with at least two supported realizations without forcing every task
through a decision-only answer vocabulary.

**Dependencies:** OPT-03–08.

## OPT-11: Search bounded composition and source implementations

**Scope:** PRG, OPT; protected graph constraints and isolated build execution.

Allow explicitly admitted topology or source surfaces after the simpler
artifact loop is verified. Materialize complete candidates, validate dataflow
and bounds, and build candidate code away from evaluator/control authority.

**Acceptance:** Reject graph cycles, hidden recursion, deleted approval/effect
checks, altered labels/graders, changed protected files, and undeclared effects.
Parallel children share reservations and require independent final checks.
Record build failure as an outcome. A broader search must justify its extra
development cost through the same scoped confirmation policy.

**Dependencies:** OPT-06–10.

## OPT-12: Complete coding and non-coding reference studies

**Scope:** Domain adapters and whole-system evaluation.

Deliver one coding workflow and one document/research workflow with semantic
contracts, hand-authored baselines, frozen studies, actual compiled candidates,
independent confirmation, and operator adoption or an honest no-win result.

**Acceptance:** Both traverse the same contract lifecycle. Publish only
explicitly cleared artifacts; private evidence stays private. Reports include
search costs, runtime quality/cost, all failed attempts, and limits of the
claim. Non-coding support is demonstrated without Git/shell assumptions.
No performance claim is required for conformance; evidence is required for
an improvement claim.

**Dependencies:** OPT-03–10; OPT-11 only if the study admits those surfaces.

## OPT-13: Define additional learning and lifecycle profiles

**Scope:** Separate designs for weight training, online adaptation, workload
drift, richer media, event-triggered studies, and cross-organization learning.

Specify these only with explicit data rights, disclosure, effect guarantees,
resource limits, exposure accounting, and independent evaluation. Reuse the
same identity and adoption boundaries. Durable waits and external business
effects need their own domain contracts; an optimization loop cannot supply
them by implication.

**Acceptance:** Each proposed profile names its threat/effect boundary,
supported host enforcement, wire additions if needed, and measurable acceptance.
No background traffic collection, automatic training, or federated aggregation
is enabled by a generic optimization grant.

**Dependencies:** OPT-12 before expanding the production scope.

## Delivery order

Deliver OPT-01–06 as a local, inspectable loop, then complete interoperable
transport and adoption through OPT-07–08. OPT-09–10 apply that loop to useful
agent behavior. OPT-11 expands the search space; OPT-12 establishes complete
domain examples; OPT-13 requires separate scoped decisions.

Documentation changes require prose, link, and contract consistency checks.
Implementation checks target the actual changed behavior and required
conformance. Paid inference, publication, and deployment are separately
authorized work, not effects of accepting this plan.
