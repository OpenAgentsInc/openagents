# AssuranceSpec — a ProductSpec companion for committed proof design

Date: 2026-07-13

Status: design proposal; no format, compiler, adapter, or hosted service is
claimed to exist

Reference implementation studied:
`projects/repos/ProductSpec` at `833d67d` (repository `0.7.0`, document format
`spec_format_version: "0.1"`)

Related episode: `docs/transcripts/252-notes.md`

Working OpenAgents product/compiler name: **Observer**

## 0. Decision

Create **AssuranceSpec**, a separate, human-reviewable companion to
ProductSpec.

- A **Product Spec** commits product intent: what should be true and why.
- An **Assurance Spec** commits proof design: what evidence would justify
  believing each relevant claim, in which environments, against which
  falsifiers, at which proof rung.
- An **Assurance Manifest** is the deterministic, immutable execution IR
  compiled from admitted source artifacts.
- **QA Swarm** and native test tools execute the manifest and explore beyond
  it.
- **Assurance Receipts** report observations. They do not revise either spec.
- Authorized maintainers accept, reject, waive, release, or revise claims.
- The product-promise registry remains the only authority for public claims.

The proposed authored filename is `<name>.assurance-spec.md`. The public
protocol is AssuranceSpec; **Observer** is the OpenAgents semantic planner,
compiler, and product codename. A future multi-project evidence surface can be
called **Observatory**. Product branding must not become protocol vocabulary.

```text
ProductSpec
  committed product intent
        ↓ exact criterion bindings
AssuranceSpec
  committed verification intent
        + Environment Profiles + adapter lock
        ↓ admission review
Observer deterministic compiler
        ↓
Assurance Manifest
  immutable execution lockfile
        ↓
behavior contracts / Eval Suites / native tests / formal models
  executable oracles
        ↓
QA Swarm + framework adapters
  execution and exploration
        ↓
Assurance Receipts + evidence aggregation
        ↓
release projection → authorized decision
```

This is a proposal to rebuild the QA harness *around* a proof-design control
file, not to discard the harnesses that already work.

## 1. Why this is a companion rather than a ProductSpec section

The ProductSpec reference repository has the right architectural precedent:
Decision Trace is a separate companion artifact. ProductSpec remains small,
portable committed intent; the companion records a different concern with a
different lifecycle.

Assurance is also a separate concern:

- product intent and proof design change for different reasons;
- one Product Spec can have multiple assurance profiles without changing the
  product claim;
- QA techniques, environments, falsifiers, permissions, and evidence policies
  are implementation-adjacent and often organization-specific;
- a generated JSON manifest alone would hide semantic judgment from reviewers;
- a `custom-assurance` section would round-trip but have no portable semantic
  contract;
- `tool_metadata` is private/export-stripped and is not a safe home for
  normative proof obligations;
- pretending prose-to-test planning is deterministic would make model judgment
  look like compiler output.

The boundary is therefore:

```text
ProductSpec says: "the signed-in user can resume the same thread."
AssuranceSpec says: "these observations would establish or refute that claim."
Manifest says: "run these exact admitted units against these exact targets."
Receipt says: "this is what this run observed."
Decision says: "given the evidence and authority policy, this may advance."
```

AssuranceSpec may reference ProductSpec criteria. It may never rewrite them.
If an assurance review discovers ambiguous or impossible product intent, the
result is a ProductSpec reconciliation, not an inventive interpretation in the
QA layer.

### Alternatives considered

| Shape | Useful property | Fatal ambiguity |
| --- | --- | --- |
| ProductSpec custom section | One file, immediately round-trips | Mixes product intent with enforcement intent; QA churn ambiguously changes `spec_revision` |
| New ProductSpec QA block | Strong parsing and locality | Makes the core product-intent standard framework- and environment-aware |
| Generated manifest only | Strictly executable | Hides semantic judgment and encourages generated-oracle authority |
| New independent standard immediately | Clean governance | Premature before dogfood and an independent implementation |
| **Incubated companion + generated manifest** | Human-reviewable source, independent revisions, deterministic IR, portable adapters | Requires explicit reconciliation, which is a feature rather than a shortcut |

Incubate AssuranceSpec inside OpenAgents. Publish it as an independent standard
or propose an upstream companion only after one OpenAgents dogfood project and
one non-OpenAgents implementation demonstrate portability.

## 2. Design laws

1. **Intent and proof design are independent authorities.** A QA artifact
   cannot weaken product intent to obtain green.
2. **Semantic planning is reviewable.** Agents may propose obligations, but
   model output is not deterministic compiler output and not admitted policy.
