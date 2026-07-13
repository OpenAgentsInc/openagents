# OpenAgents Assurance

This directory is the canonical home for OpenAgents proof-design architecture:
the proposed AssuranceSpec companion standard, Observer, Assurance Manifests,
Environment Profiles, adapter contracts, evidence receipts, and the first
dogfood plan.

Assurance is deliberately separate from both ProductSpec and QA execution:

```text
ProductSpec       commits what the product should do and why
AssuranceSpec     commits how we intend to know
Observer          compiles admitted proof design into an immutable manifest
QA tools/swarm    execute native checks and explore
Receipts          report exact observations
Maintainers       accept, reject, waive, or release under external policy
Promise registry  alone governs public claims
```

Nothing in this directory claims the proposed AssuranceSpec parser, compiler,
adapter protocol, hosted service, or Observatory is implemented unless a
document names exact code and current receipts.

## Read in this order

1. [`ASSURANCE_SPEC.md`](./ASSURANCE_SPEC.md) — the canonical companion-format
   proposal: document model, obligation language, environments, adapters,
   admission, deterministic compilation, receipts, versioning, conformance,
   and authority boundaries.
2. [`CURRENT_SYSTEM_MAP.md`](./CURRENT_SYSTEM_MAP.md) — how ProductSpec,
   behavior contracts, Eval Suites, existing tests, formal models, QA Runner,
   QA Swarm, receipts, and product promises compose without changing their
   authoritative homes.
3. [`OBSERVER_PRODUCT_PLAN.md`](./OBSERVER_PRODUCT_PLAN.md) — the extracted
   post-MVP product-plan seed, scope, candidate acceptance criteria, metrics,
   business shape, risks, and rollout for Observer.
4. [`MVP_FIRST_ASSURANCESPEC.md`](./MVP_FIRST_ASSURANCESPEC.md) — the concrete
   dogfood plan for building only what is necessary to author, validate, admit,
   compile, and minimally execute the first AssuranceSpec against the current
   OpenAgents Desktop MVP ProductSpec.
5. [`../transcripts/252-notes.md`](../transcripts/252-notes.md) — recording show
   notes only: story, episode beats, naming discussion, candidate lines, and
   honest non-claims.

## First dogfood target

AssuranceSpec will be tested first against:

```text
ProductSpec: docs/mvp/openagents-codex-workroom-mvp.product-spec.md
format:      ProductSpec 0.1
revision:    6
sha256:      3396b2dd2778c724184668b045dedc3288578685386beeef67b4316e83b99aa5
criteria:    CW-AC-01 through CW-AC-18
```

The first authored companion is intended to live beside that ProductSpec as:

```text
docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md
```

That file does **not** exist yet. We will first build the bounded format,
validation, subject binding, admission, Environment Profile, deterministic
manifest, and one native test adapter necessary to make it real.

The first executable vertical slice targets `CW-AC-04`: validator-clean
ProductSpec creation/opening with stable Acceptance Criterion IDs. It binds
the existing ProductSpec test before generating anything new, then uses the
existing duplicate-ID rejection as its falsifier. The subject block names all
18 criteria, but the first revision admits only this narrow obligation; the
other 17 produce `uncovered_acceptance_criterion`/`needs_design` diagnostics.
Desktop workroom tests are the next expansion. None silently rounds up to
green.

This dogfood validates the companion system alongside MVP development. It does
not expand MVP product scope, declare any criterion passed, make Observer a
release prerequisite, or change a public promise.

## Directory boundaries

- `docs/assurance/` owns current proof-design standards, architecture, and
  dogfood plans.
- `docs/mvp/` owns the canonical MVP ProductSpec and will own its authored
  AssuranceSpec companion when the format implementation exists.
- `docs/qa/` retains QA execution notes, runbooks, reports, and historical
  Khala Code evidence. It does not own AssuranceSpec semantics.
- `packages/product-spec/` continues to implement upstream-compatible product
  intent only.
- Future `packages/assurance-spec/` code must not be inferred from these design
  docs; it exists only when committed implementation and conformance tests do.
- Native tests, behavior contracts, Eval Suites, and formal models stay in
  their owning packages. AssuranceSpec references them; it does not duplicate
  them.

## Current status

| Layer | Status |
| --- | --- |
| ProductSpec subject | Real: MVP revision 6 is validator-managed and has stable `CW-AC-*` IDs |
| AssuranceSpec design | Proposed in this directory |
| MVP AssuranceSpec | Planned; not authored or admitted |
| Parser/schema/conformance | Not implemented |
| Environment Profile and adapter lock | Not implemented |
| Deterministic Assurance Manifest compiler | Not implemented |
| Native Bun test adapter and normalized receipt | Not implemented |
| QA Swarm manifest consumption | Not implemented |
| Hosted Observatory | Not implemented |

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
