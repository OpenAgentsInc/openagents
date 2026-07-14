# Assurance current-system map

Date: 2026-07-13

Status: architecture map; current capability and promise records remain the
factual authorities

## Purpose

AssuranceSpec is a control plane over existing QA and verification machinery,
not a replacement runner, evidence store, or ProductSpec work manager. This
map keeps each source of truth intact while showing how the proposed companion
would compose it.

## Authority chain

```text
ProductSpec intent + AC/EVAL/SM IDs + Related Artifact index
  ├──> Desktop workroom: accepted plan -> packet/lease -> evidence envelope
  │      -> unequal-ref verification receipt -> owner packet disposition
  │
  └──> AssuranceSpec obligation
         -> admitted Assurance Manifest verification unit
         -> native harness / QA Swarm
         -> normalized Assurance Receipt
                ├──> Desktop workroom evidence envelope by exact ref
                └──> approved public-safe Related Artifact link

Assurance/release projection -> separately authorized release decision
Product-promise registry     -> separately authorized public claim transition
```

No downstream layer may silently revise an upstream layer to manufacture
green.

## Existing systems and their roles

| Existing artifact/system | Remains authoritative for | Assurance role |
| --- | --- | --- |
| ProductSpec | Product intent, scope, criteria, success metrics, portable Related Artifact index | Exact subject/item refs and evidence attachment targets; links are not verdicts |
| `@openagentsinc/product-spec` | Current local ProductSpec parsing, validation, stable criterion extraction | Subject parser; upstream `0.19.0` catch-up is still required |
| Desktop ProductSpec workroom loop | Accepted plan, packets, dependencies, leases, evidence envelopes, unequal-ref verification receipts, owner packet disposition | Runtime integration point for Assurance Receipt refs; not authenticated identity, assurance, or release authority |
| Behavior-contract registry | Durable micro-promises and oracle refs | Referenced contract and oracle obligations |
| Eval Suites | Fixture-first evaluation semantics and thresholds | Eval obligations; fractional suite pass never accepts a whole criterion automatically |
| Native unit/component/e2e tests | Framework-specific executable assertions | Existing oracle implementations |
| QA Runner | Commitments, target adapters, normalized verification | One execution adapter family |
| QA Swarm | Scripted execution, exploration, findings, regression distillation | Manifest executor and explorer, never semantic or release authority |
| Seam tests | Real relationships between two artifacts/processes | First-class seam obligations |
| Property/model tests | Generated state/action sequences and minimized counterexamples | Property and model-based obligations |
| TLA+/TLC and other checkers | Bounded formal models | Formal obligations with explicit model boundaries |
| Release preflights | Exact artifact/environment acceptance checks | Release-rung execution units and receipts |
| Receipts and traces | What an exact run observed | Evidence inputs and Related Artifact targets; never product intent |
| Arbiter and `/qa` projections | Evidence-backed read models | Assurance graph visualization only |
| Product-promise registry | Public claims and their transition authority | Informative links; remains sole claim authority |

## ProductSpec boundary

Upstream ProductSpec `0.19.0` now owns a portable Evidence Loop: structured
`AC-*`, `EVAL-*`, and `SM-*` items can carry typed Related Artifact pointers.
The validator rejects nonexistent item targets and warns about unusual
artifact/item pairings. It does not fetch, verify, grade, or freshness-check
the evidence.

OpenAgents' current ProductSpec implementation predates that feature and adds
two dogfood-critical local constraints to the older v0.1 shape:

- positive `spec_revision`;
- unique, author-visible Acceptance Criterion IDs such as `CW-AC-04`.

AssuranceSpec references those IDs for the revision-6 bootstrap. ProductSpec
does not gain test techniques, environments, falsifiers, release policy, or
live verdict fields.

The local package and MVP document are not upstream-`0.19.0` conformant: the
MVP uses Markdown `CW-AC-*` criteria and semantic success-metric IDs, while
portable item-level Related Artifacts require structured `AC-*`, `EVAL-*`, and
`SM-*` items. Catch-up and ID migration must be explicit. Do not silently alias
`CW-AC-04` to `AC-4`.

Assurance subject identity will distinguish:

- an exact document digest, which changes on any edit and preserves provenance;
- a canonical intent digest, which excludes only attachments a typed classifier
  proves are evidence-only. `product_spec` dependencies and consumed
  `tool_metadata` remain intent-bound.

