# First AssuranceSpec dogfood — OpenAgents Desktop MVP

Date: 2026-07-13

Status: implementation plan; the authored Assurance Spec and supporting code do
not yet exist

## Outcome

Build only what is necessary to author, validate, review, admit, compile, and
minimally execute the first real AssuranceSpec against the current OpenAgents
Desktop MVP ProductSpec.

The first vertical slice is successful when:

1. an authored Assurance Spec binds the exact current MVP ProductSpec and all
   18 criterion IDs;
2. the format parser/validator accepts it and rejects known invalid fixtures;
3. an external review/admission artifact binds its exact revision/digest;
4. a deterministic compiler emits byte-identical Manifest bytes for identical
   inputs;
5. one existing `CW-AC-04` oracle runs through a pinned local Bun adapter;
6. one duplicate-ID falsifier is rejected through the same adapter;
7. normalized receipts keep all other criteria visibly uncovered;
8. changing the ProductSpec revision or bytes makes the Assurance Spec and
   evidence stale.

This is a dogfood pipeline gate only. It is not an MVP release gate, proof that
`CW-AC-04` is fully satisfied, proof for the other 17 criteria, or a public
promise transition.

## Exact subject

```text
path:                docs/mvp/openagents-codex-workroom-mvp.product-spec.md
spec_format_version: 0.1
spec_revision:       6
sha256:              3396b2dd2778c724184668b045dedc3288578685386beeef67b4316e83b99aa5
criterion IDs:       CW-AC-01 through CW-AC-18
```

The digest covers the exact committed UTF-8 bytes. The binder uses
`validateExecutableProductSpec`, not only standard ProductSpec validity,
because the Assurance Spec requires positive revision and stable unique
criterion IDs.

The first authored companion target is:

```text
docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md
```

It belongs beside the owner-directed co-located ProductSpec. Do not create a
second subject copy under `specs/` or `docs/assurance/`.

## Narrow first obligation

The first Assurance Spec names all 18 subject criteria, but initially admits
one narrow obligation:

```yaml
id: "MVP-AO-001"
title: "The MVP ProductSpec exposes exact stable executable criterion identity"
criterion_refs:
  - "CW-AC-04"
domains:
  - "contract"
technique: "bun_test"
environment_refs:
  - "ENV-OA-LOCAL-BUN-1"
adapter_ref: "openagents.bun_test.v1"
oracle:
  statement: >-
    The exact MVP ProductSpec is standard-valid, executable, revision-pinned,
    and exposes exactly one unique ID for CW-AC-01 through CW-AC-18.
  evaluator_ref:
    path: "packages/product-spec/test/product-spec.test.ts"
    test_name: "the MVP spec is executable with unique author-visible criteria"
falsifier:
  kind: "duplicate_acceptance_criterion_id"
  evaluator_ref:
    path: "packages/product-spec/test/product-spec.test.ts"
    test_name: "duplicate criterion IDs refuse executable admission"
  expected_error: "duplicate_acceptance_criterion_id"
evidence:
  proof_rung: "local_fixture"
  required_kinds:
    - "junit_test_report"
    - "oracle_sensitivity_receipt"
activation_gate: "GATE-ASSURANCESPEC-DOGFOOD"
```

This proves only the stable-criterion-identity subclaim of `CW-AC-04`. It does
not prove guided conversational authoring, section-addressed UI errors, opening
an existing spec through the packaged product, or the rule that no work starts
while IDs are absent. Those require later obligations, including the existing
Desktop workroom tests and eventually packaged-product evidence.

The remaining 17 criteria produce `uncovered_acceptance_criterion` adequacy
diagnostics and a `needs_design` readiness projection. They are not
`not_applicable`, waived, deferred out of sight, or counted toward green.

## Existing evidence to bind first

Do not generate replacement tests for behavior the repository already checks.

Candidate oracle:

```text
packages/product-spec/test/product-spec.test.ts
  "the MVP spec is executable with unique author-visible criteria"
```

Candidate falsifier:

```text
packages/product-spec/test/product-spec.test.ts
  "duplicate criterion IDs refuse executable admission"
```

