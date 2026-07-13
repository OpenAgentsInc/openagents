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
   evidence stale under the current exact-byte legacy binding;
9. a current `CONFIRMED` Assurance Receipt is registered by exact reference on
   a canonical-r6/`CW-AC-04` Desktop work packet through a typed resolver, then
   verified with a distinct host verifier reference and a separately enforced
   Assurance producer/reviewer policy, without changing owner, release, or
   promise state;
10. `REFUTED`, `INCONCLUSIVE`, stale, or infrastructure-failed Assurance
    results demonstrably cannot enter the workroom's current pass-only
    verification path.

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

This is an explicit OpenAgents legacy-profile binding. Upstream ProductSpec
`0.19.0` requires structured `AC-*` Acceptance Criteria and `SM-*` Success
Metrics for item-level Related Artifacts. The current r6 file uses Markdown
`CW-AC-*` criteria and semantic metric IDs, so it cannot honestly publish a
portable Related Artifact for `CW-AC-04`. The first AssuranceSpec still tests
the current MVP as requested; upstream-current evidence publication follows a
separately reviewed ID/spec revision migration.

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

## Evidence Loop composition

Three evidence surfaces must stay separate:

1. the immutable native JUnit and normalized Assurance Receipts say what the
   exact QA run observed;
2. the implemented Desktop workroom owns the packet evidence envelope,
   distinct-verifier receipt, and owner packet disposition;
3. upstream ProductSpec Related Artifacts provide portable pointers from
   standard `AC-*`/`EVAL-*`/`SM-*` IDs to durable evidence elsewhere.

The first r6 dogfood composes surfaces 1 and 2. It records the Assurance
Receipt as `evidenceKind: receipt` on an exact `CW-AC-04` packet only after a
typed bridge validates and resolves it to an immutable RefSchema-safe handle.
It uses a host verifier ref different from the lease executor and separately
checks the Assurance receipt's actual producer/reviewer policy. Owner
disposition stays pending unless the owner separately acts.