3. **Compilation is pure.** Identical admitted inputs and compiler version
   produce byte-identical manifests.
4. **Oracles must demonstrate sensitivity.** A required oracle names a
   falsifier, known-bad fixture, mutation, or counterexample that it rejects.
5. **Seams are first-class objects.** Two green component tests do not prove a
   connection between real sides.
6. **Evidence is environment-bound.** A fixture pass cannot silently count as
   release, device, staging, or live proof.
7. **Status axes do not collapse.** Admission, readiness, observation,
   infrastructure, stability, freshness, disposition, and exception remain
   separate.
8. **Unsupported means typed gap.** It never means skip-and-green.
9. **Formal proof is bounded.** A checker proves the stated model boundary and
   grants no runtime or release authority.
10. **Receipts report; people and policy decide.** No manifest, test, swarm, or
    verifier can approve its own work.
11. **Public evidence is a projection.** Private source, targets, prompts,
    credentials, screenshots, traces, and customer data remain private unless
    separately approved and redacted.
12. **Conformance is interoperability, not quality.** A valid Assurance Spec
    can still contain a weak proof plan; a conforming runner can still observe
    a failure.

## 3. The v0.1 authored document

AssuranceSpec should copy ProductSpec's best structural decision: readable
Markdown for reasoning, bounded structured blocks for things machines must
compare or execute.

### 3.1 Serialization

```text
<name>.assurance-spec.md
  ├── YAML-like frontmatter
  ├── mandatory ordered `##` sections
  ├── optional canonical and `custom-*` sections
  └── typed fenced blocks inside the relevant sections
```

The parsed semantic model—not raw Markdown—is the schema target. A reference
parser and serializer must preserve unknown valid custom sections and support
exact parse → serialize → parse semantic round trips.

Suggested frontmatter:

```yaml
---
assurance_spec_format_version: "0.1"
assurance_spec_id: "openagents-observer"
assurance_revision: 1
title: "OpenAgents Observer Assurance Spec"
artifact_type: "product_assurance"
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    title: "Owner Gates"
    after: "authority_boundaries"
tool_metadata:
  planning_issue: "#..."
---
```

Like OpenAgents Product Specs, normative public fields remain flat and
bounded. Nested machine data belongs in typed fenced blocks. `tool_metadata`
is non-normative, stripped from public exports, and never contains secrets or
private customer data.

### 3.2 Mandatory sections

The proposed mandatory section IDs, in order, are:

1. `assurance_objective` — what confidence this artifact is designed to
   establish, and what it explicitly cannot establish;
2. `subject` — exact source ProductSpec and other normative subject bindings;
3. `risk_model` — harms, forbidden outcomes, failure classes, and applicable
   invariants;
4. `assurance_scope` — in/out/cut across surfaces, seams, environments, proof
   rungs, and assurance domains;
5. `environments` — required environment-profile references and capability
   constraints;
6. `obligations` — the criterion-to-proof graph;
7. `gates` — activation and aggregate release-proof expressions;
8. `evidence_policy` — required evidence, freshness, independence, retention,
   and public-safety rules;
9. `authority_boundaries` — who may admit, verify, waive, accept, release, and
   change public claims.

Suggested optional canonical sections:

- `oracle_design`
- `behavior_contracts`
- `product_promises`
- `test_data`
- `formal_models`
- `observability`
- `security_and_privacy`
- `human_evaluation`
- `exception_policy`
- `known_gaps`
- `open_questions`
- `rollout`
- `hosted_data_policy`
- `custom-<kebab-name>`

`assurance_scope` is deliberately separate from ProductSpec scope. ProductSpec
scope answers what product is being built. Assurance scope answers which
claims, risks, surfaces, environments, and proof rungs this proof plan covers.
It cannot declare a ProductSpec item out of scope; it can only expose an
assurance gap or reviewed not-applicable disposition.

### 3.3 Structured blocks

Proposed fenced-block identifiers:

- `assurancespec-subject`
- `assurancespec-risks`
- `assurancespec-environments`
- `assurancespec-obligations`
- `assurancespec-seams`
- `assurancespec-formal-models`
- `assurancespec-gates`
- `assurancespec-evidence-policy`
- `assurancespec-authority`

Do not create one enormous nested block. Each block should have a narrow schema
and a stable location. Human-readable prose explains the reasoning; the block
contains only fields that a validator, compiler, reviewer, or projection must
compare.

## 4. Exact subject and criterion binding

An Assurance Spec is meaningless if it can drift to a different subject while
retaining green evidence. The subject block must pin:

```yaml
product_spec:
  path: "specs/qa/openagents-observer.product-spec.md"
  spec_format_version: "0.1"
  spec_revision: 1
  digest: "sha256:<digest-of-exact-utf8-bytes>"
