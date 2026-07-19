---
spec_format_version: "0.1"
title: "ASSURE-REPO: Programmatic Verification of the OpenAgents Codebase"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-19T00:00:00Z"
updated_at: "2026-07-19T00:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_epic: "9055"
  openagents_lane: "AR-0 through AR-4 (#9056-#9060)"
  openagents_owning_plan: "docs/fable/2026-07-19-verifiable-software.md (Addendum III), owner-admitted 2026-07-19"
  openagents_assurance_level: "repo-verification-inventory"
  openagents_bootstrap_note: "This spec makes the OpenAgents monorepo the first full verification subject of its own verifiable-software program. It composes under Full Auto P0 (#8967 with #8978/#8979 first), IDE-10 #9038 for host-observed tests, and the SBX lane #9023 for sandboxed sweeps. It creates no second completion gate: pnpm run check remains the repository definition of green; ASSURE-REPO maps and grades what that green proves."
---

## Problem

The OpenAgents monorepo has roughly eighty workspace projects — the web app
and its Cloud Run monolith, the desktop app, mobile, Pylon, the Cloud crates,
and dozens of packages — plus served routes, IPC boundaries, public API
endpoints, and release pipelines. Its verification assets are real but
uneven: one completion gate (`pnpm run check`), the behavior-contract
registry, ProductSpec/AssuranceSpec validators with one fully confirmed
precedent (the MVP's 18 obligations with mutation receipts), the promise
registry, smoke suites, and document checks.

No artifact binds that surface to those assets. No one can answer, per
surface, "which oracle proves this, and when did it last prove it?" Untested
surfaces are silent instead of labeled. Test suites can pass for the wrong
reasons (fixture asserts, mocked seams, coverage theater) without any audit
naming them. The repository's own governing documents assert file paths,
commands, and states that nothing checks — the unverified-operational-
directive failure class applied to the codebase itself.

## Hypothesis

If every verification-bearing surface in the repository is inventoried with
typed loss accounting (an oracle ref or an explicit `unverified` reason),
obligations are graded designed versus observed over that inventory, existing
suites are audited for false greens with mutation evidence, a standing Full
Auto lane re-derives and re-runs the whole map against current `main`, and
the repository's own documented claims get drift oracles, then the broad
surface area becomes programmatically verified in the only honest sense:
every green traceable to an oracle, every gap visible, and every claim about
the repo checkable — which is also the strongest public demonstration of the
verifiable-software thesis the product sells.

## Scope

In scope:

- AR-0 (#9056): a schema-validated, deterministically generated surface
  inventory covering apps, packages, crates, routes, workers, IPC channels,
  public endpoints, CLI entrypoints, and release pipelines, each row bound to
  behavior-contract IDs, test refs, assurance obligations, promise IDs, and
  smoke journeys, or tagged `unverified` with a reason. Freshness-guarded.
- AR-1 (#9057): per-surface obligation grading in the existing
  `packages/assurance-spec` vocabulary — mapped, designed, observed, accepted
  as four independent facts, `INCONCLUSIVE` by default, no blended scores.
- AR-2 (#9058): a demonstrated (not pattern-matched) false-green audit over
  the existing suites using the named taxonomy, plus mutation evidence
  extended beyond the MVP precedent to the behavior-contract oracles and
  selected high-value packages.
- AR-3 (#9059): a standing verification sweep as a Full Auto lane —
  sandboxed when SBX is available, degraded and honestly labeled before
  IDE-10 lands — landing per-run receipts that readiness surfaces consume
  under the no-receipt-no-light rule.
- AR-4 (#9060): drift oracles for the repository's own claims: path,
  command, route, and issue-state checks over the governing documents, with
  side-effect-free execution.

Out of scope:

- Writing new product oracles for unverified surfaces (each such oracle is
  its own admitted work; this program makes the gap visible and priced).
- Verification of external or reference repositories.
- Any public claim, promise flip, release decision, or assurance admission.

Cut:

- A repo-wide quality score or single verification percentage. Blended
  scores are structurally excluded; only per-surface facts exist.
- Live-production probing by default in drift or sweep oracles.

## User Experience

The owner (and any agent) can open one generated inventory and see every
surface, its oracles, its obligation state, and its last sweep receipt — or
its explicit `unverified` reason. Sweep runs appear as ordinary Full Auto
runs with typed termination and receipts. Document edits that break a
checkable claim fail the document check with the exact claim and location.
Nothing renders green without a decoded, fresh receipt behind it.

## Acceptance Criteria

- Every inventory row carries at least one oracle ref or an explicit typed
  `unverified` reason; a row with neither fails validation. The inventory is
  derived from the workspace, route, and crate graphs, and a staleness guard
  fails when it ages past its bound relative to `main`.
- Obligation state uses the assurance-spec vocabulary; designed and observed
  are never merged; missing evidence decodes as `INCONCLUSIVE`; intentional
  out-of-scope surfaces carry typed dispositions.
- Every reported false green is demonstrated by a reproduction (such as a
  surviving mutation), and every confirmed finding is fixed or explicitly
  dispositioned; no oracle is weakened to resolve a finding.
- Mutation evidence exists for the behavior-contract oracle set and at least
  one additional package per program area, with kills and survivors reported
  separately and no aggregate mutation score presented as proof.
- A sweep run produces one receipt binding commit, inventory generation,
  oracle set, and exact outcomes; runs execute under unmodified Full Auto
  guardrails; pre-IDE-10 runs label their evidence class honestly.
- Coverage and obligation drift between sweep runs lands as typed findings;
  a surface losing its oracle or an observation going stale is visible, not
  silent.
- At least one consuming readiness surface renders exclusively from sweep
  receipts, with absent or stale receipts rendering unknown, never green,
  enforced by test.
- Drift oracles over the governing documents run in the normal document-check
  path with side-effect-free execution, and the governed set closes with zero
  broken checkable claims or explicit dispositions for each remainder.
- No step in the program self-admits: sweep results are observations, and
  admission, release, and public claims continue to flow through the existing
  AssuranceSpec, owner-gate, and promise-registry authorities.

## Success Metrics

```productspec-success-metrics
- id: silent_surface_count
  metric: inventory_rows_with_neither_oracle_ref_nor_explicit_unverified_reason
  target: "0"
  window: every sweep run after AR-0 closes
  segment: all inventoried repo surfaces
  source: assure_repo_inventory_validation_receipts
- id: demonstrated_false_green_backlog
  metric: confirmed_false_green_findings_without_fix_or_explicit_disposition
  target: "0 at AR-2 close and each sweep thereafter"
  window: AR-2 close, then per sweep run
  segment: audited suites and behavior-contract oracles
  source: false_green_reproduction_and_disposition_ledger
- id: sweep_receipt_integrity
  metric: repo_verification_status_renders_not_backed_by_a_decoded_fresh_sweep_receipt
  target: "0"
  window: from the first consuming surface onward
  segment: readiness and any promise gate citing repo verification
  source: sweep_receipts_and_evidence_gating_tests
- id: governed_document_drift
  metric: broken_checkable_claims_in_governed_documents_without_disposition
  target: "0"
  window: every document-check run after AR-4 closes
  segment: AGENTS.md, INVARIANTS.md, docs/sol/MASTER_ROADMAP.md, and the configured governed set
  source: drift_oracle_findings_ledger
```

## Risks

- Inventory theater: a generated map that ages or overstates coverage. The
  freshness guard and the index-not-verdict rule (an oracle ref proves
  nothing by itself) are the mitigations; AR-1's grading keeps refs and
  proof separate.
- Verifier correlation: agent-classified false greens judged by similar
  agents. AR-2 requires demonstration by reproduction, and the program
  inherits the no-self-admission law; independent falsification capacity
  (per the essay's Addendum II) is the long-run mitigation.
- Sweep sprawl: a standing lane consuming capacity without findings.
  Sweep budgets are typed configuration, and the lane reports drift diffs —
  an empty diff is a cheap run, not a wasted one.
- Oracle weakening pressure: the temptation to loosen a test to clear a
  finding. Prohibited by the behavior-contract law; every fix is
  diff-reviewed against it.

## Solution

Build the program as five packets under epic #9055, in the existing
verification vocabulary rather than new machinery: Effect Schema documents
for the inventory and receipts, `packages/assurance-spec` grading,
the existing mutation harness extended, Full Auto run authority for the
standing sweep, and the document-check path for drift oracles. Sequencing
follows the bootstrap ladder: AR-0 and AR-2 start immediately, AR-1 follows
the inventory, AR-3 reaches full fidelity after IDE-10 (#9038) and SBX
integration (#9027) and is sequenced after Full Auto's own independent
admission (#8978), AR-4 rides the document-check path and joins the sweep.

## Owner Gates

- Any public claim that the OpenAgents codebase is "programmatically
  verified" or any promise-registry transition citing this program. Inventory
  and sweep receipts alone cannot authorize that statement.
- Disposition sign-off for any confirmed false-green finding resolved by
  reclassification rather than a strengthened test, per the
  behavior-contract law.

## Receipts

- AR-0 inventory generation and validation receipts with the zero-silent-
  surface check.
- AR-2 reproduction scripts, mutation run receipts, and the confirmed/
  refuted/dispositioned findings table.
- Per-run AR-3 sweep receipts binding commit, inventory, oracles, outcomes,
  and evidence class.
- AR-4 drift-oracle findings ledgers per governed-document run.

## Promise Links

- None at revision 1. No existing promise cites repo-wide verification; any
  future promise built on this program enters the registry through its own
  copy and verification gates, consuming AR-3 sweep receipts as evidence.