Surface 3 is a follow-on compatibility gate. After the local ProductSpec
package supports upstream `0.19.0` and a reviewed ProductSpec revision maps
`CW-AC-01…18` to structured `AC-1…18`, the ProductSpec can link a stable
public-safe Assurance Evidence Index. A link's existence never counts as a
pass.

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
var/assurance/receipt-resolver/assurance.receipt.<digest>.json
var/assurance/runs/<run-ref>/dogfood-projection.json
```

Manifest commit policy remains an explicit design decision. Whether committed
or regenerated and byte-compared, generated Manifests are never hand-edited.
Private/large run artifacts stay outside Git; only approved public-safe fixture
projections may be committed.

After the upstream compatibility and item-ID migration gate, add one stable
public-safe projection path rather than a new ProductSpec URL for every run:

```text
docs/mvp/evidence/openagents-codex-workroom-mvp.assurance-index.json
```

That index carries its own schema version, public-safety flag, exact ProductSpec
and AssuranceSpec identities, current Manifest ref/digest, criterion-to-
obligation mappings, immutable receipt refs/digests, freshness evaluation, and
superseded history. It is a projection, not authority. The ProductSpec link
stays stable while the validated index evolves.

## Minimum implementation sequence

AS-MVP-0 through AS-MVP-7 deliberately exercise the exact current r6 local
profile. They must label that profile and may not claim upstream Related
Artifact portability. PSEL-MVP-1 is the separate compatibility/reconciliation
gate that closes the portable Evidence Loop afterward.

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

### AS-MVP-7 — compose with the Desktop workroom loop

- create a new canonical-r6 accepted plan with at least two non-equivalent
  packets, one child allocation, valid dependencies, and every criterion mapped
  or explicitly deferred;
- make the evidence packet's criterion IDs exactly `[CW-AC-04]`, never a
  broader set. A second packet may implement the receipt bridge under the
  narrow independent-verification portion of `CW-AC-08`; it has its own tests
  and does not inherit `MVP-AO-001` evidence. The `CW-AC-04` evidence packet
  depends on that bridge packet reaching host `verified`;
- admit the bridge packet and evidence packet only while each is `active` with
  its exact live lease. The host cannot append evidence to an
  `evidence_present`, `verified`, failed, cancelled, or superseded packet;
- build `openagents.assurance_receipt_bridge.v1`. Before any host mutation it
  must decode and digest-check the Assurance Receipt; verify exact
  ProductSpec/AssuranceSpec/admission/Manifest/obligation bindings; require
  `CONFIRMED`, current, ready, stable/nonflaky sensitivity evidence; enforce
  Assurance producer/reviewer independence and publication policy; and reject
  every other state with a typed result;
- store an immutable resolver entry and issue a RefSchema-safe opaque handle
  such as `assurance.receipt.<sha256hex>`. The current host treats
  `evidenceRef` as opaque and does not dereference the Assurance Receipt;
- register that handle as `evidenceKind: receipt`, preserving the native JUnit
  and Assurance Receipt;
- use a host `verifierRef` different from the active lease executor recorded as
  `evidenceProducerRef`. This unequal-ref check is not authenticated identity
  proof and does not replace the bridge's Assurance producer/reviewer check;
- call the current pass-only `verifyEvidence` path only with the exact host
  evidence-receipt refs the reviewer actually resolved;
- prove `REFUTED`, `INCONCLUSIVE`, stale, flaky, unavailable, and
  infrastructure-failed inputs remain non-verified and visible;
- leave owner disposition pending unless an owner separately accepts or waives
  the packet; never infer release or promise state.

Exit: the first AssuranceSpec dogfood uses the real ProductSpec workroom as its
runtime evidence integration point without building a second packet/status
ledger or laundering a non-confirming result.

### PSEL-MVP-1 — upstream Evidence Loop catch-up and reconciliation

This is the next gate, not a hidden prerequisite for the r6 internal dogfood:

- vendor current upstream ProductSpec `0.19.0` conformance fixtures;
- implement structured AC/EVAL/SM and Related Artifact parser, serializer,
  validator, exact errors, and unusual-target warnings locally;
- add exact document digest plus a versioned canonical intent-projection digest
  and golden tests. The projection includes `product_spec` dependency links and
  consumed `tool_metadata`; only typed evidence attachments are excluded;
- add an owner-confirmed evidence-attachment edit path that proves intent is
  unchanged and atomically rechecks exact bytes. Do not weaken generic
  `proposeEdit`, which currently requires a revision bump for every byte edit;
- propose a ProductSpec revision mapping `CW-AC-01…18` to `AC-1…18` and the
  seven semantic metric IDs to `SM-1…7`;
- use reviewed single-line criterion-normalization fixtures to preserve
  semantics; upstream's handwritten parser does not accept YAML block scalars;
- preserve each current metric's `segment` and `source` in a keyed
  `custom-success-metric-context` section;
- create a machine-readable old-to-new ID mapping artifact and use ProductSpec
  Decision Trace prose plus a link to explain and approve it;
- retain or supersede old-identity packets/runs, then create a new accepted
  plan/run for the migrated IDs. Never retarget old history or carry its
  evidence forward automatically;
- seed the stable public-safe Assurance Evidence Index in `no_evidence`, add
  `engineering_spec`/`other`/`code` Related Artifact refs, validate dangling
  item errors and unusual-target warnings, and only then freeze the migrated
  ProductSpec document/intent identities;
- explicitly rebind and re-admit the Assurance Spec against `AC-4`, create the
  new exact-identity workroom plan/run, and rerun the narrow obligation before
  publishing current evidence;
- prove a typed evidence-attachment-only edit can keep `spec_revision` and the
  canonical intent digest while changing the document digest. The current
  Desktop run still becomes exact-identity mismatch and requires a new run.

Exit: item-level evidence links are portable under current ProductSpec, and the
Assurance system can distinguish evidence-index maintenance from intent drift.

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
- `subject_document_digest_mismatch` (the legacy r6 binder has no intent digest)
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

PSEL-MVP-1 additionally adopts upstream `invalid_related_artifact` and
`unusual_related_artifact_target` semantics, then adds link-resolution and
evidence-chain diagnostics only in their proper layers, including
`subject_intent_digest_mismatch`,
`missing_related_artifact_target`, `unsafe_related_artifact_path`,
`invalid_assurance_evidence_index`, `related_artifact_subject_mismatch`, and
`related_artifact_unresolved_external`. Offline external URLs remain
`INCONCLUSIVE`; structural validity never fabricates reachability or proof.

## Test matrix

| Case | Expected result |
| --- | --- |
| Exact MVP r6 subject, valid AssuranceSpec | Structurally valid; one obligation; 17 coverage gaps |
| ProductSpec revision changed | `subject_revision_mismatch`; no compile |
| ProductSpec bytes changed | `subject_document_digest_mismatch`; no compile |
| Criterion missing/duplicated | binding failure; no compile |
| Admission missing or stale | structurally valid but not admitted; no compile |
| Environment dependencies missing | infrastructure unavailable; not `REFUTED` |
| Candidate named test passes | candidate `CONFIRMED` |
| Duplicate-ID falsifier test rejects mutation | falsifier `REFUTED`; sensitivity `CONFIRMED` |
| Current confirmed receipt registered by exact workroom ref | bridge-valid evidence-present, then `verified` only with a distinct verifier ref |
| Opaque ref has missing/mutated/subject-mismatched Assurance Receipt | bridge refuses before `recordEvidence` |
| Refuted/inconclusive/stale/infrastructure-failed receipt presented to host adapter | no host `passed`/`verified`; explicit blocking result |
| Zero selected tests | adapter/infrastructure failure; never green |
| Identical compiler inputs twice | byte-identical Manifest |
| Changed adapter/profile/source digest | new Manifest; dependent evidence stale |
| Related Artifact targets missing `AC-*` item after PSEL-MVP-1 | upstream-compatible `invalid_related_artifact` |
| Eval artifact targets `SM-*` after PSEL-MVP-1 | valid with `unusual_related_artifact_target` warning |
| Typed evidence-attachment-only edit after dual-digest support | document digest changes; intent digest/revision stay stable; existing Desktop run mismatches |

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
- no change to the r6 MVP ProductSpec, scope, sequencing, release gate, or
  promise registry during the first internal dogfood;
- no claim that `CW-AC-*` Related Artifacts are upstream portable before the
  explicit PSEL-MVP-1 ProductSpec revision and ID reconciliation.

## Expansion order after the first receipt

1. Add the two Desktop workroom `CW-AC-04` tests named above.
2. Add `CW-AC-05` exact digest/revision and reconciliation obligations.
3. Complete PSEL-MVP-1 so item-level Related Artifacts and intent/document
   digest separation are real rather than prose.
4. Enroll the existing `mvp-proof` driver as an explicitly fixture-subject,
   Codex-ready Desktop execution adapter; do not map it to canonical criteria
   until each exercised claim is reviewed.
5. Add one real host/renderer or ProductSpec/work-packet seam obligation.
6. Add a packaged OpenAgents Desktop environment and release-artifact receipt.
7. Work outward by product risk, not criterion number, preserving uncovered
   gaps until real oracles exist.
8. Only after the MVP companion has useful breadth should Observer self-host or
   a non-Effect project test portability.

## Completion statement

The first internal dogfood is complete only when the exact r6 subject, admitted
r1 Assurance Spec, deterministic Manifest, candidate test, duplicate-ID
falsifier, normalized receipts, stale-input proof, distinct-verifier workroom
integration through a typed immutable resolver, and independent review are all
dereferenceable. “A Markdown file
exists,” “a test command returned 0,” and “a Related Artifact URL exists” are
not sufficient. Portable closure additionally requires PSEL-MVP-1.
