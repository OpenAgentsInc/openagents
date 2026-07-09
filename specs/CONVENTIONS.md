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
```

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