criterion_refs:
  - "OBS-AC-01"
  - "OBS-AC-02"
```

The canonical digest is SHA-256 over the exact UTF-8 file bytes committed for
the cited revision. The compiler records byte-normalization policy explicitly;
it never resolves “latest” during compilation.

OpenAgents already requires positive `spec_revision` and unique,
author-visible acceptance-criterion IDs such as `OBS-AC-01`. AssuranceSpec
uses those IDs.

Upstream ProductSpec v0.1 ordinary Acceptance Criteria are Markdown, not a
generic criterion AST with stable IDs. A compatibility importer may propose a
binding containing:

```yaml
section_id: "acceptance_criteria"
text_anchor: "Private or unsupported videos return a clear error."
source_digest: "sha256:<product-spec-digest>"
```

That proposal is stale on any subject revision, digest, or anchor change and
must be admitted by a reviewer before compilation. It may never silently fuzzy
match a changed sentence. For an assurance-enabled Product Spec, the preferred
resolution is to add stable author-visible IDs without changing the upstream
core format.

A possible later upstream contribution is a structured
`productspec-acceptance-criteria` block with stable `id` and `claim` fields.
That proposal should be made only with the upstream change discipline: SPEC,
schema, parser, validator codes, field guide, valid/invalid fixtures,
round-trip tests, and changelog in one compatible change.

## 5. The assurance obligation model

An obligation is a reviewed proof claim, not a generated test filename. One
criterion may require many obligations; one obligation may support several
criteria when the relationship is explicit.

Minimum normative fields:

```yaml
- id: "OBS-AO-001"
  title: "Real WebSocket authentication seam reaches live"
  criterion_refs:
    - "OBS-AC-06"
  risk_refs:
    - "OBS-RISK-003"
  contract_refs:
    - "khala_sync.seam.cookie_less_bearer.v1"
  invariant_refs: []
  promise_refs: []
  disposition: "required"
  domains:
    - "seam"
  technique: "deterministic_e2e"
  environment_refs:
    - "ENV-STAGING-SYNC"
  oracle:
    statement: "A cookie-less bearer client completes the real upgrade and reaches live."
    evaluator_ref: "tests/live-seam-smoke.e2e.test.ts"
  falsifier:
    kind: "wrong_client_server_pair"
    ref: "fixtures/ws-header-only-server"
    expected_verdict: "REFUTED"
  evidence:
    required_kinds:
      - "execution_trace"
      - "seam_receipt"
    proof_rung: "live_staging"
  independence:
    producer_may_verify: false
  dependency_refs: []
  activation_gate: "GATE-INTEGRATION"
