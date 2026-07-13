# specs/ — Product Spec conventions

This tree holds `.product-spec.md` files: the ProductSpec-format (v0.1) intent
artifacts for consequential OpenAgents work. Full rationale and adoption
design: `docs/fable/2026-07-08-productspec-adoption-analysis.md` (#8593).
Validator and CLI: `packages/product-spec` — every file here is validated in
the normal test sweep (`bun test packages/product-spec`).

## What a spec is (and is not)

A Product Spec is the durable statement of **product intent before
implementation**: problem, hypothesis, scope (in/out/**cut**), acceptance
criteria (the pre-launch build contract), success metrics (the post-launch
market contract). It sits upstream of MASTER_ROADMAP sequencing, epics/lanes,
behavior contracts, Eval Suites, receipts, and the promise registry.

A spec **declares — it never enforces**:

- Behavior contracts and Eval Suites stay the oracles. Specs link contract IDs
  and suite names; they never duplicate their content.
- The promise registry stays the only authority for public claims. A spec's
  success metrics must be consistent with the promise verification gates it
  links.
- MASTER_ROADMAP stays the sequencing authority. Specs never re-state
  sequencing; when they disagree, the roadmap wins.

## Layout and naming

```text
specs/<area>/<name>.product-spec.md          # e.g. specs/web/, specs/khala-code/, specs/sarah/
specs/<area>/<name>.decision-trace.json      # optional companion (PS-5, later)
specs/<area>/<name>.assurance-spec.md        # proposed proof-design companion (not implemented)
specs/<area>/<name>.assurance-decision-trace.json # proposed assurance-policy history
```

Owner-directed exception: the first deployable-product package keeps its one
canonical Product Spec beside its audit at
`docs/mvp/openagents-codex-workroom-mvp.product-spec.md`. The ProductSpec test
sweep validates that co-located file. Do not mirror it back into `specs/`.

Scaffold with:

```sh
bun packages/product-spec/src/cli.ts init specs/<area>/<name>.product-spec.md --title "..."
```

## Frontmatter policy

- `artifact_type`: `hypothesis` for bets/experiments/business-agent proposals;
  `prd` for committed product lanes. We do not use `openspec_proposal`.
- `spec_revision`: mandatory from revision 1. Increment when **intent**
  materially changes (scope, acceptance criteria, success metrics, hypothesis)
  — not for typo fixes. Issues, dispatch prompts, and PRs cite
  `specs/<path> @ spec_revision: N`.
- `author`: roles or agent identities only ("OpenAgents", "Sarah", "owner") —
  never individual people's names, per the repo metadata rule.
- `linked_github_repo`: always set.
- `tool_metadata`: flat string map for OpenAgents wiring (epic refs, assurance
  level, marginal-cost class, credit budget). **Stripped on any public
  export** (`stripToolMetadata`); never secrets, customer data, or private
  pricing — private engagement specs live in private repos, not here.

## OpenAgents custom sections

Declared in frontmatter `custom_sections` (the standard's extension point);
`init` stubs all three:

- `custom-owner-gates` — the NEEDS_OWNER items this work will hit, up front.
- `custom-receipts` — receipt kinds that will prove the acceptance criteria
  (Eval Suite names, behavior-contract IDs, counters).
- `custom-promise-links` — promise-registry IDs this work feeds.

## When a spec is required

For **consequential** work: it changes a customer promise or public claim;
spans multiple lanes/agents/repos; will be executed by business agents or sold
to a customer; changes an invariant surface; or is a phase-level roadmap item.
One-lane mechanical work, refactors, and engineering migrations keep the
dated-plan-doc pattern. Rule of thumb: a hypothesis about user/buyer behavior
→ spec; only a definition of done → plan doc.

> **Status of the binding rules:** the operating rules in the adoption
> analysis §5 (spec-required threshold, "no fleet dispatch against a
> spec-backed epic without citing spec@revision", promise-flip reconciliation)
> are **proposed law pending owner sign-off** (analysis §8). Until then this
> tree and its validator are available tooling, not a gate on other lanes.

## The one law already in force here

Never edit a spec to match implementation without a `spec_revision` bump —
accidental behavior never silently becomes intent. This is the same law as
"do not weaken an oracle," one layer up.

## Proposed AssuranceSpec companion (not yet enforced)

The design in
`docs/fable/2026-07-13-assurancespec-productspec-companion-design.md` proposes
an authored `<name>.assurance-spec.md` beside a Product Spec. The companion
commits **verification intent**: exact criterion bindings, risks, proof
obligations, environments, oracles, falsifiers, evidence policy, gates, and
authority boundaries. It does not add QA semantics to the ProductSpec parser.

The proposed revision matrix is:

| Change | Required action |
| --- | --- |
| Product intent changes | Increment `spec_revision`; reconcile and rebind the Assurance Spec |
| Proof obligation, risk, proof rung, seam, oracle/falsifier meaning, gate, or evidence policy changes | Increment `assurance_revision` and append the assurance decision trace when material |
| Native test implementation changes without changing proof intent | Recompile the manifest; source/command digests and dependent evidence become stale |
| Environment capability or policy changes | Increment the Environment Profile revision and recompile affected obligations |
| Observed pass/fail/inconclusive result changes | Emit a new receipt; never edit either authored spec or the generated manifest |

The companion must bind the exact ProductSpec path, format version,
`spec_revision`, digest, and stable criterion IDs. A changed subject is stale
until explicit reconciliation. Never silently change verification intent to
fit an implementation, silently rebind a changed criterion, or carry old green
evidence across changed dependencies.

Authored companions live beside Product Specs. Reusable public-safe environment
profiles belong under `assurance/environments/`; deterministic generated
Assurance Manifests belong under `generated/assurance/` if committed; private
or large run evidence belongs outside Git under the run-artifact store.

These are proposed conventions, not current gates. Do not add AssuranceSpec
files to the ProductSpec validation sweep or claim an Observer compiler until
the schema, parser, conformance corpus, and deterministic compiler exist.
