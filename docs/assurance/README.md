# OpenAgents Assurance

This directory is the canonical home for OpenAgents proof-design architecture:
the proposed AssuranceSpec companion standard, Observer, Assurance Manifests,
Environment Profiles, adapter contracts, evidence receipts, and the first
dogfood plan.

Assurance is deliberately separate from ProductSpec's portable evidence index,
the existing Desktop ProductSpec workroom loop, and QA execution:

```text
ProductSpec       commits intent and indexes evidence with Related Artifacts
  ├─ workroom     runs accepted plans, packets, evidence, verification, disposition
  └─ AssuranceSpec commits how we intend to know
       └─ Observer compiles admitted verification obligations
            └─ QA tools/swarm execute and emit exact Assurance Receipts

Assurance Receipts may feed the workroom and ProductSpec evidence index by ref.
Release policy remains separate. The promise registry alone governs claims.
```

The bounded proposal profile is implemented in `packages/assurance-spec`: it
can parse, serialize, structurally validate, assess, and deterministically
propose criterion-coverage scaffolding. Nothing here claims that richer
semantic planning, admission, compilation, adapters, execution, receipts, the
hosted service, or Observatory exists unless a document names exact code and
current receipts.

## Read in this order

1. [`ASSURANCE_SPEC.md`](./ASSURANCE_SPEC.md) — the canonical companion-format
   proposal: document model, obligation language, environments, adapters,
   admission, deterministic compilation, receipts, versioning, conformance,
   and authority boundaries.
2. [`PRODUCTSPEC_EVIDENCE_LOOP.md`](./PRODUCTSPEC_EVIDENCE_LOOP.md) — the
   adopted boundary for upstream Related Artifacts, the current Desktop
   ProductSpec workroom loop, AssuranceSpec, dual document/intent digests, and
   the OpenAgents `0.19.0` catch-up plan.
3. [`CURRENT_SYSTEM_MAP.md`](./CURRENT_SYSTEM_MAP.md) — how ProductSpec,
   behavior contracts, Eval Suites, existing tests, formal models, QA Runner,
   QA Swarm, receipts, and product promises compose without changing their
   authoritative homes.
4. [`OBSERVER_PRODUCT_PLAN.md`](./OBSERVER_PRODUCT_PLAN.md) — the extracted
   post-MVP product-plan seed, scope, candidate acceptance criteria, metrics,
   business shape, risks, and rollout for Observer.
5. [`MVP_FIRST_ASSURANCESPEC.md`](./MVP_FIRST_ASSURANCESPEC.md) — the concrete
   dogfood plan for building only what is necessary to author, validate, admit,
   compile, and minimally execute the first AssuranceSpec against the current
   OpenAgents Desktop MVP ProductSpec.
6. [`GAP_ANALYSIS.md`](./GAP_ANALYSIS.md) — three-way gap analysis: what we
   have implemented, what we have designed but not built, and what upstream
   ProductSpec 0.20 has shipped, per capability area, with actions.
7. [`AGENT_TOOLING.md`](./AGENT_TOOLING.md) — the design for our own
   AssuranceSpec agent surfaces: CLI extensions, the read-only deterministic
   MCP server with dual-digest session pinning and Agent Run 0.1 self-report
   ingest, the work/authoring skills,
   the starter-kit adoption path, and sequencing onto the AS ladder.
8. [`UX_CONTRACTS_AND_ASSURANCE.md`](./UX_CONTRACTS_AND_ASSURANCE.md) — where
   owner-stated UX/micro-interaction/visual do-and-don't rules live: statement
   verbatim in the owning app's behavior-contract registry with a test-sweep
   oracle, referenced from AssuranceSpec obligations via `contract_refs` for
   later environment-bound visual evidence; includes the worked Desktop
   icon-slot and approved-fonts example and the add-the-next-rule recipe.
9. [`../fable/2026-07-13-productspec-assurance-qa-program-analysis.md`](../fable/2026-07-13-productspec-assurance-qa-program-analysis.md)
   — strategy analysis of the whole ProductSpec/AssuranceSpec/QA landscape:
   upstream trajectory, honest inventory, committed ladders, and opinionated
   directions to evolve.
10. [`../transcripts/252-notes.md`](../transcripts/252-notes.md) — recording show
   notes only: story, episode beats, naming discussion, candidate lines, and
   honest non-claims.

## First dogfood target

AssuranceSpec will be tested first against:

```text
ProductSpec: docs/mvp/openagents-codex-workroom-mvp.product-spec.md
format:      ProductSpec 0.1
revision:    6
sha256:      fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1
criteria:    CW-AC-01 through CW-AC-18
```

This revision-6 artifact is the exact current baseline. It uses the existing
OpenAgents `CW-AC-*` profile and is not valid under upstream ProductSpec
`0.19.0`, whose item-level Related Artifacts require structured `AC-*`,
`EVAL-*`, and `SM-*` IDs. The first dogfood preserves and tests r6; a separately
reviewed ProductSpec revision must perform the portable ID/Related Artifact
migration before we claim upstream-current Evidence Loop interoperability.