```

Allowed disposition is one of `required`, `optional`, or `not_applicable`.
`not_applicable` requires a rationale and an admitted review. `blocked` and
`needs_design` are readiness projections, not ways to erase an applicable
obligation.

Suggested technique vocabulary:

- `static`
- `unit`
- `component`
- `property`
- `model_based`
- `contract`
- `seam`
- `browser`
- `native`
- `device`
- `accessibility`
- `visual`
- `performance`
- `resilience`
- `security`
- `formal`
- `exploratory`
- `human_evaluation`
- `manual`

Technique names describe proof shape, not a specific framework. Vitest,
Playwright, XCTest, Maestro, Rust `proptest`, TLC, Apalache, and customer-native
tools are adapter implementations.

### 5.1 Assurance-domain coverage

Episode 252's A0–A9 catalog remains the risk-discovery checklist. A criterion
does not need all domains. Every applicable domain needs either an obligation
or an explicit reviewed `not_applicable` disposition. Line and branch coverage
remain advisory because they measure executed code, not claim coverage.

Three coverage ledgers must stay distinct:

1. criterion → obligation traceability;
2. obligation × environment execution coverage;
3. reachable state/action/surface frontier coverage.

None can round up another. A 100% traceability table whose oracles are weak is
not a release proof.

### 5.2 Falsifiers and oracle adequacy

Each required oracle declares a falsifier appropriate to its claim:

- known-bad input or fixture;
- targeted source or configuration mutation;
- wrong-side seam double;
- invalid lifecycle sequence;
- weakened formal invariant;
- deliberately inaccessible UI state;
- timeout, restart, disconnect, permission, or resource failure;
- counterexample promoted from model checking or exploration.

The correct candidate must satisfy the oracle and the falsifier must be
refuted. Accepting both yields `oracle_unsound`; inability to execute the
falsifier yields `INCONCLUSIVE`, not green. Sensitivity evidence belongs in a
receipt and can go stale independently from the Assurance Spec.

### 5.3 Seam declarations

A seam is its own obligation. A normative seam record must name:

- both real client and server, renderer and host, process and persistence
  artifact, or other connected sides;
- the route, protocol, wire contract, or lifecycle boundary crossed;
- the environment tier;
- a wiring-level oracle;
- a falsifier that breaks the relationship rather than only one isolated
  component;
- the evidence kinds that qualify.

Mock-only tests of both components do not satisfy the seam. A qualifying test
imports/drives the real code from both sides, or a receipt proves the actual
end-to-end connection.

### 5.4 Formal obligations

A formal block must declare:

- the narrow production contract it abstracts;
- the model boundary and omitted state;
- model/checker/configuration refs;
- properties and invariants;
- bounds and fairness assumptions;
- a mutation or deliberately weakened property;
- the expected counterexample;
- the preserved counterexample fixture;
- the mapped runtime regression.

A passing model means only that the bounded model satisfied the declared
properties under the recorded checker configuration. It cannot accept an
implementation, authorize release, or override contradictory runtime
evidence.

## 6. Environment Profiles

Environment facts change on a different cadence from product intent and proof
design. Store reusable, digest-pinned profiles separately:

```text
assurance/environments/<name>.assurance-environment.json
```

A profile should contain:

- profile ID, format version, revision, and digest;
- owner (`first_party` or `external`);
- target class: fixture, local, preview, staging, release artifact, device, or
  production;
- mutability: read-only, isolated-write, explicitly-armed-write, or blocked;
- OS, architecture, runtime, framework, renderer, browser/device versions;
- typed capabilities;
- immutable snapshot policy or dynamic deployment binding;
- authentication strategy using credential refs or environment-variable names,
  never secret values;
- fresh-identity, reset, restart, isolation, and revocation behavior;
- data classification, evidence visibility, retention, and redaction policy;
- permitted and forbidden actions;
- required native commands or service endpoints.

Framework detection can propose a profile. A reviewer admits it. No filename,
dependency name, or keyword heuristic may silently select tools or grant
capabilities. A missing capability creates a typed gap.

Private overlays may supply secret *references* and live topology outside Git.
The committed profile remains public-safe.

## 7. Adapter protocol

AssuranceSpec stays framework-neutral by putting native mechanics behind typed
adapters. A conforming adapter exposes:

1. `describe` — identity, version, digest, supported techniques,
   capabilities, and schemas;
2. `validate` — typed incompatibilities and missing capabilities;
3. `compile` — pure conversion from an admitted obligation and environment
   profile to execution units;
4. `execute` — perform an execution unit within explicitly granted
   capabilities;
5. `normalize` — convert native results into standard receipts;
6. `publicProject` — create a separately redacted public-safe projection.

There is deliberately no adapter operation that semantically interprets
ProductSpec prose. That judgment occurs when agents propose and reviewers admit
the Assurance Spec.

Adapter references are locked by version and content digest. An adapter cannot
expand target permissions beyond its environment profile. Unsupported
capabilities emit typed gaps, never implicit skips.

## 8. Review and admission

AssuranceSpec needs a lifecycle separate from observed test state:

- `proposed` — authored or agent-generated, not executable policy;
- `admitted` — exact revision/digest approved by recognized review policy;
- `superseded` — replaced by a later admitted revision;
- `retired` — intentionally no longer active.

For implementation conformance, distinguish:

- `structurally_valid` — parses and validates against the format;
- `reviewed` — has portable review annotations for required axes;
- `admitted_for_execution` — an external policy recognizes the review set and
  emits an admission receipt.

Validity does not imply adequate proof design. Review does not imply authority.
Admission is an external policy decision bound to the exact Assurance Spec
revision and digest.

### 8.1 Portable review annotations

Assurance review annotations should fix a gap in the upstream ProductSpec
review shape by binding the subject revision and digest:

```yaml
review_id: "review_..."
reviewer_tool: "..."
reviewed_at: "..."
assurance_spec_revision: 1
assurance_spec_digest: "sha256:..."
targets:
  - target_type: "obligation"
    target_id: "OBS-AO-001"
    axes:
      - axis_key: "oracle_adequacy"
        verdict: "pass"
        suggestion: "..."
        evidence_refs: []