Next expansion after the first receipt:

```text
apps/openagents-desktop/src/product-spec-workroom.test.ts
  "opens an executable spec with exact digest and criterion refs"
  "creates a viewable starter draft but refuses execution until IDs are authored"
```

These Desktop tests still do not establish the complete packaged UI journey.
Their receipts must say `local_fixture` or the exact tier they actually run.

### Current-main MVP proof driver

Current `main` also contains a stronger ProductSpec-native Desktop proof path:

```text
apps/openagents-desktop/src/mvp-proof.ts
apps/openagents-desktop/src/mvp-proof.test.ts
apps/openagents-desktop/scripts/run-mvp-proof.ts
bun run --cwd apps/openagents-desktop mvp-proof
```

It is double-gated, requires an isolated temporary profile/workspace plus
Codex-ready capacity, and records shell, ProductSpec open, plan acceptance,
root/child real turns, independent artifact verification, child-transcript,
and pending owner-gate steps with screenshots and a journal. That makes it a
high-value later packaged/real-Codex adapter candidate.

It does **not** replace `MVP-AO-001`. The current script authors its own
two-criterion `FX-AC-01`/`FX-AC-02` ProductSpec fixture and therefore does not
bind or prove the canonical revision-6 `CW-AC-01…18` subject. Assurance must
record that distinction. The first slice establishes exact subject identity;
a later obligation may bind the proof driver to the specific MVP claims it
actually exercises, without projecting its fixture success onto all criteria.

## Minimum artifacts to build

### Authored and reviewed

```text
docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md
docs/mvp/openagents-codex-workroom-mvp.assurance-review.json
docs/mvp/openagents-codex-workroom-mvp.assurance-admission.json
```

The review and admission may begin as public-safe local JSON artifacts, but
must bind exact ProductSpec and AssuranceSpec revisions/digests, review-set
digest, recognized role/actor ref, and the allowed dogfood gate. An agent cannot
self-admit its proposal. Cryptographic signatures are not required for the
first local slice; Git review and an explicit recognized actor remain external
authority.

### Reusable execution inputs

```text
assurance/environments/openagents-local-bun.assurance-environment.json
assurance/adapters.lock.json
```

`ENV-OA-LOCAL-BUN-1` initially pins:

- Bun `1.3.11`;
- `bun.lock` SHA-256
  `f8c5503aeade351f47e8ba2f5267c44df56667e8292edfdd86fb791ef5202e40`;
- repository-relative commands only;
- no credentials, external network, production target, or mutable customer
  state;
- isolated run-artifact writes only;
- dependency bootstrap must complete before the environment is `ready`.

The adapter lock pins `openagents.bun_test.v1` by version and content digest.
No dependency name or filename heuristic may select it implicitly.

### Generated and ephemeral

```text
generated/assurance/openagents-codex-workroom-mvp.assurance-manifest.json
var/assurance/runs/<run-ref>/candidate.junit.xml
var/assurance/runs/<run-ref>/falsifier.junit.xml
var/assurance/runs/<run-ref>/assurance-receipt.json
var/assurance/runs/<run-ref>/dogfood-projection.json
```

Manifest commit policy remains an explicit design decision. Whether committed
or regenerated and byte-compared, generated Manifests are never hand-edited.
Private/large run artifacts stay outside Git; only approved public-safe fixture
projections may be committed.

## Minimum implementation sequence

### AS-MVP-0 — freeze the subject fixture

- copy no ProductSpec content;
- record exact path, format, revision, digest, and `CW-AC-01…18` binding in
  conformance fixtures;
- add a test that a changed byte, revision, missing ID, duplicate ID, or changed
  criterion set invalidates the binding;
- expose a canonical SHA-256 exact-byte helper from the new assurance package
  rather than copying Desktop's private helper.

Exit: the exact current subject is a passing binding fixture and every drift
variant fails with a stable code.

### AS-MVP-1 — AssuranceSpec document core

Create `packages/assurance-spec/` with:

