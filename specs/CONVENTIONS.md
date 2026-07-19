# specs/ — ProductSpec conventions

This tree holds `.product-spec.md` files.
These files are ProductSpec-format (v0.1) intent artifacts for consequential OpenAgents work.
The full rationale and adoption design is in `docs/fable/2026-07-08-productspec-adoption-analysis.md` (#8593).
The validator and CLI are in `packages/product-spec`.
The normal test sweep validates every file here (`bun test packages/product-spec`).

## What a spec is (and is not)

A ProductSpec is the durable statement of **product intent before implementation**.
It states the problem, hypothesis, and scope (in/out/**cut**).
It also states acceptance criteria and success metrics.
Acceptance criteria are the pre-launch build contract.
Success metrics are the post-launch market contract.

Current upstream ProductSpec also permits Related Artifacts.
These artifacts are typed pointers from stable intent IDs to external implementation, evaluation, and outcome evidence.
A link is an index entry, not a verification verdict.

The ProductSpec document sits upstream of MASTER_ROADMAP sequence control,
epics/lanes, behavior contracts, Eval Suites, receipts, and the promise
registry. The separate Desktop ProductSpec workroom runtime owns accepted
plans, work packets, leases, evidence envelopes, distinct-verifier references,
and owner packet dispositions for an exact spec. Neither the document
nor that runtime decides assurance adequacy, release, or public claims.

The root [`AUTHORITY.md`](../AUTHORITY.md) and
[`AuthorityDelegationSpec`](../docs/authority/AUTHORITY_DELEGATION_SPEC.md)
separately define who may take an action, against which resource, under which
conditions. They can authorize implementation of ProductSpec intent but cannot
change that intent, supply proof, or make a success metric true.

A spec **declares — it never enforces**:

- Behavior contracts and Eval Suites stay the oracles.
  Specs link contract IDs and suite names. They never duplicate their content.
- The promise registry stays the only authority for public claims. A spec's
  success metrics must be consistent with the promise verification gates it
  links.
- Related Artifacts point at durable external evidence. ProductSpec validation
  checks their shape and item target, not reachability, authenticity,
  sufficiency, freshness, or outcome.
- MASTER_ROADMAP stays the sequence authority.
  Specs never state its sequence again. If they disagree, the roadmap wins.

## Layout and names

```text
specs/<area>/<name>.product-spec.md          # e.g. specs/web/, specs/khala-code/, specs/sarah/
specs/<area>/<name>.decision-trace.json      # optional companion (PS-5, later)
specs/<area>/<name>.assurance-spec.md        # proposed proof-design companion
specs/<area>/<name>.assurance-decision-trace.json # proposed assurance-policy history
```

Owner-directed exception: the first deployable-product package keeps its one
canonical ProductSpec beside its audit at
`docs/mvp/openagents-codex-workroom-mvp.product-spec.md`. The ProductSpec test
sweep validates that co-located file. Its generated AssuranceSpec proposal
likewise lives beside it as
`docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md`. Do not mirror either
artifact back into `specs/`.

Scaffold with:

```sh
bun packages/product-spec/src/cli.ts init specs/<area>/<name>.product-spec.md --title "..."
```

Generate a structurally valid AssuranceSpec proposal from an executable
ProductSpec, optionally with committed repository context:

```sh
bun packages/assurance-spec/src/cli.ts propose \
  specs/<area>/<name>.product-spec.md --repo .
```

The generated proposal is not admitted or executable. Its repository
candidates remain unbound and every unresolved proof-design field reports
`needs_design`.

## Frontmatter policy

- `artifact_type`: `hypothesis` for bets, experiments, and business-agent proposals.
  `prd` for committed product lanes. We do not use `openspec_proposal`.
- `spec_revision`: mandatory from revision 1. Increment when **intent**
  materially changes (scope, acceptance criteria, success metrics, hypothesis)
  — not for typo fixes. Issues, dispatch prompts, and PRs cite
  `specs/<path> @ spec_revision: N`.
- `author`: roles or agent identities only ("OpenAgents", "Sarah", "owner") —
  never individual people's names, per the repo metadata rule.
- `linked_github_repo`: always set.
- `tool_metadata`: flat string map for OpenAgents connections (epic refs, assurance
  level, marginal-cost class, credit budget). **Stripped on any public
  export** (`stripToolMetadata`). Never include secrets, customer data, or private
  price information. Private customer specs live in private repos, not here.

## OpenAgents custom sections

The frontmatter `custom_sections` field declares these sections.
This field is the extension point in the standard.
`init` currently stubs all three:

- `custom-owner-gates` — the NEEDS_OWNER items this work will hit, up front.
- `custom-receipts` — transitional receipt-kind plans from the pre-Evidence
  Loop profile. It is not an evidence ledger. New required-evidence semantics
  belong in AssuranceSpec. Actual portable evidence pointers belong in Related
  Artifacts after the local upstream catch-up.
- `custom-promise-links` — promise-registry IDs this work feeds.

## When a spec is required

Use a ProductSpec for **consequential** work.
Consequential work changes a customer promise, public claim, or invariant surface.
It can also span multiple lanes, agents, or repositories.
Business agents can execute it, or OpenAgents can sell it to a customer.
A phase-level roadmap item is also consequential work.

Use the dated-plan-doc pattern for one-lane mechanical work, refactors, and technical migrations.
Use a ProductSpec for a hypothesis about user or buyer behavior.
Use a plan document when the text gives only a definition of done.

> **Status of the mandatory rules:** the control rules in the adoption
> analysis §5 (spec-required threshold, "no fleet dispatch against a
> spec-backed epic without citing spec@revision", promise-flip reconciliation)
> are **proposed law before owner approval** (analysis §8).
> Until then, this tree and its validator are available tools, not a gate on other lanes.

## The one law already in force here

Never edit a spec to match implementation without a `spec_revision` bump —
accidental behavior never silently becomes intent. This is the same law as
"do not weaken an oracle," one layer up.

## Proposed AssuranceSpec companion (not yet enforced)

The design in `docs/assurance/ASSURANCE_SPEC.md` proposes
an authored `<name>.assurance-spec.md` beside a ProductSpec. The companion
commits **verification intent**: exact criterion bindings, risks, proof
obligations, environments, oracles, falsifiers, evidence policy, gates, and
authority boundaries. It does not add QA semantics to the ProductSpec parser
or replace the Desktop ProductSpec workroom loop. The generated Manifest is an
assurance verification graph, not an implementation plan/work-packet graph.
Applicable Assurance Receipts may be registered by exact reference through the
workroom and published as ProductSpec Related Artifacts.

The proposed revision matrix is:

| Change                                                                                                   | Required action                                                                                                          |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Product intent changes                                                                                   | Increment `spec_revision`. Reconcile and rebind the AssuranceSpec.                                                       |
| Typed diff changes only classified evidence-attachment Related Artifacts and permitted provenance fields | Keep `spec_revision`. Update the exact document digest and evidence projection. Keep the canonical intent digest stable. |
| Proof obligation, risk, proof rung, seam, oracle/falsifier definition, gate, or evidence policy changes  | Increment `assurance_revision`. Append the assurance decision trace when the change is material.                         |
| Native test implementation changes with no proof-intent change                                           | Recompile the manifest. Source and command digests become stale. Dependent evidence also becomes stale.                  |
| Environment capability or policy changes                                                                 | Increment the Environment Profile revision and recompile affected obligations                                            |
| Observed pass/fail/inconclusive result changes                                                           | Emit a new receipt. Never edit an authored spec or the generated manifest.                                               |

The companion must bind the exact ProductSpec path, format version,
`spec_revision`, canonical intent digest, observed exact document digest, and
stable criterion IDs. A changed revision, intent digest, or targeted item is
stale until explicit reconciliation. Only a typed diff limited to classified
evidence attachments may change the document digest with no intent change.
`product_spec` dependencies and consumed `tool_metadata` remain intent-bound.

Until the canonical intent projection is implemented and conformance-tested,
an exact digest mismatch still requires explicit rebind or a pre-bound stable
evidence index. You must not ignore the mismatch. The current Desktop runtime remains
exact-document-digest pinned even after that Assurance-layer classification.

Authored companions live beside ProductSpecs. Reusable public-safe environment
profiles belong under `assurance/environments/`. Deterministic generated
Assurance Manifests belong under `generated/assurance/` if committed. Private
or large run evidence belongs outside Git under the run-artifact store.

These are proposed conventions, not current gates. The bounded proposal
profile has a schema, parser, serializer, validators, and CLI in
`packages/assurance-spec`. Full conformance, custom-section preservation,
admission, and compilation remain planned. The local ProductSpec package also
has not yet implemented upstream `0.19.0` structured items or Related
Artifacts. See `docs/assurance/PRODUCTSPEC_EVIDENCE_LOOP.md`. Do not add
AssuranceSpec files to the ProductSpec validation sweep or claim an Observer
compiler until the full conformance corpus and deterministic compiler exist.