```

Recommended axes:

- subject fidelity;
- criterion traceability;
- risk coverage;
- oracle adequacy;
- falsifier strength;
- seam reality;
- environment fidelity;
- evidence sufficiency;
- verifier independence;
- public safety;
- authority containment;
- feasibility.

Annotations are portable opinions. They become admission only when an external
policy recognizes the reviewer role and emits an admission receipt.

### 8.2 Calibration corpus

Calibration fixtures should include strong and weak examples, not only
schema-valid documents:

- a false green whose test repeats implementation behavior;
- a mobile/client-server seam where both component suites pass;
- a criterion with fixture proof but missing release proof;
- a formal model with an over-broad claim and narrow actual boundary;
- a valid exception with scope and expiry;
- an oracle mutation that proves sensitivity;
- an agent-generated obligation rejected during review.

These examples train reviewers and tools without turning taste into parser
validity.

## 9. Deterministic compilation

Observer's compiler begins *after* semantic planning and admission. It is a
pure function of:

```text
ProductSpec AST + format/revision/digest
AssuranceSpec AST + format/revision/digest
Environment Profile revisions/digests
adapter lock digest
accepted review-set/admission digest
compiler version
```

Normative compiler requirements:

- source digests cover exact UTF-8 bytes; parser line-ending normalization into
  the semantic model is documented and never rewrites the authored source;
- output uses canonical JSON with stable key and array ordering;
- compilation performs no network, clock, random, filesystem-discovery, or
  model calls beyond reading its declared inputs;
- no timestamps or absolute local paths appear in output;
- identical inputs produce byte-identical output;
- generated IDs derive from stable source anchors or content, never time;
- the manifest embeds all source refs and `do_not_edit: true`;
- the dependency graph identifies exactly which evidence becomes stale when an
  input changes;
- golden fixtures assert exact manifest bytes.

The Assurance Manifest contains resolved plans, not latest-run state:

- subject and source digests;
- resolved obligation graph;
- target/environment bindings;
- adapter and command digests;
- dependency and activation graph;
- oracle and falsifier execution units;
- evidence requirements;
- gate expressions;
- public-safety classification.

Readiness, verdict, infrastructure state, stability, freshness, human
disposition, and exceptions are dynamic projections from receipts and current
dependencies. They do not mutate the manifest.

Agents may generate native test scaffolds from manifest units, but those files
are proposals until their oracle and falsifier mappings are reviewed and their
digests are admitted in a new manifest.

## 10. Receipts, projections, and status axes

Normalized Assurance Receipts report one observation against exact source,
target, environment, adapter, command, oracle, falsifier, seed, and artifact
digests. Partial evidence flushes on interruption.

Never collapse these axes:

| Axis | Example values | Authority/source |
| --- | --- | --- |
| Admission | proposed, admitted, superseded, retired | Assurance Spec + admission receipt |
| Readiness | needs_design, planned_red, blocked, executable, not_applicable | compiled dependency projection |
| Observation | not_run, CONFIRMED, REFUTED, INCONCLUSIVE | run receipt |
| Infrastructure | ready, unarmed, unavailable, failed | runner receipt |
| Stability | unknown, stable, flaky | evidence aggregation |
| Freshness | current, stale | dependency projection |
| Disposition | pending_review, accepted, rejected, exception | authorized review |
| Exception | none or scoped exception receipt | external authority |

`INCONCLUSIVE`, stale, flaky, unarmed, unavailable, missing-adapter, skipped,
or missing evidence never rounds up to confirmed. Retry-until-green is not
evidence; retries are bounded and visible.

A release projection is a read model over the manifest, receipts, dependency
freshness, and external decisions. It may report that a declared gate
expression is satisfied. It cannot merge, deploy, spend, settle, accept, or
promote a public promise.

### 10.1 Exception receipts

The Assurance Spec defines exception policy; it does not carry live waivers as
ordinary authored state. A separate authorized exception receipt contains:

- exact subject, Assurance Spec, obligation, and environment scope;
- reason and recognized authority;
- issued-at and expiry/review date;
- maximum proof rung and public-claim limitation;
- evidence and compensating-control refs;
- revocation/supersession refs.

The obligation remains visible as unconfirmed-with-exception. An exception does
not transform missing proof into a pass.

## 11. Assurance Decision Trace

ProductSpec Decision Trace records product-intent drift. Assurance evolution
needs a parallel optional companion:

```text
<name>.assurance-decision-trace.json
```

Suggested events:

- `criterion_binding_changed`
- `obligation_added`
- `obligation_removed`
- `obligation_changed`
- `environment_changed`
- `adapter_changed`
- `oracle_changed`
- `falsifier_changed`
- `gate_changed`
- `evidence_policy_changed`
- `exception_granted`
- `exception_expired`
- `exception_revoked`
- `evidence_invalidated`
- `counterexample_promoted`
- `product_spec_reconciled`
- `assurance_revision`

Run results are receipts, not decision events. A consequential judgment about a
result becomes a trace event. If a finding exposes ambiguous product intent,
record both an Assurance Decision Trace event and the corresponding ProductSpec
Decision Trace event.

Until a portable assurance trace exists, ProductSpec Decision Trace can link an
Assurance Spec or receipt using its `other` link type. Potential upstream
extensions such as `assurance_spec`, `test_run`, `formal_model`,
`evidence_receipt`, `assurance_drift`, and `oracle_drift` should be proposed
only after dogfood shows the existing vocabulary is insufficient.

## 12. Validation, diagnostics, and conformance

AssuranceSpec needs the same useful rigor as ProductSpec's design while closing
several pre-1.0 enforcement gaps in the current reference implementation.

### 12.1 Three validation planes

1. **Format validity** — can tools parse, preserve, and compare the document?
2. **Assurance adequacy** — does it cover criteria and risks with plausible
   environments, oracles, falsifiers, seams, and evidence?
3. **Observed verification** — what did an exact run observe?

Plane 1 returns stable structural errors and warnings. Plane 2 returns gaps and
policy diagnostics. Plane 3 returns receipts. A schema-valid Assurance Spec is
not a claim that the product works.

### 12.2 Proposed stable structural codes

- `missing_frontmatter`
- `unsupported_version`
- `missing_required_frontmatter`
- `invalid_subject`
- `subject_revision_mismatch`
- `subject_digest_mismatch`
- `missing_required_section`
- `duplicate_section`
- `invalid_section_order`
- `invalid_custom_section_id`
- `duplicate_obligation_id`
- `dangling_source_ref`
- `dangling_environment_ref`
- `dangling_oracle_ref`
- `dangling_gate_ref`
- `invalid_gate`
- `cyclic_obligation_dependency`
- `invalid_formal_model_boundary`

### 12.3 Proposed adequacy diagnostics

- `uncovered_acceptance_criterion`
- `ambiguous_source_claim`
- `uncovered_risk`
- `weak_oracle`
- `missing_negative_case`
- `missing_falsifier`
- `mock_only_coverage`
- `missing_seam_coverage`
- `missing_real_environment`
- `formal_candidate_unmodeled`
- `evidence_policy_too_weak`
- `verifier_not_independent`
- `unsafe_environment_authority`
- `stale_generated_test`
- `stale_evidence`

Codes are API. Their meaning changes only through a format/tool version change
and conformance fixture.

### 12.4 Required conformance repository

A credible implementation should eventually contain:

```text
SPEC.md
CHANGELOG.md
GOVERNANCE.md
schema/assurance-spec.schema.json
schema/assurance-environment.schema.json
schema/assurance-manifest.schema.json
schema/assurance-run-receipt.schema.json
schema/assurance-review-annotation.schema.json
schema/assurance-decision-trace.schema.json
conformance/valid/
conformance/invalid/
conformance/adequacy/
conformance/compiler-golden/
parsers/ts/ or packages/assurance-spec/
docs/field-guide.md
docs/versioning.md
docs/validator.md
docs/decision-trace.md
```

Minimum tests:

- valid and invalid fixture corpus covering every stable code;
- schema/parser/validator parity;
- parse → serialize → parse semantic equality;
- unsupported-version rejection;
- unknown valid custom-section preservation;
- nested structured-block round trips;
- ProductSpec criterion-binding fixtures;
- duplicate, dangling, and dependency-cycle fixtures;
- deterministic compiler golden bytes;
- gate evaluation fixtures separate from parser conformance;
- environment-capability mismatch fixtures;
- mutation/falsifier fixtures proving oracle sensitivity;
- review-annotation and Decision Trace subject binding;
- a self-hosting Assurance Spec for Observer itself.

Do not repeat gaps found in the ProductSpec reference audit: selected-field
rather than complete schema/parser parity, date formats declared but not
validated, nested metadata that does not round-trip, custom-section placement
recorded but not enforced, or companion schemas without validators and
conformance corpora.

### 12.5 Conformance levels and technique declarations

Proposed interoperability levels:

- **AS-L1 Document** — parse, validate, serialize, preserve custom sections,
  and round-trip Assurance Specs;
- **AS-L2 Compiler** — emit schema-valid byte-stable manifests with stable IDs
  and typed gaps;
- **AS-L3 Execution** — implement at least one declared adapter, execute both
  correct and falsifier units, and emit normalized receipts;
- **AS-L4 Evidence lifecycle** — dependency freshness, independent
  verification, environment aggregation, redaction, and public projection.

Technique support is orthogonal. A tool claims, for example,
`AS-L3; techniques: bun_test, playwright`. It does not imply native, device,
formal, performance, or security support merely by saying “AssuranceSpec
compliant.”

## 13. Versioning and drift law

Keep these versions independent:

- `assurance_spec_format_version` — shape of the companion standard;
- `assurance_revision` — one subject's committed proof-design revision;
- bound ProductSpec format, revision, and digest;
- Environment Profile format, revision, and digest;
- adapter lock version and digest;
- Observer/compiler package version;
- Assurance Manifest format version;
- Assurance Receipt format version.

An `assurance_revision` bump is required when proof intent changes, including:

- criterion bindings;
- required or not-applicable obligations;
- risk coverage;
- proof rungs;
- seam declarations;
- oracle or falsifier meaning;
- evidence, independence, gate, or exception policy;
- authority boundaries.

A semantically equivalent test refactor does not require an Assurance Spec
revision, but changes native source/command digests, produces a new manifest,
and stales dependent evidence. Git holds the detailed diff; revision numbers
are portable citation handles.

Any ProductSpec revision or digest change stales the Assurance Spec until
explicit reconciliation. Dependency analysis may preserve unaffected evidence
only after a new Assurance Spec revision rebinds the unchanged criteria and a
new manifest records the surviving dependency digests. “The text looks close”
is not reconciliation.

## 14. Proposed repository layout

```text
specs/<area>/<name>.product-spec.md
specs/<area>/<name>.assurance-spec.md
specs/<area>/<name>.decision-trace.json
specs/<area>/<name>.assurance-decision-trace.json

