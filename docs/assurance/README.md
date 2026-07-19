# OpenAgents Assurance

This directory is the canonical home for OpenAgents proof-design architecture:
the AssuranceSpec companion standard, Observer, Assurance Manifests,
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

The implemented `packages/assurance-spec` package parses, serializes, validates,
proposes, reviews, admits, compiles, executes through typed adapters, normalizes
receipts, projects the eight independent status axes, and exposes CLI, MCP,
skill, and owned-runner surfaces. The admitted Desktop MVP, immutable evidence
index, mutation evidence, workroom bridge, and hosted Observatory name their
exact code and receipts below. Rich semantic planning, release decisions, and
public-claim authority remain separate and are never inferred from these tools.

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
   owner-stated UX/micro-interaction/visual do-and-do not rules live: statement
   verbatim in the owning app's behavior-contract registry with a test-sweep
   oracle, referenced from AssuranceSpec obligations via `contract_refs` for
   later environment-bound visual evidence. Includes the worked Desktop
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
`EVAL-*`, and `SM-*` IDs. The first dogfood preserves and tests r6. A separately
reviewed ProductSpec revision must perform the portable ID/Related Artifact
migration before we claim upstream-current Evidence Loop interoperability.

The reviewed and admitted live AssuranceSpec lives beside that ProductSpec as:

```text
docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md
```

The proposal utility created revision 1 from the exact ProductSpec plus
committed-repository inventory. Its frozen bytes remain in the conformance
corpus. The live revision 2 is reviewed and admitted. All 18 obligations have
candidate, falsifier, and sensitivity receipts, and the full Desktop gate is
green. The deterministic compiler, digest-bound Environment Profile and
adapter lock, narrow Vite Plus test adapter, receipt normalizer, and Desktop workroom
receipt bridge are implemented. That evidence does not grant release or
public-claim authority, and broader browser/device/formal adapters remain
separate future work.

This dogfood validates the companion system alongside MVP development. It does
not expand MVP product scope, declare any criterion passed, make Observer a
release prerequisite, or change a public promise.

## IDE proof-design proposals

The canonical IDE roadmap now has two proposed umbrella companions:

- `specs/desktop/desktop-trust-complete-workbench.assurance-spec.md` binds the
  exact Desktop ProductSpec revision-7 bytes and AC-1..AC-52. And
- `specs/openagents/cursor-capability-parity.assurance-spec.md` binds the exact
  Cursor parity ProductSpec revision-3 bytes and CP-AC-01..CP-AC-27.

Both remain `lifecycle_state: proposed`. They provide complete criterion
coverage but zero ready obligations: environments, techniques, oracles,
falsifiers, evidence policy, independent review, and gates remain
`needs_design`. No repository candidate, Environment Profile, proof technique,
oracle, falsifier, reviewer, or gate is selected. Neither proposal proves an
IDE packet, admits execution, changes a release rung, or
supports a Cursor-parity claim. `specs/IDE_ROADMAP_CROSSWALK.md` maps each
IDE-00..19 packet to its owning criteria and states which unchanged specs—most
notably Full Auto revision 14 and AssuranceSpec revision 4—are dependencies
rather than subjects to rewrite.

There is one explicit tooling gap. The Desktop ProductSpec uses the validated
upstream-style `productspec-acceptance-criteria` block, while the current
AssuranceSpec proposal/session executable-profile helper extracts only top-
level Markdown criterion bullets. The Desktop companion was therefore
mechanically built from the ProductSpec parser's validated structured item
list and rebound to the SHA-256 of the unchanged original bytes. Its own
Objective and Subject disclose that bridge. Structural validation works. A
future tooling change must teach proposal/session handling to consume the
structured list directly before that companion can use the ordinary session-
pin path. This limitation is not permission to downgrade the ProductSpec,
ignore its digest, or treat the proposal as admitted.

## Directory boundaries

- `docs/assurance/` owns current proof-design standards, architecture, and
  dogfood plans.
- `docs/mvp/` owns the canonical MVP ProductSpec and its generated, unadmitted
  AssuranceSpec proposal.
- `apps/openagents-desktop/src/product-spec-workroom*` owns the implemented
  plan/packet/lease/evidence/independent-verification/owner-disposition loop.
  AssuranceSpec feeds it receipts by reference. It does not replace its state.