A changed revision, intent digest, targeted item ID, or targeted item meaning
stales proof design. A document-only change refreshes the evidence index
without changing proof intent only after a typed semantic diff proves it is
limited to classified evidence attachments and permitted provenance fields.
Until that dual-digest projection is implemented and conformance-tested, use a
stable public-safe evidence-index path or explicitly rebind; never wave away an
exact digest mismatch.

This is proposed Assurance-layer classification, not current Desktop identity.
The workroom pins the exact ProductSpec document digest; any byte edit puts the
existing run into `revision_mismatch`, while its old receipts remain historical
under the old identity.

See [`PRODUCTSPEC_EVIDENCE_LOOP.md`](./PRODUCTSPEC_EVIDENCE_LOOP.md) for the
normative boundary and migration sequence.

## Desktop ProductSpec workroom loop

The implemented Desktop runtime is distinct from the ProductSpec document and
from upstream Related Artifacts. It currently owns:

- proposed and accepted ProductSpec implementation plans;
- criterion-mapped work packets, dependencies, leases, and terminal state;
- evidence envelopes of kind `test_run`, `behavior_eval`, `artifact`,
  `diff_review`, or `receipt`;
- verification receipts whose `verifierRef` must differ from the host
  `evidenceProducerRef` (currently the active lease executor);
- owner packet dispositions of `accepted` or `waived`, with a reason required
  for waiver.

Its current verification receipt has only the verdict `passed`, and its
`evidenceRef` is opaque. Therefore a typed bridge must dereference and validate
a current `CONFIRMED` Assurance Receipt before issuing an immutable opaque host
ref and requesting packet verification. The bridge separately enforces real
Assurance producer/reviewer policy; the host's unequal refs are not
authenticated identity proof. It must not launder `REFUTED`,
`INCONCLUSIVE`, stale, flaky, unavailable, or infrastructure-failed Assurance
observations through that pass-only path. Those remain visible as evidence and
must block, fail, or await a richer host contract.

Desktop `verified` means the packet's linked evidence passed that host policy.
It does not mean the full ProductSpec item is assured, the owner accepted the
packet, release is allowed, or a public promise is green.

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

- The semantic planner proposes a human-reviewable Assurance Spec through the
  typed boundary in `packages/assurance-spec/src/semantic-planner.ts`. An
  explicit accepted-subject pin must match the exact ProductSpec bytes, and
  planner output must dispose every exact criterion once. Provider/model calls
  are injected outside deterministic parsing and compilation. The committed
  fixture planner intentionally returns `needs_design`; it is a boundary smoke,
  not proof design.
- External review policy admits an exact revision/digest.
- Observer compiles a deterministic manifest.
- QA Runner/native adapters execute exact units.
- QA Swarm can shard those units and explore beyond them.
- A distilled finding becomes durable only through a reviewed oracle or
  AssuranceSpec revision.

The current QA Runner now has an evidence-only Manifest orchestrator for
`apps/openagents-desktop`. It partitions every exact unit once across six typed,
independently budgeted lanes: scripted browser, seeded monkey, LLM explorer,
performance, terminal, and macOS native. Adapters are injected from the existing
runner backends. Missing support, missing arming, execution failure, budget
overflow, and non-exact provider usage produce `INCONCLUSIVE`; they never become
silent skips. Those no-run states carry blockers but no invented report,
artifact commitment, or Assurance Receipt. Each unit with observed adapter
output produces the existing normalized Assurance Receipt plus an independently
digestible lane wrapper carrying exact budgets, arming, provider usage, and
artifact commitments. The lane adapter must match the exact Manifest unit
adapter. This is execution evidence, not
AssuranceSpec admission, review, acceptance, release, or promise authority.

Exploration that cannot be distilled remains `INCONCLUSIVE`. A green run cannot
admit its own plan, accept its own evidence, deploy, or promote a promise.

Semantic planner compilation likewise stops at `proposed`. It copies source
claim snapshots and digests from the checked request rather than planner prose,
rejects stale/missing/duplicate ids and drifted bindings, and cannot review,
admit, execute, verify, release, or change a public promise.

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
7. normalize receipts without erasing native evidence;
8. resolve qualifying Assurance Receipts through a typed immutable bridge, then
   register opaque refs through the existing Desktop workroom loop without
   copying or upgrading their verdicts;
9. publish only reviewed public-safe evidence pointers through ProductSpec
   Related Artifacts after the local `0.19.0` compatibility gate.

The first concrete application is
[`MVP_FIRST_ASSURANCESPEC.md`](./MVP_FIRST_ASSURANCESPEC.md).