The first generated proposal lives beside that ProductSpec as:

```text
docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md
```

The proposal utility creates that file from the exact ProductSpec plus optional
committed-repository inventory. It is structurally valid but deliberately not
executable: all 18 generated obligations begin as `needs_design`. Admission,
Environment Profiles, deterministic manifests, and native adapters remain
future slices.

The first reviewed executable vertical slice will target `CW-AC-04`: validator-clean
ProductSpec creation/opening with stable Acceptance Criterion IDs. It binds
the existing ProductSpec test before generating anything new, then uses the
existing duplicate-ID rejection as its falsifier. The generated revision names
all 18 criteria and creates one unresolved obligation for each. A later
reviewed revision may fully design this one narrow obligation while the other
17 remain `needs_design`.
Desktop workroom tests and receipt registration through the existing workroom
loop are the next expansion. None silently rounds up to green.

This dogfood validates the companion system alongside MVP development. It does
not expand MVP product scope, declare any criterion passed, make Observer a
release prerequisite, or change a public promise.

## Directory boundaries

- `docs/assurance/` owns current proof-design standards, architecture, and
  dogfood plans.
- `docs/mvp/` owns the canonical MVP ProductSpec and its generated, unadmitted
  AssuranceSpec proposal.
- `apps/openagents-desktop/src/product-spec-workroom*` owns the implemented
  plan/packet/lease/evidence/independent-verification/owner-disposition loop.
  AssuranceSpec feeds it receipts by reference; it does not replace its state.
- `docs/qa/` retains QA execution notes, runbooks, reports, and historical
  Khala Code evidence. It does not own AssuranceSpec semantics.
- `packages/product-spec/` implements the current OpenAgents ProductSpec
  profile. It is behind upstream `0.19.0`; structured AC/SM items and Related
  Artifacts are planned catch-up work, not current compatibility claims.
- `packages/assurance-spec/` owns the implemented bounded proposal profile,
  parser, serializer, structural validator, adequacy assessment, repository
  inventory, and CLI. It does not own test execution or release authority.
- Native tests, behavior contracts, Eval Suites, and formal models stay in
  their owning packages. AssuranceSpec references them; it does not duplicate
  them.

## Current status

| Layer | Status |
| --- | --- |
| ProductSpec subject | Real: MVP revision 6 is validator-managed and has stable `CW-AC-*` IDs |
| Upstream ProductSpec Evidence Loop | Local parser/validator is pinned to upstream `0.19.0`, including structured AC/EVAL/SM, `applies_to`, Related Artifacts, and document/intent digests; portable publication for the legacy r6 subject still requires the explicit ID migration |
| Desktop ProductSpec workroom loop | Real: accepted plans, packets, leases, evidence receipts, verifier/producer ref checks, and owner dispositions |
| AssuranceSpec design | Implemented protocol and tooling in this directory and `packages/assurance-spec`; design remains separate from execution, verdict, release, and public-claim authority |
| MVP AssuranceSpec | Revision 2 reviewed and admitted; 18/18 candidates `CONFIRMED`, 18/18 falsifiers `REFUTED`, 18 sensitivity receipts, and the full Desktop gate green |
| Proposal schema/parser/serializer/validation | Implemented with conformance corpus, custom-section preservation, review annotations, and schema/parser parity tests |
| ProductSpec-to-AssuranceSpec proposal | Implemented deterministic coverage skeleton; semantic proof planning remains separately reviewed and is never inferred by the deterministic tool |
| Environment Profile and adapter lock | Implemented and digest-bound in the admitted MVP manifest |
| Deterministic Assurance Manifest compiler | Implemented with golden-byte and binding tests |
| Native Bun test adapter and normalized receipt | Implemented; native output remains private and reviewed projections are digest-bound |
| Typed Assurance Receipt resolver/registration in the Desktop workroom | Implemented through the immutable bridge and exercised by `AO-CW-AC-04-01` |
| ProductSpec Related Artifact publication | Parser and evidence-only edit path implemented; portable item-level publication for the live legacy r6 identity waits for PSEL-3 rebinding |
| QA Swarm manifest consumption | Outside the accepted Desktop MVP; any future adapter remains a separately admitted execution lane |
| Hosted Observatory | Deployed read-only criterion-first projection at `/observer/traces/openagents-desktop-codex-workroom-mvp` |

## Naming

- **AssuranceSpec**: framework-neutral companion standard and authored
  artifact.
- **Assurance Spec**: human-readable artifact name.
- **Observer**: OpenAgents planner/compiler/product codename, pending normal
  naming review.
- **Assurance Manifest**: generated immutable execution lockfile.
- **Assurance Receipt**: normalized observed evidence.
- **Observatory**: possible future multi-project evidence surface, not a
  current product claim.