- `docs/qa/` retains QA execution notes, runbooks, reports, and historical
  Khala Code evidence. It does not own AssuranceSpec semantics.
- `packages/product-spec/` implements the pinned upstream `0.19.0` structured
  AC/EVAL/SM, Related Artifact, dual-digest/evidence-edit, and Decision Trace
  v0.1 surfaces alongside the legacy OpenAgents profile. The dependency graph
  and upstream MCP checklist/session behavior remain unsupported.
- `packages/assurance-spec/` owns the implemented bounded profile, parser,
  serializer, validators, review/admission artifacts, deterministic compiler,
  Environment Profile and adapter contracts, narrow owned runner, normalized
  receipts, repository inventory, CLI, and MCP surfaces. It owns no release or
  public-claim authority.
- Native tests, behavior contracts, Eval Suites, and formal models stay in
  their owning packages. AssuranceSpec references them. It does not duplicate
  them.

## Current status

| Layer | Status |
| --- | --- |
| ProductSpec subject | Real: MVP revision 6 is validator-managed and has stable `CW-AC-*` IDs |
| Upstream ProductSpec Evidence Loop | Local parser/validator is pinned to upstream `0.19.0`, including structured AC/EVAL/SM, `applies_to`, Related Artifacts, and document/intent digests. Portable publication for the legacy r6 subject still requires the explicit ID migration |
| Desktop ProductSpec workroom loop | Real: accepted plans, packets, leases, evidence receipts, verifier/producer ref checks, owner dispositions, and an exact-byte owner-confirmed evidence-attachment-only maintenance path. Prior runs remain stale history |
| AssuranceSpec design | Implemented protocol and tooling in this directory and `packages/assurance-spec`. Design remains separate from execution, verdict, release, and public-claim authority |
| MVP AssuranceSpec | Revision 2 reviewed and admitted. 18/18 Candidates `CONFIRMED`, 18/18 falsifiers `REFUTED`, 18 sensitivity receipts, and the full Desktop gate green |
| Proposal schema/parser/serializer/validation | Implemented with conformance corpus, custom-section preservation, review annotations, and schema/parser parity tests |
| ProductSpec-to-AssuranceSpec proposal | Implemented deterministic coverage skeleton plus a typed, injected Observer semantic-planner boundary. Every result remains a separately reviewed `proposed` AssuranceSpec and the deterministic tool never infers proof semantics |
| IDE AssuranceSpec proposals | Desktop rev 7: 52/52 criteria represented, 0 ready. Cursor parity rev 3: 27/27 represented, 0 ready. Both proposed and execution-unauthorized. Desktop structured-item proposal/session extraction remains an explicit tooling gap |
| Environment Profile and adapter lock | Implemented and digest-bound in the admitted MVP manifest |
| Deterministic Assurance Manifest compiler | Implemented with golden-byte and binding tests |
| Native Vite Plus test adapter and normalized receipt | Implemented. Native output remains private and reviewed projections are digest-bound |
| Typed Assurance Receipt resolver/registration in the Desktop workroom | Implemented through the immutable bridge and exercised by `AO-CW-AC-04-01` |
| ProductSpec Related Artifact publication | Parser, package planner/apply primitives, Desktop proposal-only agent tool, and separate owner-confirmation path implemented. Portable item-level publication for the live legacy r6 identity waits for PSEL-3 rebinding |
| QA Swarm manifest consumption | Evidence-only six-lane orchestration is implemented for the current Desktop target. Real/spend/native execution remains explicitly armed, and its receipts require separate review/admission |
| Hosted Observatory | Deployed read-only criterion-first projection at `/observer/traces/openagents-desktop-codex-workroom-mvp` |

## Naming

- **AssuranceSpec**: framework-neutral companion standard and authored
  artifact.
- **AssuranceSpec**: human-readable artifact name.
- **Observer**: OpenAgents planner/compiler/product codename, pending normal
  naming review.
- **Assurance Manifest**: generated immutable execution lockfile.
- **Assurance Receipt**: normalized observed evidence.
- **Observatory**: possible future multi-project evidence surface, not a
  current product claim.