assurance/environments/*.assurance-environment.json
assurance/adapters.lock.json
generated/assurance/*.assurance-manifest.json

tests/...                         # native tests stay in normal authoritative homes
specs/.../*.tla                  # formal models stay in their normal homes
var/assurance/runs/...           # ephemeral/private receipts and artifacts
```

Commit authored Product Specs, Assurance Specs, public-safe Environment
Profiles, and the adapter lock. Deterministic manifests may be committed or
regenerated and byte-compared once that repository policy is settled; either
way, never hand-edit them. Store large or private run evidence outside Git with
stable, redacted refs. A public Observatory reads approved projections, never
raw private artifacts.

## 15. How this rebuilds the existing QA harness

The first useful version wraps existing authoritative homes:

| Existing artifact | AssuranceSpec role |
| --- | --- |
| ProductSpec criterion | Subject criterion ref |
| `productspec-ai-evals` | Eval obligation seed; a fractional eval threshold does not accept the whole criterion |
| Behavior contract | Durable expectation and oracle ref |
| Planned-feature Eval Suite | Proposed or planned-red obligation seed |
| Target adapter | Environment/adapter implementation input |
| QA Runner commitment | Oracle statement input |
| QA Runner verify result | Observation input |
| QA Runner receipt | Evidence normalized by an adapter |
| QA Swarm | Manifest execution and exploration substrate |
| Distilled regression | Proposed new obligation or oracle revision |
| TLA+/TLC model | Formal obligation plus checker adapter |
| Arbiter and `/qa` | Receipt-backed projection only |
| Product promise | Informative link; registry still owns the public claim |

The harness does not become one monolithic runner. AssuranceSpec becomes the
portable proof-design control plane; adapters preserve the test tools already
native to each repository and framework.

## 16. Rollout

### AS-0 — standard dossier and calibration

- keep this proposal and Episode 252 honest about unshipped state;
- hand-author one example Assurance Spec against a real Product Spec;
- review strong/weak obligation and falsifier examples;
- settle mandatory sections, status vocabulary, and admission policy.

Exit: a reviewed format dossier and calibration packet, no runtime claim.

### AS-1 — document implementation

- add `packages/assurance-spec` with schema, parser, serializer, validator, and
  CLI;
- land valid/invalid fixtures and complete parity tests;
- support exact ProductSpec subject binding and OpenAgents criterion IDs;
- add portable review annotations.

Exit: AS-L1 with a self-validating example.

### AS-2 — deterministic compiler

- define Environment Profile, adapter lock, Manifest, and golden schemas;
- compile admitted source artifacts into byte-stable manifests;
- implement dependency-based stale projections;
- do not generate native tests yet.

Exit: AS-L2 and exact golden fixture bytes.

### AS-3 — wrap current harnesses

- add thin adapters for Bun tests, QA Runner, behavior contracts, target
  adapters, and TLC;
- normalize current receipts without moving their authoritative homes;
- demonstrate correct/falsifier execution and partial-evidence handling.

Exit: one end-to-end AS-L3 dogfood packet.

### AS-4 — Effect Native and OpenAgents Desktop dogfood

- use Effect Native as customer zero, not protocol shape;
- cover shared behavior plus renderer-specific capability boundaries;
- require real seam, release-artifact, and sensitivity evidence;
- publish only public-safe projections.

Exit: one admitted Assurance Spec whose release projection is independently
reviewed.

### AS-5 — portability and self-hosting

- implement one non-Effect project through native tooling;
- author an Assurance Spec for Observer itself;
- mutation-test the parser, compiler, adapter, and receipt normalizer;
- prove unsupported techniques produce gaps rather than skips.

Exit: evidence that the protocol is not an OpenAgents-only wrapper.

### AS-6 — hosted service and Observatory

- private hosted browser/native/device matrices;
- opt-in public project progress/evidence pages;
- retention, redaction, data residency, cost, and arming controls;
- ProductSpec/AssuranceSpec review as a hosted service.

Exit: separately priced and promise-gated service; no hosted dependency for
the local OSS protocol.

## 17. Upstream and governance posture

The current ProductSpec standard remains unchanged. OpenAgents consumes its
published semantic model, checks `spec_format_version`, binds exact
`spec_revision`, preserves Markdown/custom sections, and never mutates intent
during QA generation.

Possible upstream contributions after dogfood:

- stable structured Acceptance Criterion IDs;
- Decision Trace link types for Assurance Specs, test runs, formal models, and
  evidence receipts;
- assurance/oracle drift events if the existing event vocabulary proves
  insufficient.

Any future standalone AssuranceSpec standard should have the same governance
discipline it asks of implementers: issue-backed changes, compatibility impact,
fixtures, schema/parser/docs/validator updates, changelog, and independent
implementation feedback. OpenAgents-specific adapters, environments, release
policy, calibration, and hosted services remain extensions rather than core
vocabulary.

## 18. Authority matrix

| Layer | Owns | May not do |
| --- | --- | --- |
| ProductSpec | committed product intent | choose tests or grant release |
| AssuranceSpec | reviewed proof design | revise product intent or claim an observation |
| Environment Profile | declared target capabilities and policy | supply secrets or expand runtime authority |
| Assurance Manifest | deterministic execution plan | contain mutable latest status or be hand-edited |
| Oracles | evaluate bounded behavior | accept their own adequacy |
| QA Swarm/adapters | execute and explore | interpret product prose or silently skip |
| Receipts | report exact observations | make decisions or promote claims |
| Independent verifier | grade evidence under policy | become deploy/promise authority by implication |
| Maintainer/release policy | accept, reject, waive, release | rewrite evidence history |
| Product-promise registry | govern public claims | infer green from a test command alone |
| Formal model | prove a bounded abstraction | claim the unmodeled production system |

No layer may weaken or silently reinterpret an upstream layer to manufacture a
green downstream state.

## 19. Open design questions

- Which review roles can emit an admission receipt for each risk class?
- Should Environment Profiles be a core AssuranceSpec schema or a sibling
  companion specification?
- Is canonical JSON sufficient for manifests and receipts, or should a future
  content-addressed envelope standardize signatures too?
- What minimum falsifier qualifies for subjective UI and human-evaluation
  obligations?
- Which gate-expression language is small enough to audit and rich enough for
  proof-rung activation?
- Which evidence can survive a semantically equivalent source refactor, and
  who admits that equivalence?
- Should deterministic manifests be committed, or regenerated and compared in
  the normal local/OpenAgents-owned verification sweep?
- Which non-Effect reference project best demonstrates portability?
- What parts of Assurance Decision Trace should eventually converge with
  ProductSpec Decision Trace?
- When does incubation justify a separate `AssuranceSpec` repository and
  independent governance?

## 20. The concise product sentence

> ProductSpec commits what the product should do. AssuranceSpec commits how we
> will know. Observer compiles that reviewed proof design into an immutable
> execution graph. QA Swarm runs it. Receipts say what happened. Authorized
> people and policies decide what the evidence permits.