- Effect Schema semantic model for v0.1 frontmatter, mandatory sections, and
  the subject/environment/obligation/gate/evidence/authority blocks needed by
  this pilot;
- Markdown parser and serializer;
- structural validator with stable error codes;
- adequacy validator separated from structural validity;
- CLI `validate` and `coverage` commands;
- valid/invalid fixture corpus;
- parse → serialize → parse semantic equality;
- schema/parser parity and unsupported-version tests;
- custom-section preservation.

Do not implement speculative techniques not used by the pilot. Keep the schema
extensible and make unsupported techniques typed gaps.

Exit: the hand-authored MVP Assurance Spec is structurally valid, round-trips,
binds the exact subject, and reports 17 uncovered criteria without pretending
they pass.

### AS-MVP-2 — review and admission

- define minimal Assurance review-annotation and admission schemas;
- require review of subject fidelity, traceability, oracle adequacy, falsifier
  strength, environment fidelity, evidence sufficiency, and authority
  containment for `MVP-AO-001`;
- bind exact AssuranceSpec revision/digest, ProductSpec revision/digest, and
  review-set digest;
- make compilation refuse missing, stale, mismatched, or unauthorized
  admission;
- keep review opinion separate from admission authority.

Exit: one recognized reviewer admits only `GATE-ASSURANCESPEC-DOGFOOD` for the
exact source artifacts.

### AS-MVP-3 — Environment Profile and adapter lock

- implement the public-safe Environment Profile schema;
- create `ENV-OA-LOCAL-BUN-1` with explicit capabilities and forbidden actions;
- pin the exact Bun adapter and lock digest;
- return `infrastructure: unavailable` if dependencies are absent;
- never turn missing dependencies into `REFUTED` or an implicit skip.

Exit: environment and adapter validation pass in a dependency-bootstrapped clean
worktree and fail precisely when Bun, lock digest, or required capability
differs.

### AS-MVP-4 — deterministic compiler

Compile only admitted inputs into canonical JSON containing:

- exact subject, AssuranceSpec, admission, profile, lock, and compiler refs;
- resolved `MVP-AO-001` candidate and falsifier units;
- dependency and dogfood-gate graph;
- JUnit and sensitivity-receipt requirements;
- repository-relative command argv and symbolic run-relative artifact slots;
- `do_not_edit: true`.

The compiler performs no model, network, clock, random, environment discovery,
or absolute-path operation. Golden fixtures compare exact bytes. Changed input
digests create new output and stale the prior evidence.

Exit: identical inputs produce byte-identical Manifest bytes and every drift
fixture refuses or changes the expected digest.

### AS-MVP-5 — thin Bun-test adapter

Add a narrow `openagents.bun_test.v1` adapter. It may live behind QA Runner once
the contract is stable, but must consume the Assurance Manifest rather than
reinterpret ProductSpec prose.

Requirements:

- spawn explicit argv without a shell;
- run a dependency-bootstrapped clean worktree;
- use Bun's JUnit reporter rather than parsing console prose;
- select and assert the exact named test case;
- treat zero selected tests as infrastructure/adapter failure, never green;
- record exit status plus JUnit, stdout/stderr, source, command, and toolchain
  digests;
- normalize exact ProductSpec, AssuranceSpec, obligation, environment, adapter,
  candidate/falsifier, and evidence refs into an Assurance Receipt;
- retain native JUnit evidence rather than replacing it.

Candidate command shape:

```text
bun test packages/product-spec/test/product-spec.test.ts
  --test-name-pattern "the MVP spec is executable with unique author-visible criteria"
  --reporter=junit
  --reporter-outfile <run-relative-candidate-path>
```

The falsifier unit selects the duplicate-ID test and requires the expected
typed error. Exact CLI spelling must be confirmed against the pinned Bun
version during implementation rather than assumed from this plan.

Exit: candidate and falsifier each run exactly one named test and produce
normalized evidence.

### AS-MVP-6 — sensitivity, projection, and independent review

- execute the candidate and known-bad units;
- require candidate `CONFIRMED` plus falsifier `REFUTED` before emitting
  sensitivity `CONFIRMED`;
