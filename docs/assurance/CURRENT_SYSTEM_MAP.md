# Assurance current-system map

Date: 2026-07-13

Status: architecture map; current capability and promise records remain the
factual authorities

## Purpose

AssuranceSpec is a control plane over existing QA and verification machinery,
not a replacement runner. This map keeps each source of truth intact while
showing how the proposed companion would compose it.

## Authority chain

```text
ProductSpec criterion
  ↓ exact revision/digest/criterion binding
AssuranceSpec obligation
  ↓ admitted proof intent
Assurance Manifest execution unit
  ↓ exact adapter/environment/command/oracle/falsifier digests
Native harness execution
  ↓ normalized observation
Assurance Receipt
  ↓ independent review + gate projection
Maintainer/release decision
  ↓ separately authorized claim transition
Product-promise registry
```

No downstream layer may silently revise an upstream layer to manufacture
green.

## Existing systems and their roles

| Existing artifact/system | Remains authoritative for | Assurance role |
| --- | --- | --- |
| ProductSpec | Product intent, scope, criteria, success metrics | Exact subject and criterion refs |
| `@openagentsinc/product-spec` | ProductSpec parsing, validation, stable criterion extraction | Subject parser; no QA semantics |
| Behavior-contract registry | Durable micro-promises and oracle refs | Referenced contract and oracle obligations |
| Eval Suites | Fixture-first evaluation semantics and thresholds | Eval obligations; fractional suite pass never accepts a whole criterion automatically |
| Native unit/component/e2e tests | Framework-specific executable assertions | Existing oracle implementations |
| QA Runner | Commitments, target adapters, normalized verification | One execution adapter family |
| QA Swarm | Scripted execution, exploration, findings, regression distillation | Manifest executor and explorer, never semantic or release authority |
| Seam tests | Real relationships between two artifacts/processes | First-class seam obligations |
| Property/model tests | Generated state/action sequences and minimized counterexamples | Property and model-based obligations |
| TLA+/TLC and other checkers | Bounded formal models | Formal obligations with explicit model boundaries |
| Release preflights | Exact artifact/environment acceptance checks | Release-rung execution units and receipts |
| Receipts and traces | What an exact run observed | Evidence inputs; never product intent |
| Arbiter and `/qa` projections | Evidence-backed read models | Assurance graph visualization only |
| Product-promise registry | Public claims and their transition authority | Informative links; remains sole claim authority |

## ProductSpec boundary

OpenAgents' ProductSpec implementation already adds two dogfood-critical
constraints to the upstream v0.1 shape:

- positive `spec_revision`;
- unique, author-visible Acceptance Criterion IDs such as `CW-AC-04`.

AssuranceSpec references those IDs. ProductSpec does not gain test techniques,
environments, falsifiers, release policy, or live verdict fields. A ProductSpec
revision or byte digest change makes its companion stale until explicit
reconciliation.

## Behavior contracts and Eval Suites

A behavior contract stays the durable micro-promise. An Assurance Spec may:

- reference an existing contract;
- require it at one or more environment/proof rungs;
- require evidence that its oracle rejects a falsifier;
- propose a missing contract for separate review.

It may not copy, silently edit, activate, waive, or retire the contract. Eval
Suites follow the same rule. Their fixtures and thresholds remain in their
authoritative homes; an Assurance obligation explains which part of a criterion
they support and what they do not prove.

## QA Runner and QA Swarm

QA Runner and QA Swarm should not become the AssuranceSpec parser or semantic
planner.

- The semantic planner proposes a human-reviewable Assurance Spec.
- External review policy admits an exact revision/digest.
- Observer compiles a deterministic manifest.
- QA Runner/native adapters execute exact units.
- QA Swarm can shard those units and explore beyond them.
- A distilled finding becomes durable only through a reviewed oracle or
  AssuranceSpec revision.

Exploration that cannot be distilled remains `INCONCLUSIVE`. A green run cannot
admit its own plan, accept its own evidence, deploy, or promote a promise.

OpenAgents Desktop now also has a real-Codex `mvp-proof` driver. It is a useful
future execution adapter and receipt source, not AssuranceSpec authority. Its
current script creates a separate two-criterion `FX-AC-*` fixture; it must not
be presented as proof for the canonical MVP ProductSpec's `CW-AC-*` criteria
until exact claim mappings are reviewed and admitted.

## Existing `docs/qa` collection

`docs/qa/` is mostly the retained Khala Code QA execution/evidence collection:
nightly matrices, visual gates, latency budgets, seed corpora, native AX
runbooks, and QA Swarm case-study artifacts. Many paths are referenced by
scripts, tests, reports, and stable documentation links, so consolidation must
preserve those paths.

The collection is useful source material for adapters and obligation patterns,
but it is not current AssuranceSpec authority and does not prove the active
OpenAgents Desktop MVP. `docs/qa/README.md` is the navigation and status index.

## Seam declarations

A seam obligation names:

- both real artifacts or sides;
- the route, protocol, wire contract, or lifecycle boundary;
- the environment and required proof rung;
- the wiring-level oracle;
- a falsifier that breaks the relationship;
- qualifying evidence and verifier-independence policy.

Mock-only tests of both sides do not satisfy the seam. Existing seam incidents
and audits supply calibration examples; the behavior-contract registry retains
the durable seam statement.

## Formal methods

Formal obligations declare the production contract, model boundary, checker,
bounds, invariants, mutation, expected counterexample, and runtime-regression
mapping. A passing model proves only its bounded abstraction. Formal proof does
not accept an implementation or grant release authority.

## Environment and adapter boundary

Environment Profiles state exact target capabilities and safety policy without
secrets. Adapters expose typed `describe`, `validate`, `compile`, `execute`,
`normalize`, and public-projection operations. No adapter interprets ProductSpec
prose or broadens environment authority.

The first dogfood uses one local deterministic Bun profile and one Bun-test
adapter. Effect Native, browser, packaged native, device, staging, and live
adapters remain later obligations, not implied support.

## Status model

Keep these axes separate:

| Axis | Values |
| --- | --- |
| Admission | proposed, admitted, superseded, retired |
| Readiness | needs_design, planned_red, blocked, executable, not_applicable |
| Observation | not_run, CONFIRMED, REFUTED, INCONCLUSIVE |
| Infrastructure | ready, unarmed, unavailable, failed |
| Stability | unknown, stable, flaky |
| Freshness | current, stale |
| Disposition | pending_review, accepted, rejected, exception |

The generated Assurance Manifest contains none of the mutable latest values.
Receipts and projections compute them from exact dependencies.

## Migration rule

Wrap existing machinery before generating replacements:

1. inventory current criteria, contracts, tests, and receipts;
2. import them as proposed obligation/oracle refs;
3. expose missing coverage and weak oracles honestly;
4. prove sensitivity with falsifiers;
5. add new tests only for uncovered obligations;
6. keep every native artifact in its normal owning package;
7. normalize receipts without erasing native evidence.

The first concrete application is
[`MVP_FIRST_ASSURANCESPEC.md`](./MVP_FIRST_ASSURANCESPEC.md).