- keep observation, infrastructure, stability, freshness, and reviewer
  disposition separate;
- project `CW-AC-04` as partially supported at `local_fixture`, not accepted;
- project the remaining criteria as uncovered/`needs_design`;
- independently review exact JUnit and receipt refs;
- demonstrate that a copied subject with one-byte/revision change makes the
  admission and evidence stale.

Exit: the dogfood gate is satisfied without changing the MVP release or promise
state.

## Stable error and diagnostic minimum

Structural/binding errors needed for the first slice:

- `missing_frontmatter`
- `unsupported_version`
- `missing_required_frontmatter`
- `missing_required_section`
- `duplicate_section`
- `invalid_section_order`
- `invalid_subject`
- `subject_revision_mismatch`
- `subject_digest_mismatch`
- `duplicate_obligation_id`
- `dangling_source_ref`
- `dangling_environment_ref`
- `dangling_gate_ref`
- `invalid_gate`

Adequacy diagnostics needed for the first slice:

- `uncovered_acceptance_criterion`
- `weak_oracle`
- `missing_falsifier`
- `missing_real_environment`
- `evidence_policy_too_weak`
- `verifier_not_independent`
- `stale_evidence`

Do not add a stable code without at least one fixture asserting its exact
meaning.

## Test matrix

| Case | Expected result |
| --- | --- |
| Exact MVP r6 subject, valid AssuranceSpec | Structurally valid; one obligation; 17 coverage gaps |
| ProductSpec revision changed | `subject_revision_mismatch`; no compile |
| ProductSpec bytes changed | `subject_digest_mismatch`; no compile |
| Criterion missing/duplicated | binding failure; no compile |
| Admission missing or stale | structurally valid but not admitted; no compile |
| Environment dependencies missing | infrastructure unavailable; not `REFUTED` |
| Candidate named test passes | candidate `CONFIRMED` |
| Duplicate-ID falsifier test rejects mutation | falsifier `REFUTED`; sensitivity `CONFIRMED` |
| Zero selected tests | adapter/infrastructure failure; never green |
| Identical compiler inputs twice | byte-identical Manifest |
| Changed adapter/profile/source digest | new Manifest; dependent evidence stale |

## Current blocker already observed

In the clean documentation worktree used to design this pilot,

```text
bun test packages/product-spec
```

cannot start because the workspace dependency `effect` is not installed. No
test assertion runs. The first execution environment must bootstrap the pinned
lockfile before declaring `ENV-OA-LOCAL-BUN-1` ready. Until then this state is
`infrastructure: unavailable`, not a product refutation and not a skipped green.

## Explicit non-goals for the first slice

- no prose-to-obligation generation;
- no automatic test generation;
- no browser, native, device, staging, release, or production adapter;
- no QA Swarm sharding;
- no full `CW-AC-04` acceptance;
- no obligations claiming the other 17 MVP criteria;
- no hosted Observatory;
- no public report beyond approved fixture-safe artifacts;
- no change to the MVP ProductSpec, scope, sequencing, release gate, or promise
  registry.

## Expansion order after the first receipt

1. Add the two Desktop workroom `CW-AC-04` tests named above.
2. Add `CW-AC-05` exact digest/revision and reconciliation obligations.
3. Enroll the existing `mvp-proof` driver as an explicitly fixture-subject,
   Codex-ready Desktop execution adapter; do not map it to canonical criteria
   until each exercised claim is reviewed.
4. Add one real host/renderer or ProductSpec/work-packet seam obligation.
5. Add a packaged OpenAgents Desktop environment and release-artifact receipt.
6. Work outward by product risk, not criterion number, preserving uncovered
   gaps until real oracles exist.
7. Only after the MVP companion has useful breadth should Observer self-host or
   a non-Effect project test portability.

## Completion statement

The first dogfood is complete only when the exact r6 subject, admitted r1
Assurance Spec, deterministic Manifest, candidate test, duplicate-ID
falsifier, normalized receipts, stale-input proof, and independent review are
all dereferenceable. “A Markdown file exists” and “a test command returned 0”
are not sufficient.
